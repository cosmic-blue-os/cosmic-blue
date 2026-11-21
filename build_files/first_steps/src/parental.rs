// cosmic-blue Parental controls: how long and when children can use the
// computer, and a family filter for the web.
//
//   - the parent's window: the child accounts (accounts without
//     administrator rights) with their screen time today, a button that
//     creates one, and for each: a daily limit (weekdays, weekend),
//     bedtime hours, the web filter and the screen time of the week;
//   - in a child's session, a small agent (autostart) that warns 10, 5 and
//     1 minutes before the session closes, with an "Ask for more time"
//     button;
//   - that button's window: a parent gives 15 minutes, 30 minutes or an hour
//     with their password, there and then.
//
// The rules are applied by a service (parental_daemon.rs: screen time,
// closing the session, the web filter) and at login (PAM). Changing them
// needs an administrator password: the app runs
// /usr/libexec/cosmic-blue-parental-helper (polkit action
// io.github.cosmic_blue_os.parental.manage, remembered a few minutes), and
// more time /usr/libexec/cosmic-blue-parental-extend (action
// io.github.cosmic_blue_os.parental.extend, asked every time, so that a
// child cannot give itself more).
//
//   cosmic-blue-parental-controls               the parent's window
//   cosmic-blue-parental-controls --user NAME   the same, on a child's page
//   cosmic-blue-parental-controls --more-time   the "more time" window
//   cosmic-blue-parental-controls --agent       the warnings (autostart)
//   cosmic-blue-parental-controls --check       image build test: every text
//                                               has its translations
//
// The look is the tours' and Updates' (main.rs, updates.rs).

use cosmic::app::{Core, Settings, Task};
use cosmic::iced::core::text::Wrapping;
use cosmic::iced::{Alignment, Length, Limits};
use cosmic::widget::{self, settings, text, toaster::Toasts};
use cosmic::{Application, ApplicationExt, Apply, Element, cosmic_theme, executor, theme};
use std::collections::BTreeSet;
use std::path::Path;

mod i18n;
mod parental_rules;
use i18n::tr;
use parental_rules::{Now, Rules, Usage};

pub(crate) const LANGUAGES: &[(&str, &str)] = &[
    ("de", include_str!("../po/parental/de.po")),
    ("es", include_str!("../po/parental/es.po")),
    ("fr", include_str!("../po/parental/fr.po")),
    ("it", include_str!("../po/parental/it.po")),
    ("pt_BR", include_str!("../po/parental/pt_BR.po")),
];
pub(crate) const TEMPLATE: &str = include_str!("../po/parental/cosmic-blue-parental-controls.pot");
pub(crate) const PO_DIR: &str = "po/parental";
pub(crate) const TEMPLATE_NAME: &str = "cosmic-blue-parental-controls.pot";

const HELPER: &str = "/usr/libexec/cosmic-blue-parental-helper";
const EXTEND: &str = "/usr/libexec/cosmic-blue-parental-extend";
const ICON: &str = "cosmic-blue-parental-controls";
/// The child's launcher (see more_time_launcher)
const MORE_TIME_ID: &str = "io.github.cosmic_blue_os.ParentalControls.MoreTime";
/// Banner of the tours (branding/tour/build.py), behind the app icon
const BANNER: &str = "/usr/share/cosmic-blue/tour-banner.svg";
const AVATARS: &str = "/var/lib/AccountsService/icons";
/// Warnings before the session closes, in seconds
const WARNINGS: [u64; 3] = [600, 300, 60];
/// Steps of the - and + buttons, in minutes (any value can be typed)
const STEP: u32 = 15;
/// Bounds of a daily screen time, in minutes
const LIMIT_MIN: u32 = 5;
const LIMIT_MAX: u32 = 12 * 60;

/// A time setting of a child account: a daily screen time or a bedtime end
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Weekdays,
    Weekend,
    BedtimeStart,
    BedtimeEnd,
}

impl Field {
    fn is_clock(self) -> bool {
        matches!(self, Field::BedtimeStart | Field::BedtimeEnd)
    }

    fn get(self, rules: &Rules) -> u32 {
        match self {
            Field::Weekdays => rules.weekdays,
            Field::Weekend => rules.weekend,
            Field::BedtimeStart => rules.bedtime_start,
            Field::BedtimeEnd => rules.bedtime_end,
        }
    }

    fn set(self, rules: &mut Rules, value: u32) {
        match self {
            Field::Weekdays => rules.weekdays = value,
            Field::Weekend => rules.weekend = value,
            Field::BedtimeStart => rules.bedtime_start = value,
            Field::BedtimeEnd => rules.bedtime_end = value,
        }
    }

    fn show(self, value: u32) -> String {
        if self.is_clock() {
            parental_rules::clock(value)
        } else {
            duration(u64::from(value) * 60)
        }
    }

    /// The value one step up or down: the next multiple of the step, so that
    /// a typed 50 min goes to 45 or 1 h; times of day go round midnight
    fn step(self, value: u32, up: bool) -> u32 {
        let next = if up {
            value / STEP * STEP + STEP
        } else {
            value.div_ceil(STEP) * STEP - STEP.min(value.div_ceil(STEP) * STEP)
        };
        if self.is_clock() {
            if !up && value == 0 { 24 * 60 - STEP } else { next % (24 * 60) }
        } else {
            next.clamp(LIMIT_MIN, LIMIT_MAX)
        }
    }

