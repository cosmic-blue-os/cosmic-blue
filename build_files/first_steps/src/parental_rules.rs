// Parental controls: the rules of a child account and the time it used,
// shared by the service that applies them (parental_daemon.rs, as root) and
// the app that shows and changes them (parental.rs).
//
// Files (JSON, readable by everyone, written by root only):
//   /etc/cosmic-blue/parental-controls/<user>.json      the rules, set by a
//       parent in the app (through cosmic-blue-parental-helper):
//       {"limit": {"enabled": true, "weekdays": 120, "weekend": 180},
//        "bedtime": {"enabled": true, "start": 1230, "end": 420},
//        "web_filter": true}
//       (minutes a day; start and end in minutes after midnight)
//   /var/lib/cosmic-blue/parental-controls/<user>.json  the time used, kept
//       by the service: today's, and the days before for the weekly chart
//   /var/lib/cosmic-blue/parental-controls/<user>.extra more time granted
//       by a parent: one "<unix time> <minutes>" line per grant
//
// An account without a rules file has no limits.

// (Each program uses its part)
#![allow(dead_code)]

use serde_json::{Value, json};
use std::path::PathBuf;

pub const CONFIG_DIR: &str = "/etc/cosmic-blue/parental-controls";
pub const STATE_DIR: &str = "/var/lib/cosmic-blue/parental-controls";
/// Days kept for the weekly chart, today included
pub const HISTORY_DAYS: usize = 7;

#[derive(Clone, Debug, PartialEq)]
pub struct Rules {
    pub limit_enabled: bool,
    /// Minutes a day, Monday to Friday
    pub weekdays: u32,
    /// Minutes a day, Saturday and Sunday
    pub weekend: u32,
    pub bedtime_enabled: bool,
    /// Minutes after midnight
    pub bedtime_start: u32,
    pub bedtime_end: u32,
    pub web_filter: bool,
}

impl Default for Rules {
    /// What a new child account gets
    fn default() -> Self {
        Rules {
            limit_enabled: true,
            weekdays: 120,
            weekend: 180,
            bedtime_enabled: true,
            bedtime_start: 20 * 60 + 30,
            bedtime_end: 7 * 60,
            web_filter: true,
        }
    }
}

impl Rules {
    /// No limits at all: an account without a rules file
    pub fn none() -> Self {
        Rules {
            limit_enabled: false,
            bedtime_enabled: false,
            web_filter: false,
            ..Rules::default()
        }
    }

    pub fn from_json(text: &str) -> Option<Rules> {
        let value: Value = serde_json::from_str(text).ok()?;
        let default = Rules::default();
        let minutes = |v: &Value, max: u32, fallback: u32| {
            v.as_u64()
                .map_or(fallback, |m| u32::try_from(m).unwrap_or(max).min(max))
        };
        Some(Rules {
            limit_enabled: value["limit"]["enabled"].as_bool().unwrap_or(false),
            weekdays: minutes(&value["limit"]["weekdays"], 24 * 60, default.weekdays),
            weekend: minutes(&value["limit"]["weekend"], 24 * 60, default.weekend),
            bedtime_enabled: value["bedtime"]["enabled"].as_bool().unwrap_or(false),
            bedtime_start: minutes(&value["bedtime"]["start"], 24 * 60 - 1, default.bedtime_start),
            bedtime_end: minutes(&value["bedtime"]["end"], 24 * 60 - 1, default.bedtime_end),
            web_filter: value["web_filter"].as_bool().unwrap_or(false),
        })
    }

    pub fn to_json(&self) -> String {
        json!({
            "limit": {"enabled": self.limit_enabled, "weekdays": self.weekdays, "weekend": self.weekend},
            "bedtime": {"enabled": self.bedtime_enabled, "start": self.bedtime_start, "end": self.bedtime_end},
            "web_filter": self.web_filter,
        })
        .to_string()
    }

    /// The rules of an account, if it has any
    pub fn load(user: &str) -> Option<Rules> {
        if !valid_user(user) {
            return None;
        }
        let text = std::fs::read_to_string(config_path(user)).ok()?;
        Rules::from_json(&text)
    }

