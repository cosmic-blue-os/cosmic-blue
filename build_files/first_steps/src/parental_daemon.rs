// cosmic-blue parental controls: the service that applies the rules of the
// child accounts (see parental_rules.rs), as root.
//
//   cosmic-blue-parental-daemon run
//       the service (cosmic-blue-parental-controls.service), every 15 s:
//         - counts the screen time of each child account: the time one of
//           its sessions is the one on screen, not locked;
//         - closes the sessions of an account when its time is up or at
//           bedtime (the session agent warns 10, 5 and 1 minutes before,
//           see `cosmic-blue-parental-controls --agent`);
//         - sends the DNS queries of the accounts with the web filter on to
//           a family filter (cosmic-blue-family-dns.service): nftables rules
//           for their user ids, and no access to systemd-resolved's own
//           socket, so that the name lookups go through DNS.
//   cosmic-blue-parental-daemon login-check
//       PAM (pam_exec, account): refuses a login during bedtime or once the
//       day's time is used up, with a message saying when it is possible
//       again. Accounts without rules are always let in.
//   cosmic-blue-parental-daemon refusal
//       the login screen's message when the check gives no reason
//   cosmic-blue-parental-daemon status
//       what the rules allow now, for each child account (tests)

use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[allow(dead_code)] // (the translation check is the app's)
mod i18n;
mod parental_rules;
use i18n::tr;
use parental_rules::{Block, Now, Rules, Usage};

// The texts of the login check are the app's (po/parental)
pub(crate) const LANGUAGES: &[(&str, &str)] = &[
    ("de", include_str!("../po/parental/de.po")),
    ("es", include_str!("../po/parental/es.po")),
    ("fr", include_str!("../po/parental/fr.po")),
    ("it", include_str!("../po/parental/it.po")),
    ("pt_BR", include_str!("../po/parental/pt_BR.po")),
];
#[allow(dead_code)]
pub(crate) const TEMPLATE: &str = include_str!("../po/parental/cosmic-blue-parental-controls.pot");
#[allow(dead_code)]
pub(crate) const PO_DIR: &str = "po/parental";
#[allow(dead_code)]
pub(crate) const TEMPLATE_NAME: &str = "cosmic-blue-parental-controls.pot";

const TICK: Duration = Duration::from_secs(15);
const TABLE: &str = "cosmic_blue_parental";
/// The family filter (dnsmasq, see cosmic-blue-family-dns.service)
const FILTER_SERVICE: &str = "cosmic-blue-family-dns.service";
/// Port 53 on 127.0.0.1: systemd-resolved only listens on 127.0.0.53 and
/// 127.0.0.54, and SELinux lets dnsmasq bind the DNS port
const FILTER_PORT: u16 = 53;
/// systemd-resolved's socket, used by glibc's name lookups (nss-resolve)
/// before DNS: closed to the filtered accounts, whose lookups then go
/// through DNS (/etc/resolv.conf), which nftables sends to the filter
const RESOLVE_SOCKET: &str = "/run/systemd/resolve/io.systemd.Resolve";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => run(),
        Some("login-check") => std::process::exit(login_check()),
        Some("status") => status(),
        Some("refusal") => {
            // The login screen's message when the check gives no reason
            system_language();
            println!("{}", tr("Parental controls do not allow logging in now. Ask a parent."));
        }
        _ => {
            eprintln!("Usage: cosmic-blue-parental-daemon run | login-check | refusal | status");
            std::process::exit(2);
        }
    }
}