    /// What was typed: a duration (45, 45 min, 1 h, 1h20, 1:20) or a time
    /// of day (20:45, 20h45, 20.45, 2045, 20), whatever the language
    fn parse(self, typed: &str) -> Option<u32> {
        let typed = typed.trim().to_lowercase();
        // Groups of digits, of letters, and separators, in order
        let mut tokens: Vec<String> = Vec::new();
        for c in typed.chars() {
            let kind = |c: char| (c.is_ascii_digit(), c.is_alphabetic());
            match tokens.last_mut() {
                Some(last) if !c.is_whitespace() && last.chars().next().map(kind) == Some(kind(c)) && (c.is_ascii_digit() || c.is_alphabetic()) => last.push(c),
                _ if c.is_whitespace() => {}
                _ => tokens.push(c.to_string()),
            }
        }
        let mut numbers: Vec<String> = Vec::new();
        let mut hours_mark = false;
        for token in &tokens {
            let first = token.chars().next()?;
            if first.is_ascii_digit() {
                numbers.push(token.clone());
            } else if numbers.len() == 1 && (matches!(first, ':' | '.' | ',' | 'h' | 's' | 'o')) {
                // 1:20, 1.20, h, hours, heures, Std, Stunden, ore
                hours_mark = true;
            } else if !first.is_alphabetic() || !matches!(first, 'h' | 's' | 'o' | 'm') {
                return None;
            }
        }
        let values: Vec<u32> = numbers.iter().map(|n| n.parse().ok()).collect::<Option<_>>()?;
        if self.is_clock() {
            let (h, m) = match values.as_slice() {
                [n] if numbers[0].len() >= 3 => (n / 100, n % 100),
                [h] => (*h, 0),
                [h, m] => (*h, *m),
                _ => return None,
            };
            (h <= 24 && m < 60 && h * 60 + m <= 24 * 60).then_some((h * 60 + m) % (24 * 60))
        } else {
            let minutes = match values.as_slice() {
                [m] if !hours_mark => *m,
                [h] => h.checked_mul(60)?,
                [h, m] if hours_mark && *m < 60 => h.checked_mul(60)? + m,
                _ => return None,
            };
            (minutes > 0).then_some(minutes.clamp(LIMIT_MIN, LIMIT_MAX))
        }
    }
}

/// Every text, for the translation check
const TEXTS: &[&str] = &[
    "Parental controls",
    "Choose how long and when your children can use this computer.",
    "Child accounts",
    "Other accounts",
    "Administrator · no limits",
    "No limits",
    "Today: {} of {}",
    "Today: {}",
    "Add a child account",
    "An account without administrator rights, ready in one click",
    "Add…",
    "Only a parent (an administrator account) can change these settings.",
    "Back",
    "Screen time",
    "Limit screen time",
    "Only time in front of the screen counts. Warnings 10, 5 and 1 minutes before the end.",
    "Monday to Friday",
    "Saturday and Sunday",
    "Bedtime",
    "No computer at night",
    "The session closes and logging in is refused during these hours.",
    "From",
    "To",
    "Internet",
    "Block unsuitable websites",
    "Adult content, gambling and malicious sites, in every browser. Safe search is forced. Uses the CleanBrowsing family filter.",
    "This week",
    "Average: {} a day",
    "Mon",
    "Tue",
    "Wed",
    "Thu",
    "Fri",
    "Sat",
    "Sun",
    "{h} h {m} min",
    "{h} h",
    "{m} min",
    "Name",
    "Password",
    "The child logs in with this name and password.",
    "Create",
    "Cancel",
    "Something went wrong, nothing was changed.",
    "Recommended",
    "More time for {}",
    "A parent confirms with their password.",
    "+15 min",
    "+30 min",
    "+1 h",
    "No limit today",
    "Give",
    "{} minutes left",
    "1 minute left",
    "Your session closes at {}. Save your work.",
    "Ask for more time",
    "It is bedtime.",
    "Screen time is over for today.",
    "You can log in again at {}.",
    "You can log in again tomorrow.",
    "You can log in again tomorrow at {}.",
    "Type a duration, for example 45 or 1:20.",
    "Type a time, for example 20:45.",
    "Parental controls do not allow logging in now. Ask a parent.",
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--check") {
        std::process::exit(if i18n::check(TEXTS) { 0 } else { 1 });
    }
    if args.iter().any(|arg| arg == "--agent") {
        agent();
        return Ok(());
    }
    let mode = if args.iter().any(|arg| arg == "--more-time") {
        Mode::MoreTime
    } else {
        Mode::Parent(
            args.iter()
                .position(|arg| arg == "--user")
                .and_then(|i| args.get(i + 1).cloned()),
        )
    };
    let size = match mode {
        Mode::MoreTime => cosmic::iced::Size::new(480.0, 380.0),
        Mode::Parent(_) => cosmic::iced::Size::new(760.0, 820.0),
    };
    let settings = Settings::default()
        .size(size)
        .size_limits(Limits::NONE.min_width(440.0).min_height(340.0));
    cosmic::app::run::<App>(settings, mode)?;
    Ok(())
}

fn user_name() -> String {
    std::env::var("USER").unwrap_or_default()
}

/// "1 h 20 min", "2 h", "25 min"
fn duration(seconds: u64) -> String {
    let minutes = seconds / 60;
    let (h, m) = (minutes / 60, minutes % 60);
    if h == 0 {
        tr("{m} min").replace("{m}", &m.to_string())
    } else if m == 0 {
        tr("{h} h").replace("{h}", &h.to_string())
    } else {
        tr("{h} h {m} min")
            .replace("{h}", &h.to_string())
            .replace("{m}", &format!("{m:02}"))
    }
}

/// A translated text with its {} replaced
fn fill(text: &str, value: &str) -> String {
    text.replacen("{}", value, 1)
}

// ---------------------------------------------------------------------------
// The agent: warnings in a child's session
// ---------------------------------------------------------------------------