    /// Minutes allowed on a day (1 = Monday ... 7 = Sunday)
    pub fn daily_limit(&self, weekday: u32) -> Option<u32> {
        self.limit_enabled
            .then_some(if weekday >= 6 { self.weekend } else { self.weekdays })
    }

    /// True when `minute` (after midnight) is in the bedtime hours
    pub fn is_bedtime(&self, minute: u32) -> bool {
        if !self.bedtime_enabled || self.bedtime_start == self.bedtime_end {
            return false;
        }
        if self.bedtime_start < self.bedtime_end {
            (self.bedtime_start..self.bedtime_end).contains(&minute)
        } else {
            minute >= self.bedtime_start || minute < self.bedtime_end
        }
    }
}

/// Account names this feature accepts (what useradd makes by default)
pub fn valid_user(user: &str) -> bool {
    let mut chars = user.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase() || c == '_')
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && user.len() <= 32
}

pub fn config_path(user: &str) -> PathBuf {
    PathBuf::from(CONFIG_DIR).join(format!("{user}.json"))
}

pub fn usage_path(user: &str) -> PathBuf {
    PathBuf::from(STATE_DIR).join(format!("{user}.json"))
}

pub fn extra_path(user: &str) -> PathBuf {
    PathBuf::from(STATE_DIR).join(format!("{user}.extra"))
}

/// The accounts that have rules
pub fn children() -> Vec<String> {
    let mut users: Vec<String> = std::fs::read_dir(CONFIG_DIR)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let user = name.strip_suffix(".json")?;
            valid_user(user).then(|| user.to_owned())
        })
        .collect();
    users.sort();
    users
}

/// The local time, as the service and the login check see it
#[derive(Clone, Debug, PartialEq)]
pub struct Now {
    /// "2026-11-21"
    pub date: String,
    /// Minutes after midnight
    pub minute: u32,
    /// 1 = Monday ... 7 = Sunday
    pub weekday: u32,
    /// Seconds since 1970
    pub unix: i64,
}

impl Now {
    /// From date(1), which knows the time zone
    pub fn read() -> Option<Now> {
        let output = std::process::Command::new("date")
            .arg("+%F %H %M %u %s")
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        Now::parse(&text)
    }

    pub fn parse(text: &str) -> Option<Now> {
        let mut parts = text.split_whitespace();
        let date = parts.next()?.to_owned();
        let hour: u32 = parts.next()?.parse().ok()?;
        let minute: u32 = parts.next()?.parse().ok()?;
        let weekday: u32 = parts.next()?.parse().ok()?;
        let unix: i64 = parts.next()?.parse().ok()?;
        Some(Now {
            date,
            minute: hour * 60 + minute,
            weekday,
            unix,
        })
    }
}

/// One day of use
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Day {
    pub date: String,
    /// 1 = Monday ... 7 = Sunday
    pub weekday: u32,
    pub seconds: u64,
}

/// The time an account used: today, and the days before (oldest first)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Usage {
    pub today: Day,
    pub history: Vec<Day>,
}

impl Usage {
    pub fn from_json(text: &str) -> Usage {
        let value: Value = serde_json::from_str(text).unwrap_or_default();
        let day = |v: &Value| Day {
            date: v["date"].as_str().unwrap_or_default().to_owned(),
            weekday: v["weekday"].as_u64().unwrap_or(0) as u32,
            seconds: v["seconds"].as_u64().unwrap_or(0),
        };
        Usage {
            today: day(&value["today"]),
            history: value["history"]
                .as_array()
                .into_iter()
                .flatten()
                .map(day)
                .collect(),
        }
    }

    pub fn to_json(&self) -> String {
        let day = |d: &Day| json!({"date": d.date, "weekday": d.weekday, "seconds": d.seconds});
        json!({
            "today": day(&self.today),
            "history": self.history.iter().map(day).collect::<Vec<_>>(),
        })
        .to_string()
    }

    pub fn load(user: &str) -> Usage {
        std::fs::read_to_string(usage_path(user))
            .map(|text| Usage::from_json(&text))
            .unwrap_or_default()
    }

