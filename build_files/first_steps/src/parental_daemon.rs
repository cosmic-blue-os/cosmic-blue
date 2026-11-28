// Celenn OS parental controls: the service that applies the rules of the
// child accounts (see parental_rules.rs), as root.
//
//   celenn-parental-daemon run
//       the service (celenn-parental-controls.service), every 15 s:
//         - counts the screen time of each child account: the time one of
//           its sessions is the one on screen, not locked; and the same
//           time by application in front, as the session's agent says;
//         - closes the sessions of an account when its time is up or at
//           bedtime (the session agent warns 10, 5 and 1 minutes before,
//           see `celenn-parental-controls --agent`);
//         - sends the DNS queries of the accounts with the web filter on to
//           a family filter (celenn-family-dns.service): nftables rules
//           for their user ids, and no access to systemd-resolved's own
//           socket, so that the name lookups go through DNS.
//       and, every 2 s (another thread), for the accounts whose
//       applications are validated by a parent (parental_apps.rs): stops the
//       applications not allowed and the downloaded programs, and tells the
//       child why.
//   celenn-parental-daemon login-check
//       the login screen's check (the PAM account module
//       pam_celenn_parental.so, see pam_parental.rs): refuses a login
//       during bedtime or once the day's time is used up, with a message
//       saying when it is possible again. Accounts without rules are
//       always let in.
//   celenn-parental-daemon refusal
//       the login screen's message when the check gives no reason
//   celenn-parental-daemon status
//       what the rules allow now, for each child account (tests)

use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[allow(dead_code)] // (the translation check is the app's)
mod i18n;
mod parental_apps;
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
pub(crate) const TEMPLATE: &str = include_str!("../po/parental/celenn-parental-controls.pot");
#[allow(dead_code)]
pub(crate) const PO_DIR: &str = "po/parental";
#[allow(dead_code)]
pub(crate) const TEMPLATE_NAME: &str = "celenn-parental-controls.pot";

const TICK: Duration = Duration::from_secs(15);
const TABLE: &str = "celenn_parental";
/// The family filter (dnsmasq, see celenn-family-dns.service)
const FILTER_SERVICE: &str = "celenn-family-dns.service";
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
            println!(
                "{}",
                tr("Parental controls do not allow logging in now. Ask a parent.")
            );
        }
        _ => {
            eprintln!("Usage: celenn-parental-daemon run | login-check | refusal | status");
            std::process::exit(2);
        }
    }
}

/// The user id of an account
fn uid(user: &str) -> Option<u32> {
    let output = Command::new("id").args(["-u", user]).output().ok()?;
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

/// The home folder of an account
fn home(user: &str) -> Option<std::path::PathBuf> {
    let output = Command::new("getent")
        .args(["passwd", user])
        .output()
        .ok()?;
    let line = String::from_utf8_lossy(&output.stdout).into_owned();
    let home = line.trim_end().split(':').nth(5)?;
    home.starts_with('/').then(|| home.into())
}

/// The application in front in a child's session: the app id its agent
/// wrote less than half a minute ago (parental_rules::FOCUS). The file is
/// the child's: opened without following links, without waiting (not a
/// pipe), a few bytes at most, and only an app id is kept from it.
fn focused_app(user: &str) -> Option<String> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    let path = home(user)?.join(parental_rules::FOCUS);
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc_flags())
        .open(path)
        .ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file()
        || metadata
            .modified()
            .ok()?
            .elapsed()
            .unwrap_or_default()
            .as_secs()
            > parental_rules::FOCUS_FRESH
    {
        return None;
    }
    let mut text = String::new();
    file.take(256).read_to_string(&mut text).ok()?;
    parental_rules::clean_app_id(&text)
}