/// Runs for the whole session of an account with rules: warns 10, 5 and 1
/// minutes before the session closes (the service closes it). Granting more
/// time makes the warnings come again before the new end.
fn agent() {
    let user = user_name();
    // Administrators cannot have rules (see cosmic-blue-parental-helper)
    let groups = std::process::Command::new("id")
        .arg("-nG")
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default();
    if groups.split_whitespace().any(|group| group == "wheel") {
        more_time_launcher(false);
        return;
    }
    let mut warned: BTreeSet<u64> = BTreeSet::new();
    let mut launcher = None;
    loop {
        // Rules can be added or removed while the session runs
        let rules = Rules::load(&user);
        let limited = rules.as_ref().is_some_and(|rules| rules.limit_enabled);
        if launcher != Some(limited) {
            more_time_launcher(limited);
            launcher = Some(limited);
        }
        if let (Some(rules), Some(now)) = (rules, Now::read()) {
            let usage = Usage::load(&user);
            let extra = parental_rules::load_extra(&user);
            let verdict = parental_rules::verdict(&rules, usage.used_today(&now), &extra, &now);
            if let Some(remaining) = verdict.remaining.filter(|r| *r > 0) {
                // More time given: warn again before the new end
                warned.retain(|threshold| remaining <= threshold + 30);
                let due: Vec<u64> = WARNINGS
                    .iter()
                    .copied()
                    .filter(|threshold| remaining <= *threshold && !warned.contains(threshold))
                    .collect();
                if !due.is_empty() {
                    warned.extend(due);
                    let end = (now.minute + remaining.div_ceil(60) as u32) % (24 * 60);
                    notify(remaining, &parental_rules::clock(end));
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(10));
    }
}

/// The child's "Ask for more time" launcher, in its applications: present
/// while its screen time is limited, so that it can ask at any time, not only
/// from a warning
fn more_time_launcher(show: bool) {
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let path = Path::new(&home).join(format!(".local/share/applications/{MORE_TIME_ID}.desktop"));
    if !show {
        let _ = std::fs::remove_file(&path);
        return;
    }
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName={}\nComment={}\nIcon={ICON}\nExec=/usr/libexec/cosmic-blue-parental-controls --more-time\nCategories=Utility;\nTerminal=false\n",
        tr("Ask for more time"),
        tr("A parent confirms with their password."),
    );
    if std::fs::read_to_string(&path).ok().as_deref() == Some(entry.as_str()) {
        return;
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(error) = std::fs::write(&path, entry) {
        eprintln!("{}: {error}", path.display());
    }
}

/// A notification with an "Ask for more time" button, which opens the
/// window where a parent gives it
fn notify(remaining: u64, end: &str) {
    let minutes = remaining.div_ceil(60);
    let title = if minutes <= 1 {
        tr("1 minute left").to_owned()
    } else {
        fill(tr("{} minutes left"), &minutes.to_string())
    };
    let body = fill(tr("Your session closes at {}. Save your work."), end);
    let action = format!("more={}", tr("Ask for more time"));
    // The notification shown, if any: the next warning replaces it in place
    // (no pile of warnings), and its notify-send, which waits for a click on
    // the old one, is stopped
    let (id, waiting) = {
        let shown = SHOWN.lock().unwrap_or_else(|e| e.into_inner());
        *shown
    };
    if let Some(pid) = waiting {
        let _ = std::process::Command::new("kill").arg(pid.to_string()).status();
    }
    let replace = id.map(|id| id.to_string());
    std::thread::spawn(move || {
        use std::io::BufRead;
        let mut args = vec![
            "--app-name",
            tr("Parental controls"),
            "--icon",
            ICON,
            "--urgency",
            "critical",
            // Until the child closes it: without a time, the COSMIC
            // notifications take 3 seconds, even for a critical one
            "--expire-time",
            "0",
            "--action",
            &action,
            "--print-id",
            "--wait",
        ];
        if let Some(replace) = &replace {
            args.extend(["--replace-id", replace]);
        }
        args.extend([title.as_str(), body.as_str()]);
        let child = std::process::Command::new("notify-send")
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(child) => child,
            Err(error) => {
                eprintln!("notify-send: {error}");
                return;
            }
        };
        let pid = child.id();
        // First line: the notification's id; then the action clicked
        let mut lines = std::io::BufReader::new(child.stdout.take().expect("piped")).lines();
        let id = lines.next().and_then(Result::ok).and_then(|line| line.trim().parse().ok());
        *SHOWN.lock().unwrap_or_else(|e| e.into_inner()) = (id, Some(pid));
        let clicked = lines.next().and_then(Result::ok).unwrap_or_default();
        let _ = child.wait();
        {
            let mut shown = SHOWN.lock().unwrap_or_else(|e| e.into_inner());
            if shown.1 == Some(pid) {
                shown.1 = None;
            }
        }
        if clicked.trim() == "more" {
            if let Err(error) = std::process::Command::new("/proc/self/exe")
                .arg("--more-time")
                .spawn()
                .or_else(|_| {
                    std::process::Command::new("/usr/libexec/cosmic-blue-parental-controls")
                        .arg("--more-time")
                        .spawn()
                })
            {
                eprintln!("Could not open the more time window: {error}");
            }
        }
    });
}

/// The warning notification shown: its id, and the notify-send waiting on it
static SHOWN: std::sync::Mutex<(Option<u32>, Option<u32>)> = std::sync::Mutex::new((None, None));

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Account {
    user: String,
    /// The full name, or the user name
    name: String,
    admin: bool,
    rules: Option<Rules>,
    usage: Usage,
    extra: Vec<(i64, u32)>,
    avatar: Option<widget::image::Handle>,
}

/// The people's accounts (user ids 1000 and up, with a login shell):
/// administrators (group wheel) first, then the others
fn accounts() -> Vec<Account> {
    let getent = |args: &[&str]| {
        std::process::Command::new("getent")
            .args(args)
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .unwrap_or_default()
    };
    let wheel: BTreeSet<String> = getent(&["group", "wheel"])
        .split(':')
        .nth(3)
        .unwrap_or_default()
        .trim()
        .split(',')
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect();
    let mut accounts: Vec<Account> = getent(&["passwd"])
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split(':').collect();
            let (user, uid, gecos, shell) = (fields.first()?, fields.get(2)?, fields.get(4)?, fields.get(6)?);
            let uid: u32 = uid.parse().ok()?;
            if !(1000..60000).contains(&uid) || shell.ends_with("nologin") || shell.ends_with("false") {
                return None;
            }
            let name = gecos.split(',').next().unwrap_or_default().trim();
            let avatar = Path::new(AVATARS).join(user);
            Some(Account {
                user: (*user).to_owned(),
                name: if name.is_empty() { (*user).to_owned() } else { name.to_owned() },
                admin: wheel.contains(*user),
                rules: Rules::load(user),
                usage: Usage::load(user),
                extra: parental_rules::load_extra(user),
                avatar: avatar.exists().then(|| widget::image::Handle::from_path(avatar)),
            })
        })
        .collect();
    accounts.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    accounts
}