    /// Starts a new day when the date changed; true when it did
    pub fn roll(&mut self, now: &Now) -> bool {
        if self.today.date == now.date {
            return false;
        }
        if !self.today.date.is_empty() {
            self.history.push(std::mem::take(&mut self.today));
        }
        // Only the days of the last week, for the chart
        let keep = HISTORY_DAYS - 1;
        if self.history.len() > keep {
            self.history.drain(..self.history.len() - keep);
        }
        self.today = Day {
            date: now.date.clone(),
            weekday: now.weekday,
            seconds: 0,
        };
        true
    }

    /// Seconds used today (0 when the file is from another day)
    pub fn used_today(&self, now: &Now) -> u64 {
        if self.today.date == now.date {
            self.today.seconds
        } else {
            0
        }
    }
}

/// More time granted by a parent: (unix time of the grant, minutes)
pub fn load_extra(user: &str) -> Vec<(i64, u32)> {
    std::fs::read_to_string(extra_path(user))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
        })
        .collect()
}

/// Why an account cannot use the computer now
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Block {
    /// The daily screen time is used up
    Limit,
    /// Bedtime hours
    Bedtime,
}

/// What the rules allow now
#[derive(Clone, Debug, PartialEq)]
pub struct Verdict {
    /// Some: no use allowed now, and why
    pub blocked: Option<Block>,
    /// Seconds left before the session is closed (None: no end today)
    pub remaining: Option<u64>,
    /// Today's limit, more time included, in seconds (None: no limit)
    pub allowed_today: Option<u64>,
    /// Minute after midnight when a login is possible again, when blocked
    pub back_at: Option<u32>,
}

/// Applies the rules to the time used and the time granted
pub fn verdict(rules: &Rules, used: u64, extra: &[(i64, u32)], now: &Now) -> Verdict {
    // More time granted today adds to the daily limit; granted less than
    // its length ago, it also allows that long past the bedtime start
    let day_start = now.unix - i64::from(now.minute) * 60;
    let extra_today: u64 = extra
        .iter()
        .filter(|(at, _)| *at >= day_start)
        .map(|(_, minutes)| u64::from(*minutes) * 60)
        .sum();
    let override_until = extra
        .iter()
        .map(|(at, minutes)| at + i64::from(*minutes) * 60)
        .max()
        .filter(|until| *until > now.unix);

    // 0 minutes granted: no screen time limit for the rest of the day (the
    // bedtime hours still apply)
    let unlimited_today = extra.iter().any(|(at, minutes)| *at >= day_start && *minutes == 0);
    let allowed_today = rules
        .daily_limit(now.weekday)
        .filter(|_| !unlimited_today)
        .map(|minutes| u64::from(minutes) * 60 + extra_today);
    let left_today = allowed_today.map(|allowed| allowed.saturating_sub(used));

    let bedtime_now = rules.is_bedtime(now.minute);
    let in_override = override_until.is_some();
    // Seconds before the bedtime starts (or before the extra time granted
    // during it ends)
    let to_bedtime = if !rules.bedtime_enabled || rules.bedtime_start == rules.bedtime_end {
        None
    } else if let Some(until) = override_until {
        if bedtime_now || rules.is_bedtime(minute_at(now, until)) {
            Some((until - now.unix) as u64)
        } else {
            Some(u64::from((rules.bedtime_start + 24 * 60 - now.minute) % (24 * 60)) * 60)
        }
    } else if bedtime_now {
        Some(0)
    } else {
        Some(u64::from((rules.bedtime_start + 24 * 60 - now.minute) % (24 * 60)) * 60)
    };

    let blocked = if bedtime_now && !in_override {
        Some(Block::Bedtime)
    } else if left_today == Some(0) {
        Some(Block::Limit)
    } else {
        None
    };
    let remaining = match (left_today, to_bedtime) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    let back_at = blocked.map(|block| match block {
        Block::Bedtime => rules.bedtime_end,
        // Tomorrow: at midnight, or when tomorrow's bedtime hours end
        Block::Limit if rules.is_bedtime(0) => rules.bedtime_end,
        Block::Limit => 0,
    });
    Verdict {
        blocked,
        remaining: if blocked.is_some() { Some(0) } else { remaining },
        allowed_today,
        back_at,
    }
}