/// O_NOFOLLOW | O_NONBLOCK (Linux's values; no libc crate here)
fn libc_flags() -> i32 {
    const O_NOFOLLOW: i32 = if cfg!(any(target_arch = "aarch64", target_arch = "arm")) {
        0o100_000
    } else {
        0o400_000
    };
    O_NOFOLLOW | 0o4000
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
                .args([
                    "show-session",
                    id,
                    "-p",
                    "State",
                    "-p",
                    "LockedHint",
                    "-p",
                    "Class",
                ])
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

/// Public resolvers that answer DNS over HTTPS (port 443): a browser set to
/// use one of them would skip the family filter. CleanBrowsing, the filter
/// itself, is not in the list.
const DOH_V4: &str = "1.1.1.1, 1.0.0.1, 1.1.1.2, 1.0.0.2, 1.1.1.3, 1.0.0.3, \
    8.8.8.8, 8.8.4.4, 9.9.9.9, 149.112.112.112, 9.9.9.10, 149.112.112.10, \
    9.9.9.11, 149.112.112.11, 94.140.14.14, 94.140.15.15, 94.140.14.140, \
    94.140.14.141, 208.67.222.222, 208.67.220.220, 208.67.222.123, \
    208.67.220.123, 45.90.28.0/24, 45.90.30.0/24, 76.76.2.0/24, 76.76.10.0/24, \
    194.242.2.0/24, 101.101.101.101, 223.5.5.5, 223.6.6.6";
const DOH_V6: &str = "2606:4700:4700::1111, 2606:4700:4700::1001, \
    2606:4700:4700::1112, 2606:4700:4700::1002, 2606:4700:4700::1113, \
    2606:4700:4700::1003, 2001:4860:4860::8888, 2001:4860:4860::8844, \
    2620:fe::fe, 2620:fe::9, 2620:fe::10, 2620:fe::fe:10, 2620:fe::11, \
    2620:fe::fe:11, 2a10:50c0::ad1:ff, 2a10:50c0::ad2:ff, 2620:119:35::35, \
    2620:119:53::53, 2a07:a8c0::/32, 2a07:e340::/32";
/// Ports of the usual VPN protocols (WireGuard, OpenVPN, IPsec, L2TP,
/// PPTP), refused to the filtered accounts' own programs. Connections made
/// by NetworkManager run as root: polkit keeps children from adding them
/// (10-celenn-parental.rules).
const VPN_UDP: &str = "51820, 1194, 500, 4500, 1701";
const VPN_TCP: &str = "1194, 1723";

/// The nftables rules for the accounts with the web filter on: their DNS
/// queries to systemd-resolved's stub go to the filter, and DNS straight to
/// other servers, DNS over TLS, DNS over HTTPS to the public resolvers and
/// the usual VPN ports are refused
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
        meta skuid {{ {set} }} ip daddr {{ {DOH_V4} }} meta l4proto {{ tcp, udp }} th dport 443 reject
        meta skuid {{ {set} }} ip6 daddr {{ {DOH_V6} }} meta l4proto {{ tcp, udp }} th dport 443 reject
        meta skuid {{ {set} }} udp dport {{ {VPN_UDP} }} reject
        meta skuid {{ {set} }} tcp dport {{ {VPN_TCP} }} reject
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
    // Before the second thread starts: the environment is only safe to
    // change while nothing else reads it
    system_language();
    std::thread::spawn(watch_apps);
    let mut applied = Applied::default();
    let mut last = Instant::now();
    loop {
        let elapsed = last.elapsed().as_secs().min(60);
        last = Instant::now();
        match Now::read() {
            Some(now) => tick(&now, elapsed, &mut applied),
            None => eprintln!("Could not read the time: no rules applied"),
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
                if on_screen {
                    "on screen: counting"
                } else {
                    "not on screen"
                }
            );
            applied.on_screen.insert(user.clone(), on_screen);
        }

        let mut usage = Usage::load(&user);
        let mut changed = usage.roll(now);
        if on_screen && elapsed > 0 {
            usage.today.seconds += elapsed;
            // The same time for the application in front, as the child's
            // session says (only its name comes from the child)
            if let Some(app) = focused_app(&user) {
                *usage.today.apps.entry(app).or_default() += elapsed;
            }
            changed = true;
        }
        if changed
            && let Err(error) = write_file(&parental_rules::usage_path(&user), &usage.to_json())
        {
            eprintln!("{user}: could not save the time used: {error}");
        }

        let extra = parental_rules::load_extra(&user);
        let verdict = parental_rules::verdict(&rules, usage.today.seconds, &extra, now);
        if let Some(block) = verdict.blocked
            && !mine.is_empty()
        {
            println!(
                "{user}: {}, closing the sessions {}",
                match block {
                    Block::Limit => "screen time used up",
                    Block::Bedtime => "bedtime",
                },
                mine.iter()
                    .map(|s| s.id.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            if !run_quiet("loginctl", &["terminate-user", &user]) {
                eprintln!("{user}: could not close the sessions");
            }
        }
    }
    applied.web_filter(filtered);
    sync_group(&parental_rules::children());
}

/// The group of the child accounts, which polkit and D-Bus rules use: no
/// network connections of their own (VPN), no time zone change, no direct
/// systemd-resolved lookups. Its members are the accounts with rules (the
/// helper adds a new child at once; this also removes former ones).
const GROUP: &str = "celenn-children";

fn sync_group(children: &[String]) {
    let Ok(output) = Command::new("getent").args(["group", GROUP]).output() else {
        return;
    };
    if !output.status.success() {
        eprintln!("No {GROUP} group (systemd-sysusers)");
        return;
    }
    let line = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let members: BTreeSet<&str> = line
        .rsplit(':')
        .next()
        .unwrap_or_default()
        .split(',')
        .filter(|member| !member.is_empty())
        .collect();
    for child in children {
        if !members.contains(child.as_str())
            && uid(child).is_some()
            && run_quiet("gpasswd", &["-a", child, GROUP])
        {
            println!("{child}: added to {GROUP}");
        }
    }
    for member in members {
        if !children.iter().any(|child| child == member)
            && run_quiet("gpasswd", &["-d", member, GROUP])
        {
            println!("{member}: removed from {GROUP}");
        }
    }
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
        // Every caller is still single-threaded (run() calls this before
        // it starts the applications thread), and no translation was read
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
            tr("You can log in again tomorrow at {}.").replacen(
                "{}",
                &parental_rules::clock(back),
                1
            )
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
            eprintln!("celenn-parental-daemon: login refused for {user}");
            println!("{message}");
            1
        }
        None => {
            if Rules::load(&user).is_some() {
                eprintln!("celenn-parental-daemon: login allowed for {user}");
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
            verdict
                .remaining
                .map_or("unlimited".to_owned(), |s| format!("{s}s")),
            match verdict.blocked {
                Some(Block::Limit) => "limit",
                Some(Block::Bedtime) => "bedtime",
                None => "no",
            },
            if rules.web_filter { "on" } else { "off" },
        );
        if rules.apps_enabled {
            println!(
                "{user}: applications validated by a parent: {}",
                rules.allowed_apps.len()
            );
        }
    }
}