/// A user name from a person's name: "Léa Martin" -> "lea-martin"
fn user_from_name(name: &str, taken: &[String]) -> String {
    let mut base = String::new();
    for c in name.to_lowercase().chars() {
        let c = match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ñ' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ý' | 'ÿ' => 'y',
            'ß' => 's',
            c if c.is_ascii_alphanumeric() => c,
            _ => '-',
        };
        if c != '-' || !base.ends_with('-') {
            base.push(c);
        }
    }
    let mut base = base.trim_matches('-').to_owned();
    base.truncate(24);
    if !base.starts_with(|c: char| c.is_ascii_lowercase()) {
        base = format!("child{base}");
    }
    let base = base.trim_end_matches('-').to_owned();
    let mut user = base.clone();
    let mut n = 2;
    while taken.contains(&user) || std::process::Command::new("getent")
        .args(["passwd", &user])
        .status()
        .is_ok_and(|status| status.success())
    {
        user = format!("{base}{n}");
        n += 1;
    }
    user
}

// ---------------------------------------------------------------------------
// The windows
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Mode {
    /// The parent's window, maybe on a child's page
    Parent(Option<String>),
    /// The "more time" window, in a child's session
    MoreTime,
}

#[derive(Clone, Debug, PartialEq)]
enum Page {
    Home,
    Child(String),
    Add,
}

#[derive(Clone, Debug)]
enum Message {
    Accounts(Vec<Account>),
    Open(String),
    Back,
    ShowAdd,
    /// A change to the rules of the account shown
    Rules(Rules),
    /// End of saving the rules: whether it worked (None: cancelled)
    Saved(Option<bool>),
    Name(String),
    Password(String),
    TogglePassword,
    Create,
    Created(String, Option<bool>),
    Minutes(u32),
    Give,
    Given(Option<bool>),
    Close,
    CloseToast(widget::ToastId),
    /// - or + next to a time setting
    Step(Field, bool),
    /// Typing in a time setting, then done with it (Enter or elsewhere)
    Edit(Field, String),
    Commit,
}

struct App {
    core: Core,
    mode: Mode,
    page: Page,
    accounts: Vec<Account>,
    /// The rules being saved, and the next ones to save once that is done
    saving: bool,
    pending: Option<(String, Rules)>,
    name: String,
    password: String,
    password_hidden: bool,
    busy: bool,
    minutes: u32,
    /// The time setting being typed in, and what is typed
    editing: Option<(Field, String)>,
    toasts: Toasts<Message>,
    banner: Option<widget::svg::Handle>,
}

/// Runs pkexec in the background, with `input` on its standard input:
/// Some(true) when it worked, None when the password prompt was dismissed
/// (126) or the password was wrong three times (127)
async fn pkexec(args: Vec<String>, input: String) -> Option<bool> {
    use tokio::io::AsyncWriteExt;
    let child = tokio::process::Command::new("pkexec")
        .args(&args)
        .stdin(std::process::Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(error) => {
            eprintln!("Could not start pkexec: {error}");
            return Some(false);
        }
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input.as_bytes()).await;
    }
    match child.wait().await {
        Ok(status) if status.success() => Some(true),
        Ok(status) if matches!(status.code(), Some(126 | 127)) => None,
        Ok(status) => {
            eprintln!("pkexec {args:?}: {status}");
            Some(false)
        }
        Err(error) => {
            eprintln!("pkexec: {error}");
            Some(false)
        }
    }
}

fn load() -> Task<Message> {
    Task::perform(
        async {
            tokio::task::spawn_blocking(accounts)
                .await
                .unwrap_or_default()
        },
        |accounts| cosmic::Action::App(Message::Accounts(accounts)),
    )
}

fn save(user: String, rules: Rules) -> Task<Message> {
    let args = vec![HELPER.to_owned(), "set".to_owned(), user];
    Task::perform(pkexec(args, rules.to_json()), |result| {
        cosmic::Action::App(Message::Saved(result))
    })
}

impl App {
    fn toast(&mut self, message: &'static str) -> Task<Message> {
        self.toasts
            .push(widget::toaster::Toast::new(tr(message)))
            .map(cosmic::Action::App)
    }

    /// The rules of the child account shown
    fn shown_rules(&self) -> Option<Rules> {
        let Page::Child(user) = &self.page else {
            return None;
        };
        Some(self.account(user)?.rules.clone().unwrap_or_else(Rules::none))
    }

    fn account(&self, user: &str) -> Option<&Account> {
        self.accounts.iter().find(|account| account.user == user)
    }
}