/// The minute after midnight of a unix time, today's time zone offset
/// assumed
fn minute_at(now: &Now, unix: i64) -> u32 {
    let delta = (unix - now.unix).div_euclid(60);
    (i64::from(now.minute) + delta).rem_euclid(24 * 60) as u32
}

/// "20:30"
pub fn clock(minute: u32) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now(minute: u32, weekday: u32) -> Now {
        Now {
            date: "2026-11-20".into(),
            minute,
            weekday,
            unix: 1_000_000 + i64::from(minute) * 60,
        }
    }

    #[test]
    fn limit() {
        let rules = Rules {
            bedtime_enabled: false,
            ..Rules::default()
        };
        let v = verdict(&rules, 60 * 60, &[], &now(600, 5));
        assert_eq!(v.remaining, Some(60 * 60));
        assert_eq!(v.blocked, None);
        let v = verdict(&rules, 120 * 60, &[], &now(600, 5));
        assert_eq!(v.blocked, Some(Block::Limit));
        assert_eq!(v.back_at, Some(0));
        // Weekend limit
        let v = verdict(&rules, 120 * 60, &[], &now(600, 6));
        assert_eq!(v.remaining, Some(60 * 60));
        // More time
        let n = now(600, 5);
        let v = verdict(&rules, 120 * 60, &[(n.unix - 60, 15)], &n);
        assert_eq!(v.remaining, Some(15 * 60));
        // No limit today, but not tomorrow
        let v = verdict(&rules, 300 * 60, &[(n.unix - 60, 0)], &n);
        assert_eq!((v.blocked, v.remaining, v.allowed_today), (None, None, None));
        let v = verdict(&rules, 300 * 60, &[(n.unix - 86400, 0)], &n);
        assert_eq!(v.blocked, Some(Block::Limit));
        // Bedtime still applies
        let rules = Rules::default();
        let v = verdict(&rules, 300 * 60, &[(n.unix - 60, 0)], &now(21 * 60, 5));
        assert_eq!(v.blocked, Some(Block::Bedtime));
    }

    #[test]
    fn bedtime() {
        let rules = Rules {
            limit_enabled: false,
            ..Rules::default()
        };
        assert!(rules.is_bedtime(21 * 60));
        assert!(rules.is_bedtime(6 * 60));
        assert!(!rules.is_bedtime(12 * 60));
        let v = verdict(&rules, 0, &[], &now(20 * 60, 5));
        assert_eq!(v.remaining, Some(30 * 60));
        let v = verdict(&rules, 0, &[], &now(21 * 60, 5));
        assert_eq!(v.blocked, Some(Block::Bedtime));
        assert_eq!(v.back_at, Some(7 * 60));
        // More time during bedtime
        let n = now(21 * 60, 5);
        let v = verdict(&rules, 0, &[(n.unix - 5 * 60, 30)], &n);
        assert_eq!(v.blocked, None);
        assert_eq!(v.remaining, Some(25 * 60));
        // Both: the first end wins
        let rules = Rules::default();
        let v = verdict(&rules, 110 * 60, &[], &now(20 * 60, 5));
        assert_eq!(v.remaining, Some(10 * 60));
    }

    #[test]
    fn json() {
        let rules = Rules::default();
        assert_eq!(Rules::from_json(&rules.to_json()), Some(rules));
        let mut usage = Usage::default();
        for day in 1..=9 {
            let n = Now {
                date: format!("2026-11-{day:02}"),
                minute: 0,
                weekday: 1,
                unix: 0,
            };
            usage.roll(&n);
            usage.today.seconds = day;
        }
        assert_eq!(usage.history.len(), HISTORY_DAYS - 1);
        assert_eq!(Usage::from_json(&usage.to_json()), usage);
        assert!(valid_user("lea"));
        assert!(!valid_user("../etc"));
        assert!(!valid_user("Lea"));
    }
}