/// Applications validated by a parent: every 2 s, stops what the children
/// with that option may not run (see parental_apps.rs)
fn watch_apps() {
    const EVERY: Duration = Duration::from_secs(2);
    /// The installed applications are read again after this (new ones)
    const APPS_AGAIN: Duration = Duration::from_secs(60);
    // (the system language was set by run(), before this thread started)
    let lang = std::env::var("LANG").unwrap_or_default();
    let mut apps: Option<(Instant, Vec<parental_apps::App>)> = None;
    // Last message per child and reason, not to repeat it every 2 s
    let mut told: std::collections::BTreeMap<(u32, parental_apps::Stop), Instant> =
        Default::default();
    loop {
        std::thread::sleep(EVERY);
        let children: Vec<(String, u32, Rules)> = parental_rules::children()
            .into_iter()
            .filter_map(|user| {
                let rules = Rules::load(&user).filter(|rules| rules.apps_enabled)?;
                Some((user.clone(), uid(&user)?, rules))
            })
            .collect();
        if children.is_empty() {
            continue;
        }
        if apps
            .as_ref()
            .is_none_or(|(read, _)| read.elapsed() > APPS_AGAIN)
        {
            apps = Some((Instant::now(), parental_apps::installed(&lang)));
        }
        let Some((_, list)) = &apps else {
            continue;
        };
        for (user, uid, rules) in children {
            let policy = parental_apps::Policy::new(list, &rules.allowed_apps);
            for (pid, exe, cgroup) in parental_apps::processes(uid) {
                let Some(stop) = policy.check(exe.as_deref(), &cgroup) else {
                    continue;
                };
                // At once, without letting it save: it should not run at all
                let _ = Command::new("kill")
                    .args(["-KILL", &pid.to_string()])
                    .status();
                let key = (uid, stop.clone());
                if told
                    .get(&key)
                    .is_some_and(|at| at.elapsed() < Duration::from_secs(10))
                {
                    continue;
                }
                println!(
                    "{user}: stopped {} ({stop:?})",
                    exe.as_deref()
                        .map(|e| e.display().to_string())
                        .unwrap_or_default()
                );
                tell_child(&user, uid, &stop);
                told.insert(key, Instant::now());
            }
        }
        told.retain(|_, at| at.elapsed() < Duration::from_secs(60));
    }
}

/// A notification in the child's session saying what was stopped and why
fn tell_child(user: &str, uid: u32, stop: &parental_apps::Stop) {
    let (title, body) = match stop {
        parental_apps::Stop::App(name) => (
            tr("{} is not allowed").replacen("{}", name, 1),
            tr("A parent must allow it first in Parental controls.").to_owned(),
        ),
        parental_apps::Stop::Downloaded(name) => (
            tr("Program blocked").to_owned(),
            tr("«{}» cannot run on this account: downloaded programs are not allowed.")
                .replacen("{}", name, 1),
        ),
    };
    let bus = format!("DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/{uid}/bus");
    let _ = Command::new("runuser")
        .args([
            "-u",
            user,
            "--",
            "env",
            &bus,
            "notify-send",
            "--app-name",
            tr("Parental controls"),
        ])
        .args(["--icon", "celenn-parental-controls", &title, &body])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .and_then(|mut child| child.wait());
}