impl Application for App {
    type Executor = executor::Default;
    type Flags = Mode;
    type Message = Message;
    const APP_ID: &'static str = "io.github.cosmic_blue_os.ParentalControls";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, mode: Mode) -> (Self, Task<Message>) {
        let page = match &mode {
            Mode::Parent(Some(user)) => Page::Child(user.clone()),
            _ => Page::Home,
        };
        let mut app = App {
            core,
            mode,
            page,
            accounts: Vec::new(),
            saving: false,
            pending: None,
            name: String::new(),
            password: String::new(),
            password_hidden: true,
            busy: false,
            minutes: 30,
            editing: None,
            toasts: Toasts::new(Message::CloseToast),
            banner: Path::new(BANNER)
                .exists()
                .then(|| widget::svg::Handle::from_path(BANNER)),
        };
        let title = match app.core.main_window_id() {
            Some(id) => app.set_window_title(tr("Parental controls").to_owned(), id),
            None => Task::none(),
        };
        (app, Task::batch([title, load()]))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Accounts(accounts) => self.accounts = accounts,
            Message::Open(user) => self.page = Page::Child(user),
            Message::Back => {
                self.editing = None;
                self.page = Page::Home;
                return load();
            }
            Message::ShowAdd => {
                self.name.clear();
                self.password.clear();
                self.page = Page::Add;
            }
            Message::Rules(rules) => {
                let Page::Child(user) = self.page.clone() else {
                    return Task::none();
                };
                // Shown at once; saved one change at a time (the password
                // is asked once, then remembered for a few minutes)
                if let Some(account) = self.accounts.iter_mut().find(|a| a.user == user) {
                    account.rules = Some(rules.clone());
                }
                if self.saving {
                    self.pending = Some((user, rules));
                } else {
                    self.saving = true;
                    return save(user, rules);
                }
            }
            Message::Saved(result) => {
                self.saving = false;
                if result == Some(true) {
                    if let Some((user, rules)) = self.pending.take() {
                        self.saving = true;
                        return save(user, rules);
                    }
                    return Task::none();
                }
                // Cancelled or failed: show what is saved
                self.pending = None;
                let toast = if result == Some(false) {
                    self.toast("Something went wrong, nothing was changed.")
                } else {
                    Task::none()
                };
                return Task::batch([toast, load()]);
            }
            Message::Name(name) => self.name = name,
            Message::Password(password) => self.password = password,
            Message::TogglePassword => self.password_hidden = !self.password_hidden,
            Message::Create => {
                let name = self.name.trim().to_owned();
                if name.is_empty() || self.password.is_empty() || self.busy {
                    return Task::none();
                }
                let taken: Vec<String> = self.accounts.iter().map(|a| a.user.clone()).collect();
                let user = user_from_name(&name, &taken);
                self.busy = true;
                let args = vec![HELPER.to_owned(), "add-child".to_owned(), user.clone(), name];
                let input = format!("{}\n{}", self.password, Rules::default().to_json());
                return Task::perform(pkexec(args, input), move |result| {
                    cosmic::Action::App(Message::Created(user.clone(), result))
                });
            }
            Message::Created(user, result) => {
                self.busy = false;
                match result {
                    Some(true) => {
                        self.password.clear();
                        self.page = Page::Child(user);
                        return load();
                    }
                    Some(false) => return self.toast("Something went wrong, nothing was changed."),
                    None => {}
                }
            }
            Message::Minutes(minutes) => self.minutes = minutes,
            Message::Give => {
                self.busy = true;
                // 0: no limit for the rest of the day
                let amount = if self.minutes == 0 { "day".to_owned() } else { self.minutes.to_string() };
                let args = vec![EXTEND.to_owned(), user_name(), amount];
                return Task::perform(pkexec(args, String::new()), |result| {
                    cosmic::Action::App(Message::Given(result))
                });
            }
            Message::Given(result) => {
                self.busy = false;
                match result {
                    Some(true) => std::process::exit(0),
                    Some(false) => return self.toast("Something went wrong, nothing was changed."),
                    None => {}
                }
            }
            Message::Step(field, up) => {
                self.editing = None;
                if let Some(mut rules) = self.shown_rules() {
                    let value = field.step(field.get(&rules), up);
                    field.set(&mut rules, value);
                    return self.update(Message::Rules(rules));
                }
            }
            Message::Edit(field, typed) => self.editing = Some((field, typed)),
            Message::Commit => {
                let Some((field, typed)) = self.editing.take() else {
                    return Task::none();
                };
                let Some(mut rules) = self.shown_rules() else {
                    return Task::none();
                };
                match field.parse(&typed) {
                    Some(value) if value == field.get(&rules) => {}
                    Some(value) => {
                        field.set(&mut rules, value);
                        return self.update(Message::Rules(rules));
                    }
                    None => {
                        return self.toast(if field.is_clock() {
                            "Type a time, for example 20:45."
                        } else {
                            "Type a duration, for example 45 or 1:20."
                        });
                    }
                }
            }
            Message::Close => std::process::exit(0),
            Message::CloseToast(id) => self.toasts.remove(id),
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let page = match (&self.mode, &self.page) {
            (Mode::MoreTime, _) => self.more_time(),
            (Mode::Parent(_), Page::Home) => self.home(),
            (Mode::Parent(_), Page::Child(user)) => self.child(user),
            (Mode::Parent(_), Page::Add) => self.add(),
        };
        widget::toaster(&self.toasts, page)
    }
}

/// The page around a content: banner with the app icon, title, content
fn frame<'a>(
    banner: Option<&widget::svg::Handle>,
    title: String,
    back: bool,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    let cosmic_theme::Spacing {
        space_xxs,
        space_s,
        space_l,
        ..
    } = theme::spacing();
    let icon = widget::icon::from_name(ICON).size(64);
    let header: Element<'a, Message> = match banner {
        Some(banner) => cosmic::iced::widget::stack![
            widget::svg(banner.clone()).width(600).height(100),
            widget::container(icon).center(Length::Fill),
        ]
        .width(600)
        .height(100)
        .apply(widget::container)
        .center_x(Length::Fill)
        .into(),
        None => widget::container(icon).center_x(Length::Fill).into(),
    };
    let mut page = widget::column::with_capacity(8).push(widget::space::vertical().height(space_s));
    if back {
        page = page.push(
            widget::button::text(tr("Back"))
                .leading_icon(widget::icon::from_name("go-previous-symbolic"))
                .on_press(Message::Back),
        );
    }
    page.push(header)
        .push(widget::space::vertical().height(space_xxs))
        .push(text::title2(title).center().width(Length::Fill))
        .push(widget::space::vertical().height(space_s))
        .push(content)
        .push(widget::space::vertical().height(space_l))
        .max_width(640.0)
        .width(640.0)
        .apply(widget::container)
        .center_x(Length::Fill)
        .apply(widget::scrollable)
        .into()
}