/// The user id of an account
fn uid(user: &str) -> Option<u32> {
    let output = Command::new("id").args(["-u", user]).output().ok()?;
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

/// One login session (loginctl)
struct Session {
    id: String,
    /// The session on screen, not locked
    on_screen: bool,
}

/// The login sessions of an account (none when it is not logged in)
fn sessions(user: &str) -> Vec<Session> {
    let Ok(output) = Command::new("loginctl")
        .args(["show-user", user, "-p", "Sessions", "--value"])
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .filter_map(|id| {
            let props = Command::new("loginctl")
                .args(["show-session", id, "-p", "State", "-p", "LockedHint", "-p", "Class"])
                .output()
                .ok()?;
            let props = String::from_utf8_lossy(&props.stdout).into_owned();
            let prop = |name: &str| {
                props
                    .lines()
                    .find_map(|line| line.strip_prefix(name)?.strip_prefix('='))
                    .unwrap_or_default()
                    .to_owned()
            };
            let on_screen = prop("State") == "active"
                && prop("LockedHint") != "yes"
                && prop("Class").starts_with("user");
            Some(Session {
                id: id.to_owned(),
                on_screen,
            })
        })
        .collect()
}

/// Writes a file readable by everyone, all at once
fn write_file(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(tmp, path)
}

fn run_quiet(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// The nftables rules for the accounts with the web filter on: their DNS
/// queries to systemd-resolved's stub go to the filter, and DNS straight to
/// other servers (or DNS over TLS) is refused
fn filter_rules(uids: &BTreeSet<u32>) -> String {
    let set = uids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "table inet {TABLE}
delete table inet {TABLE}
table inet {TABLE} {{
    chain dns_nat {{
        type nat hook output priority dstnat; policy accept;
        meta skuid {{ {set} }} ip daddr {{ 127.0.0.53, 127.0.0.54 }} meta l4proto {{ tcp, udp }} th dport 53 dnat ip to 127.0.0.1:{FILTER_PORT}
    }}
    chain dns_filter {{
        type filter hook output priority filter; policy accept;
        meta skuid {{ {set} }} ip daddr != 127.0.0.0/8 meta l4proto {{ tcp, udp }} th dport {{ 53, 853 }} reject
        meta skuid {{ {set} }} ip6 daddr != ::1 meta l4proto {{ tcp, udp }} th dport {{ 53, 853 }} reject
    }}
}}
"
    )
}

/// What the service last applied, to change only what changed
#[derive(Default)]
struct Applied {
    filtered: Option<BTreeSet<u32>>,
    /// Whether each child account was on screen at the last tick (logged
    /// when it changes)
    on_screen: std::collections::BTreeMap<String, bool>,
}

impl Applied {
    fn web_filter(&mut self, uids: BTreeSet<u32>) {
        // Re-applied every time: systemd-resolved makes a new socket when it
        // restarts, and the rules must survive a firewall reload
        let previous = self.filtered.take().unwrap_or_default();
        for uid in previous.difference(&uids) {
            run_quiet("setfacl", &["-x", &format!("u:{uid}"), RESOLVE_SOCKET]);
        }
        if uids.is_empty() {
            if !previous.is_empty() || self.filtered.is_none() {
                run_quiet("nft", &["delete", "table", "inet", TABLE]);
                run_quiet("systemctl", &["stop", FILTER_SERVICE]);
            }
            self.filtered = Some(uids);
            return;
        }
        if !run_quiet("systemctl", &["is-active", "--quiet", FILTER_SERVICE]) {
            run_quiet("systemctl", &["start", FILTER_SERVICE]);
        }
        let applied = run_quiet("nft", &["list", "table", "inet", TABLE]);
        if previous != uids || !applied {
            let rules = filter_rules(&uids);
            let ok = Command::new("nft")
                .args(["-f", "-"])
                .stdin(Stdio::piped())
                .spawn()
                .and_then(|mut child| {
                    child.stdin.take().unwrap().write_all(rules.as_bytes())?;
                    child.wait()
                })
                .is_ok_and(|status| status.success());
            if !ok {
                eprintln!("Could not apply the web filter rules");
            }
        }
        for uid in &uids {
            run_quiet("setfacl", &["-m", &format!("u:{uid}:---"), RESOLVE_SOCKET]);
        }
        self.filtered = Some(uids);
    }
}

fn run() {
    let mut applied = Applied::default();
    let mut last = Instant::now();
    loop {
        let elapsed = last.elapsed().as_secs().min(60);
        last = Instant::now();
        if let Some(now) = Now::read() {
            tick(&now, elapsed, &mut applied);
        }
        std::thread::sleep(TICK);
    }
}

fn tick(now: &Now, elapsed: u64, applied: &mut Applied) {
    let mut filtered = BTreeSet::new();
    for user in parental_rules::children() {
        let Some(rules) = Rules::load(&user) else {
            continue;
        };
        let Some(uid) = uid(&user) else {
            continue;
        };
        if rules.web_filter {
            filtered.insert(uid);
        }
        let mine = sessions(&user);
        let on_screen = mine.iter().any(|s| s.on_screen);
        if applied.on_screen.get(&user) != Some(&on_screen) {
            println!(
                "{user}: {} session(s), {}",
                mine.len(),
                if on_screen { "on screen: counting" } else { "not on screen" }
            );
            applied.on_screen.insert(user.clone(), on_screen);
        }

        let mut usage = Usage::load(&user);
        let mut changed = usage.roll(now);
        if on_screen && elapsed > 0 {
            usage.today.seconds += elapsed;
            changed = true;
        }
        if changed {
            if let Err(error) = write_file(&parental_rules::usage_path(&user), &usage.to_json()) {
                eprintln!("{user}: could not save the time used: {error}");
            }
        }

        let extra = parental_rules::load_extra(&user);
        let verdict = parental_rules::verdict(&rules, usage.today.seconds, &extra, now);
        if let Some(block) = verdict.blocked {
            if !mine.is_empty() {
                println!(
                    "{user}: {}, closing the sessions {}",
                    match block {
                        Block::Limit => "screen time used up",
                        Block::Bedtime => "bedtime",
                    },
                    mine.iter().map(|s| s.id.as_str()).collect::<Vec<_>>().join(" ")
                );
                if !run_quiet("loginctl", &["terminate-user", &user]) {
                    eprintln!("{user}: could not close the sessions");
                }
            }
        }
    }
    applied.web_filter(filtered);
}

/// The language of the system (/etc/locale.conf), for the login check: PAM
/// modules do not get the greeter's
fn system_language() {
    if std::env::var_os("LANG").is_some() {
        return;
    }
    let lang = std::fs::read_to_string("/etc/locale.conf")
        .unwrap_or_default()
        .lines()
        .find_map(|line| {
            line.strip_prefix("LANG=")
                .map(|value| value.trim_matches('"').to_owned())
        });
    if let Some(lang) = lang {
        // Single-threaded here, before any translation is read
        unsafe { std::env::set_var("LANG", lang) };
    }
}

/// The message shown on the login screen, when the rules refuse the login
fn refusal(user: &str) -> Option<String> {
    let rules = Rules::load(user)?;
    let now = Now::read()?;
    let usage = Usage::load(user);
    let extra = parental_rules::load_extra(user);
    let verdict = parental_rules::verdict(&rules, usage.used_today(&now), &extra, &now);
    let block = verdict.blocked?;
    let back = verdict.back_at.unwrap_or(0);
    Some(match block {
        Block::Bedtime => format!(
            "{}\n{}",
            tr("It is bedtime."),
            tr("You can log in again at {}.").replacen("{}", &parental_rules::clock(back), 1)
        ),
        Block::Limit if back == 0 => format!(
            "{}\n{}",
            tr("Screen time is over for today."),
            tr("You can log in again tomorrow.")
        ),
        Block::Limit => format!(
            "{}\n{}",
            tr("Screen time is over for today."),
            tr("You can log in again tomorrow at {}.").replacen("{}", &parental_rules::clock(back), 1)
        ),
    })
}

fn login_check() -> i32 {
    let Ok(user) = std::env::var("PAM_USER") else {
        return 0;
    };
    system_language();
    // stderr goes to the journal of the login screen
    match refusal(&user) {
        Some(message) => {
            eprintln!("cosmic-blue-parental-daemon: login refused for {user}");
            println!("{message}");
            1
        }
        None => {
            if Rules::load(&user).is_some() {
                eprintln!("cosmic-blue-parental-daemon: login allowed for {user}");
            }
            0
        }
    }
}

fn status() {
    let Some(now) = Now::read() else {
        eprintln!("Could not read the time");
        std::process::exit(1);
    };
    for user in parental_rules::children() {
        let Some(rules) = Rules::load(&user) else {
            continue;
        };
        let usage = Usage::load(&user);
        let extra = parental_rules::load_extra(&user);
        let verdict = parental_rules::verdict(&rules, usage.used_today(&now), &extra, &now);
        println!(
            "{user}: used {}s, allowed {}, remaining {}, blocked {}, web filter {}",
            usage.used_today(&now),
            verdict
                .allowed_today
                .map_or("unlimited".to_owned(), |s| format!("{s}s")),
            verdict.remaining.map_or("unlimited".to_owned(), |s| format!("{s}s")),
            match verdict.blocked {
                Some(Block::Limit) => "limit",
                Some(Block::Bedtime) => "bedtime",
                None => "no",
            },
            if rules.web_filter { "on" } else { "off" },
        );
    }
}