/// A row's text: a first line, and a caption under it
fn two_lines<'a>(first: impl Into<String>, caption: String) -> Element<'a, Message> {
    widget::column::with_capacity(2)
        .spacing(2)
        .push(text::body(first.into()))
        .push(text::caption(caption).wrapping(Wrapping::Word))
        .width(Length::Fill)
        .into()
}

fn avatar<'a>(account: &Account, size: u16) -> Element<'a, Message> {
    match &account.avatar {
        Some(handle) => widget::image(handle.clone())
            .width(size)
            .height(size)
            .border_radius([f32::from(size) / 2.0; 4])
            .into(),
        None => widget::icon::from_name("avatar-default-symbolic")
            .size(size)
            .into(),
    }
}

/// A rounded bar in the accent color (or a faint one)
fn bar<'a>(width: Length, height: f32, accent: bool) -> Element<'a, Message> {
    widget::container(widget::space::horizontal().width(Length::Fixed(0.0)))
        .width(width)
        .height(Length::Fixed(height))
        .class(theme::Container::custom(move |theme| {
            let cosmic = theme.cosmic();
            let color = if accent {
                cosmic.accent_color()
            } else {
                let mut color = cosmic.on_bg_color();
                color.alpha = 0.15;
                color
            };
            cosmic::iced::widget::container::Style {
                background: Some(cosmic::iced::Color::from(color).into()),
                border: cosmic::iced::Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        }))
        .into()
}

/// Today's screen time of an account, as a caption and a gauge (0 to 1)
fn today(account: &Account) -> (String, Option<f32>) {
    let Some(rules) = &account.rules else {
        return (tr("No limits").to_owned(), None);
    };
    let Some(now) = Now::read() else {
        return (String::new(), None);
    };
    let used = account.usage.used_today(&now);
    let verdict = parental_rules::verdict(rules, used, &account.extra, &now);
    match verdict.allowed_today {
        Some(allowed) => (
            tr("Today: {} of {}")
                .replacen("{}", &duration(used), 1)
                .replacen("{}", &duration(allowed), 1),
            Some(if allowed == 0 { 1.0 } else { (used as f32 / allowed as f32).min(1.0) }),
        ),
        None if rules.bedtime_enabled || rules.web_filter => {
            (fill(tr("Today: {}"), &duration(used)), None)
        }
        None => (tr("No limits").to_owned(), None),
    }
}

impl App {
    fn home(&self) -> Element<'_, Message> {
        let space_s = theme::spacing().space_s;
        let space_m = theme::spacing().space_m;
        let mut content = widget::column::with_capacity(8)
            .spacing(space_m)
            .align_x(Alignment::Center)
            .push(
                text::body(tr("Choose how long and when your children can use this computer."))
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
            );

        let children: Vec<&Account> = self.accounts.iter().filter(|a| !a.admin).collect();
        if !children.is_empty() {
            let mut section = settings::section().title(tr("Child accounts"));
            for account in children {
                let (caption, gauge) = today(account);
                let mut lines = widget::column::with_capacity(3)
                    .spacing(2)
                    .push(text::body(account.name.clone()))
                    .push(text::caption(caption))
                    .width(Length::Fill);
                if let Some(gauge) = gauge {
                    lines = lines
                        .push(widget::space::vertical().height(4))
                        .push(widget::progress_bar::determinate_linear(gauge).width(Length::Fill).girth(6));
                }
                section = section.add(
                    widget::button::custom(
                        settings::item_row(vec![
                            avatar(account, 40),
                            lines.into(),
                            widget::icon::from_name("go-next-symbolic").size(16).into(),
                        ])
                        .spacing(space_s),
                    )
                    .class(theme::Button::MenuItem)
                    .padding(0)
                    .width(Length::Fill)
                    .on_press(Message::Open(account.user.clone())),
                );
            }
            content = content.push(section);
        }

        content = content.push(
            settings::section().add(
                settings::item_row(vec![
                    two_lines(
                        tr("Add a child account"),
                        tr("An account without administrator rights, ready in one click").to_owned(),
                    ),
                    widget::button::suggested(tr("Add…"))
                        .on_press(Message::ShowAdd)
                        .into(),
                ])
                .spacing(space_s),
            ),
        );

        let admins: Vec<&Account> = self.accounts.iter().filter(|a| a.admin).collect();
        if !admins.is_empty() {
            let mut section = settings::section().title(tr("Other accounts"));
            for account in admins {
                section = section.add(
                    settings::item_row(vec![
                        avatar(account, 40),
                        two_lines(account.name.clone(), tr("Administrator · no limits").to_owned()),
                    ])
                    .spacing(space_s),
                );
            }
            content = content.push(section);
        }
        content = content.push(
            text::caption(tr("Only a parent (an administrator account) can change these settings."))
                .align_x(Alignment::Center)
                .width(Length::Fill),
        );
        frame(self.banner.as_ref(), tr("Parental controls").to_owned(), false, content.into())
    }

    fn child(&self, user: &str) -> Element<'_, Message> {
        let space_s = theme::spacing().space_s;
        let space_m = theme::spacing().space_m;
        let Some(account) = self.account(user) else {
            return frame(self.banner.as_ref(), user.to_owned(), true, widget::column::with_capacity(0).into());
        };
        let rules = account.rules.clone().unwrap_or_else(Rules::none);
        let toggle = |on: bool, change: fn(&mut Rules, bool)| {
            let rules = rules.clone();
            widget::toggler(on).on_toggle(move |value| {
                let mut rules = rules.clone();
                change(&mut rules, value);
                Message::Rules(rules)
            })
        };
        // - and + by 15 minutes, and the value itself can be typed in
        let minutes_button = |field: Field| -> Element<'_, Message> {
            let shown = match &self.editing {
                Some((editing, typed)) if *editing == field => typed.clone(),
                _ => field.show(field.get(&rules)),
            };
            widget::row::with_capacity(3)
                .push(widget::button::icon(widget::icon::from_name("list-remove-symbolic")).on_press(Message::Step(field, false)))
                .push(
                    // A visible field, so that it shows it can be typed in
                    widget::text_input("", shown)
                        .select_on_focus(true)
                        .on_input(move |typed| Message::Edit(field, typed))
                        .on_submit(|_| Message::Commit)
                        .on_unfocus(Message::Commit)
                        .width(Length::Fixed(96.0)),
                )
                .push(widget::button::icon(widget::icon::from_name("list-add-symbolic")).on_press(Message::Step(field, true)))
                .align_y(Alignment::Center)
                .into()
        };

        let mut screen = settings::section().title(tr("Screen time")).add(
            settings::item_row(vec![
                two_lines(
                    tr("Limit screen time"),
                    tr("Only time in front of the screen counts. Warnings 10, 5 and 1 minutes before the end.").to_owned(),
                ),
                toggle(rules.limit_enabled, |r, v| r.limit_enabled = v).into(),
            ])
            .spacing(space_s),
        );
        if rules.limit_enabled {
            screen = screen
                .add(settings::item(
                    tr("Monday to Friday"),
                    minutes_button(Field::Weekdays),
                ))
                .add(settings::item(
                    tr("Saturday and Sunday"),
                    minutes_button(Field::Weekend),
                ));
        }

        let mut bedtime = settings::section().title(tr("Bedtime")).add(
            settings::item_row(vec![
                two_lines(
                    tr("No computer at night"),
                    tr("The session closes and logging in is refused during these hours.").to_owned(),
                ),
                toggle(rules.bedtime_enabled, |r, v| r.bedtime_enabled = v).into(),
            ])
            .spacing(space_s),
        );
        if rules.bedtime_enabled {
            bedtime = bedtime
                .add(settings::item(
                    tr("From"),
                    minutes_button(Field::BedtimeStart),
                ))
                .add(settings::item(
                    tr("To"),
                    minutes_button(Field::BedtimeEnd),
                ));
        }

        let internet = settings::section().title(tr("Internet")).add(
            settings::item_row(vec![
                widget::column::with_capacity(2)
                    .spacing(2)
                    .push(
                        widget::row::with_capacity(2)
                            .spacing(space_s)
                            .align_y(Alignment::Center)
                            .push(text::body(tr("Block unsuitable websites")))
                            .push(
                                text::caption(tr("Recommended"))
                                    .apply(widget::container)
                                    .padding([1, 8])
                                    .class(theme::Container::custom(|theme| {
                                        let cosmic = theme.cosmic();
                                        cosmic::iced::widget::container::Style {
                                            background: Some(cosmic::iced::Color::from(cosmic.accent_color()).into()),
                                            text_color: Some(cosmic::iced::Color::from(cosmic.accent.on).into()),
                                            border: cosmic::iced::Border {
                                                radius: 10.0.into(),
                                                ..Default::default()
                                            },
                                            ..Default::default()
                                        }
                                    })),
                            ),
                    )
                    .push(
                        text::caption(tr("Adult content, gambling and malicious sites, in every browser. Safe search is forced. Uses the CleanBrowsing family filter."))
                            .wrapping(Wrapping::Word),
                    )
                    .width(Length::Fill)
                    .into(),
                toggle(rules.web_filter, |r, v| r.web_filter = v).into(),
            ])
            .spacing(space_s),
        );

        let content = widget::column::with_capacity(4)
            .spacing(space_m)
            .push(screen)
            .push(bedtime)
            .push(internet)
            .push(self.week(account, &rules));
        frame(self.banner.as_ref(), account.name.clone(), true, content.into())
    }

    /// The screen time of the last seven days, as bars
    fn week<'a>(&'a self, account: &Account, rules: &Rules) -> Element<'a, Message> {
        const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
        const HEIGHT: f32 = 110.0;
        let space_s = theme::spacing().space_s;
        let Some(now) = Now::read() else {
            return widget::column::with_capacity(0).into();
        };
        let mut days = account.usage.history.clone();
        days.push(if account.usage.today.date == now.date {
            account.usage.today.clone()
        } else {
            parental_rules::Day {
                date: now.date.clone(),
                weekday: now.weekday,
                seconds: 0,
            }
        });
        let skip = days.len().saturating_sub(parental_rules::HISTORY_DAYS);
        let days = &days[skip..];
        let highest = days
            .iter()
            .map(|d| d.seconds)
            .chain(rules.daily_limit(now.weekday).map(|m| u64::from(m) * 60))
            .max()
            .unwrap_or(0)
            .max(60);
        let mut row = widget::row::with_capacity(7)
            .spacing(space_s)
            .align_y(Alignment::End)
            .height(Length::Fixed(HEIGHT + 24.0));
        for day in days {
            let height = (day.seconds as f32 / highest as f32 * HEIGHT).max(2.0);
            let label = DAYS
                .get(day.weekday.saturating_sub(1) as usize)
                .map_or(String::new(), |d| tr(d).to_owned());
            row = row.push(
                widget::column::with_capacity(2)
                    .spacing(4)
                    .align_x(Alignment::Center)
                    .width(Length::Fill)
                    .push(bar(Length::Fill, height, day.seconds > 0))
                    .push(text::caption(label)),
            );
        }
        let used: Vec<u64> = days.iter().map(|d| d.seconds).collect();
        let average = used.iter().sum::<u64>() / used.len().max(1) as u64;
        settings::section()
            .title(tr("This week"))
            .add(
                widget::column::with_capacity(2)
                    .spacing(space_s)
                    .align_x(Alignment::Center)
                    .push(row)
                    .push(text::caption(fill(tr("Average: {} a day"), &duration(average))))
                    .padding([space_s, 0]),
            )
            .into()
    }

    fn add(&self) -> Element<'_, Message> {
        let space_s = theme::spacing().space_s;
        let space_m = theme::spacing().space_m;
        let ready = !self.name.trim().is_empty() && !self.password.is_empty() && !self.busy;
        let create: Element<'_, Message> = if self.busy {
            widget::progress_bar::indeterminate_circular().into()
        } else {
            widget::button::suggested(tr("Create"))
                .on_press_maybe(ready.then_some(Message::Create))
                .into()
        };
        let content = widget::column::with_capacity(4)
            .spacing(space_m)
            .push(
                settings::section()
                    .add(settings::item(
                        tr("Name"),
                        widget::text_input(tr("Name"), &self.name)
                            .on_input(Message::Name)
                            .width(Length::Fixed(280.0)),
                    ))
                    .add(settings::item(
                        tr("Password"),
                        widget::secure_input(
                            tr("Password"),
                            &self.password,
                            Some(Message::TogglePassword),
                            self.password_hidden,
                        )
                        .on_input(Message::Password)
                        .on_submit(|_| Message::Create)
                        .width(Length::Fixed(280.0)),
                    )),
            )
            .push(
                text::caption(tr("The child logs in with this name and password."))
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
            )
            .push(
                widget::row::with_capacity(2)
                    .spacing(space_s)
                    .push(widget::space::horizontal().width(Length::Fill))
                    .push(widget::button::standard(tr("Cancel")).on_press(Message::Back))
                    .push(create),
            );
        frame(self.banner.as_ref(), tr("Add a child account").to_owned(), true, content.into())
    }

    /// The child's window: a parent gives more time with their password
    fn more_time(&self) -> Element<'_, Message> {
        let cosmic_theme::Spacing {
            space_s, space_m, ..
        } = theme::spacing();
        let user = user_name();
        let name = self
            .account(&user)
            .map_or(user.clone(), |account| account.name.clone());
        let choice = |label: &'static str, minutes: u32| -> Element<'_, Message> {
            let button = if self.minutes == minutes {
                widget::button::suggested(tr(label))
            } else {
                widget::button::standard(tr(label))
            };
            button
                .width(Length::Fill)
                .on_press(Message::Minutes(minutes))
                .into()
        };
        let give: Element<'_, Message> = if self.busy {
            widget::progress_bar::indeterminate_circular().into()
        } else {
            widget::button::suggested(tr("Give")).on_press(Message::Give).into()
        };
        widget::column::with_capacity(7)
            .spacing(space_m)
            .padding(space_m)
            .align_x(Alignment::Center)
            .push(widget::icon::from_name(ICON).size(56))
            .push(text::title3(fill(tr("More time for {}"), &name)).center().width(Length::Fill))
            .push(
                text::body(tr("A parent confirms with their password."))
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
            )
            .push(
                widget::row::with_children(vec![
                    choice("+15 min", 15),
                    choice("+30 min", 30),
                    choice("+1 h", 60),
                ])
                .spacing(space_s),
            )
            .push(choice("No limit today", 0))
            .push(
                widget::row::with_capacity(3)
                    .spacing(space_s)
                    .push(widget::space::horizontal().width(Length::Fill))
                    .push(widget::button::standard(tr("Cancel")).on_press(Message::Close))
                    .push(give),
            )
            .max_width(460.0)
            .apply(widget::container)
            .center_x(Length::Fill)
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::Field;

    #[test]
    fn typed_durations() {
        for (typed, minutes) in [("45", 45), ("45 min", 45), ("1 h", 60), ("1h20", 80), ("1 h 20 min", 80), ("1:20", 80), ("2 Std", 120), ("2", 5), ("30h", 720)] {
            assert_eq!(Field::Weekdays.parse(typed), Some(minutes), "{typed}");
        }
        for typed in ["", "0", "abc", "1:75", "-5", "1 x"] {
            assert_eq!(Field::Weekdays.parse(typed), None, "{typed}");
        }
    }

    #[test]
    fn typed_times() {
        for (typed, minute) in [("20:45", 1245), ("20h45", 1245), ("20.45", 1245), ("2045", 1245), ("20", 1200), ("7", 420), ("0:00", 0), ("24:00", 0)] {
            assert_eq!(Field::BedtimeStart.parse(typed), Some(minute), "{typed}");
        }
        for typed in ["25:00", "20:60", "abc", "2475"] {
            assert_eq!(Field::BedtimeStart.parse(typed), None, "{typed}");
        }
    }

    #[test]
    fn steps() {
        assert_eq!(Field::Weekdays.step(50, true), 60);
        assert_eq!(Field::Weekdays.step(50, false), 45);
        assert_eq!(Field::Weekdays.step(60, false), 45);
        assert_eq!(Field::Weekdays.step(15, false), 5);
        assert_eq!(Field::Weekdays.step(720, true), 720);
        assert_eq!(Field::BedtimeStart.step(1245, true), 1260);
        assert_eq!(Field::BedtimeStart.step(1435, true), 0);
        assert_eq!(Field::BedtimeStart.step(0, false), 1425);
        assert_eq!(Field::BedtimeStart.step(10, false), 0);
    }
}
