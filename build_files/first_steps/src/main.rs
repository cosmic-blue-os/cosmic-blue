// Celenn OS tours, built with libcosmic so they look exactly like COSMIC's
// own first-run assistant (cosmic-initial-setup): same window, title, text,
// lists and buttons, frosted glass and accent color from the COSMIC theme.
//
//   celenn-tour             "First steps": the installed system, opened once
//                                at the first login of each new account
//                                (/usr/libexec/celenn-first-steps-autostart)
//                                and from the "First steps" launcher
//   celenn-tour --live      Welcome tour of the live ISO, with a button
//                                that starts the installer
//   celenn-tour --open FILE What a downloaded .exe, .msi, .deb or .rpm
//                                file opens: why it does not install here, and
//                                a search for the app in COSMIC Store
//   celenn-tour --shortcuts The "Keyboard shortcuts" app: COSMIC's main
//                                shortcuts on one page
//   celenn-tour --belongings "Bring your things": files, photos,
//                                bookmarks and email from the old computer
//   celenn-tour --check     Image build test: every text has its
//                                translations (no window)
//
// The first steps tour adapts to the person. Its first page asks where they
// come from (Windows, macOS or Linux: the bearings page), and those coming
// from Linux how well they know it: five short pages, or three for the
// experienced. Every change applies at once (look, apps, the computer's
// settings) and nothing is installed or changed without a click.
// No notifications: the tour stays in the app library.
//
// Text is in English, translated to the session language from the PO files
// of po/ (see LANGUAGES). The layout follows cosmic-initial-setup's
// App::view (src/main.rs of its epoch-<version> tag).

use cosmic::app::{Core, Settings, Task};
use cosmic::iced::core::text::Wrapping;
use cosmic::iced::keyboard::{self, Key, key::Named};
use cosmic::iced::{Alignment, Event, Length, Limits, Subscription, event};
use cosmic::widget::{self, settings, text, toaster::Toasts};
use cosmic::{Application, ApplicationExt, Apply, Element, cosmic_theme, executor, theme};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

mod apps;
mod devtools;
mod edition;
mod failure;
// The live session's hardware check reads the same details as Tools'
// Computer page (celenn-tools), not all of them
#[allow(dead_code)]
mod hardware;
mod i18n;
mod layout;
mod network;
mod process;
mod readiness;
mod shortcuts;
mod style;
mod system;
mod toggle;
mod tour_keys;
mod tour_pages;
mod tour_view;
mod units;
use apps::{CATEGORIES, DEVELOPMENT};
use devtools::{
    BITWARDEN, BITWARDEN_EXTENSION, DevTool, ToolKind, brew_install, brew_installed, dev_tools,
};
use failure::{Card, CardAction, CardMessage, Failure, Outcome};
use i18n::{fill, language, tr};
use process::spawn;
use shortcuts::{KEY_LABELS, SHORTCUTS, close_open_sheet};
use style::dim_caption;
use system::{Computer, Package};
use tour_keys::*;
use tour_pages::*;
use tour_view::*;

const SITE: &str = "https://celenn-os.github.io/celenn/";
const LOGO: &str = "/usr/share/icons/hicolor/scalable/apps/celenn-logo.svg";
/// Welcome page illustration of the live tour: the default wallpaper in
/// miniature
const HERO: &str = "/usr/share/celenn/tour-welcome.svg";
/// The same sky (600 x 100), behind the icon of the other pages
/// (both from branding/tour/build.py)
const BANNER: &str = "/usr/share/celenn/tour-banner.svg";
const INSTALLERS: [&str; 2] = [
    "/usr/share/applications/liveinst.desktop",
    "/usr/share/applications/anaconda.desktop",
];
const UPDATES_APP: &str = "/usr/libexec/celenn-updates";
/// This program, for the windows it opens (--phone)
const TOUR: &str = "/usr/libexec/celenn-tour";
const TOOLS: &str = "/usr/libexec/celenn-tools";
/// LocalSend (first-boot app): files between phones and computers
const LOCALSEND: &[&str] = &["flatpak", "run", "org.localsend.localsend_app"];
/// A QR code of LocalSend's download page, for the phone
const LOCALSEND_QR: &str = "/usr/share/celenn/localsend-qr.svg";

/// One shortcut per line (the Settings button next to it opens the rest)
const ACCESSIBILITY: &str = "Screen reader: Super + Alt + S\nZoom: Super + = and Super + -";

/// Translations, one gettext PO file per language in po/ (copies of
/// po/celenn-tour.pot). To add a language, add its file here: pt_BR is
/// a regional variant, also used for the other variants of its language when
/// they have no file of their own.
pub(crate) const LANGUAGES: &[(&str, &str)] = &[
    ("de", include_str!("../po/de.po")),
    ("es", include_str!("../po/es.po")),
    ("fr", include_str!("../po/fr.po")),
    ("it", include_str!("../po/it.po")),
    ("pt_BR", include_str!("../po/pt_BR.po")),
];
pub(crate) const TEMPLATE: &str = include_str!("../po/celenn-tour.pot");

/// Where the PO files are, for the messages of the translation check
pub(crate) const PO_DIR: &str = "po";
pub(crate) const TEMPLATE_NAME: &str = "celenn-tour.pot";

#[derive(Clone, Debug, PartialEq)]
enum Mode {
    FirstSteps,
    Live,
    Open(PathBuf),
    Shortcuts,
    /// "Send without a cable": files between the phone and the computer
    Phone,
    /// "Bring your things": files, photos, bookmarks and email from before
    Belongings,
}

/// Every text of the tours and windows, for the translation check
fn all_texts() -> Vec<&'static str> {
    let mut texts: Vec<&'static str> = TEXTS.to_vec();
    texts.extend(failure::TEXTS);
    texts.extend(units::TEXTS);
    texts.extend(readiness::TEXTS);
    for origin in Origin::ALL {
        // (names of operating systems are not translated)
        texts.push(origin.card().2);
    }
    texts.extend(apps::texts());
    for thing in Try::ALL {
        let (title, caption) = thing.texts();
        texts.extend([title, caption]);
    }
    texts.extend(KEY_LABELS);
    for (heading, rows) in SHORTCUTS {
        texts.push(heading);
        texts.extend(rows.iter().map(|(_, description)| *description));
    }
    let origins = [
        None,
        Some(Origin::Windows),
        Some(Origin::Mac),
        Some(Origin::Linux),
    ];
    for row in belongings_rows() {
        texts.extend([row.title, row.caption]);
        texts.extend(row.button.map(|(label, _)| label));
    }
    for (mode, origin, experienced) in origins
        .iter()
        .flat_map(|origin| {
            [
                (Mode::FirstSteps, *origin, false),
                (Mode::FirstSteps, *origin, true),
            ]
        })
        .chain([(Mode::Live, None, false)])
    {
        for page in pages(&mode, origin, experienced) {
            texts.push(page.title);
            texts.extend(page.description);
            texts.extend(page.note);
            texts.extend(page.button_after.map(|(label, _)| label));
            match page.body {
                Body::Features(rows) => {
                    texts.extend(
                        rows.into_iter()
                            .flat_map(|(title, subtitle)| [title, subtitle]),
                    );
                }
                Body::Rows(rows) | Body::Essentials(rows) => {
                    for row in rows {
                        texts.extend([row.title, row.caption]);
                        texts.extend(row.button.map(|(label, _)| label));
                    }
                }
                _ => {}
            }
        }
    }
    let mut seen = BTreeSet::new();
    texts.retain(|text| seen.insert(*text));
    texts
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--check") {
        std::process::exit(if i18n::check(&all_texts()) { 0 } else { 1 });
    }
    // The image's tests: every shortcut of the sheet is one COSMIC has
    // (its defaults and our custom shortcuts, the files given)
    if args.first().map(String::as_str) == Some("--check-shortcuts") {
        std::process::exit(if shortcuts::check(&args[1..]) { 0 } else { 1 });
    }
    // As wide as cosmic-initial-setup, a little taller, so that every
    // page fits without scrolling (the page scrolls on smaller screens)
    let tour = Settings::default()
        .size(cosmic::iced::Size::new(900.0, 720.0))
        .size_limits(Limits::NONE.max_width(900.0).max_height(720.0));
    match args.first().map(String::as_str) {
        Some("--live") => cosmic::app::run::<App<LiveTour>>(tour, Mode::Live)?,
        Some("--open") => {
            let file = PathBuf::from(args.get(1).ok_or("--open needs a file")?);
            let settings = Settings::default()
                .size(cosmic::iced::Size::new(620.0, 380.0))
                .resizable(None);
            cosmic::app::run::<App<OpenDialog>>(settings, Mode::Open(file))?;
        }
        Some("--phone") => {
            let settings = Settings::default()
                .size(cosmic::iced::Size::new(640.0, 440.0))
                .resizable(None);
            cosmic::app::run::<App<PhoneDialog>>(settings, Mode::Phone)?;
        }
        Some("--belongings") => {
            let settings = Settings::default()
                .size(cosmic::iced::Size::new(680.0, 660.0))
                .resizable(None);
            cosmic::app::run::<App<BelongingsDialog>>(settings, Mode::Belongings)?;
        }
        Some("--shortcuts") => {
            // Super + F1 opens the sheet, and closes it when it is open
            if close_open_sheet() {
                return Ok(());
            }
            let settings = Settings::default()
                .size(cosmic::iced::Size::new(1120.0, 680.0))
                .size_limits(Limits::NONE.min_width(480.0).min_height(400.0));
            cosmic::app::run::<App<ShortcutsApp>>(settings, Mode::Shortcuts)?;
        }
        _ => cosmic::app::run::<App<FirstStepsTour>>(tour, Mode::FirstSteps)?,
    }
    Ok(())
}

/// The state of the profile installation
#[derive(Clone, Debug, Default, PartialEq)]
enum Install {
    #[default]
    Idle,
    /// Flathub apps and developer tools (numbers of dev_tools()), and how
    /// many of the two installs (flatpak, Homebrew) still run
    Running {
        apps: Vec<String>,
        tools: Vec<usize>,
        pending: u8,
        /// The first install that did not work
        failed: Option<Failure>,
    },
    Done,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
enum Nvidia {
    Idle,
    Switching,
    Switched,
}

#[derive(Clone, Debug)]
enum Message {
    PageOpen(usize),
    /// Next (1) or previous (-1) page
    Turn(isize),
    /// Enter: what the suggested button at the bottom does
    Enter,
    Run(Action),
    CloseToast(widget::ToastId),
    Exit,
    Origin(Origin),
    Dark(bool),
    Wallpaper(usize),
    Avatar(usize),
    /// One bar at the bottom (true), or the top bar and the dock
    Layout(bool),
    /// Tick or untick an app: its category, its number in it
    Pick(usize, usize, bool),
    /// Open a category's list (a number of CATEGORIES), or show the tiles
    /// again (None)
    Sub(Option<usize>),
    /// New to this (false) or experienced (true): the long or short tour
    Level(bool),
    /// Install what was chosen, then turn the page
    InstallAndNext,
    InstallDone(Outcome),
    /// The card of an install that did not work
    InstallCard(CardMessage),
    /// Every few seconds: which apps are installed now
    Tick,
    /// Flatpak apps, Homebrew tools, first-boot apps still installing
    Apps(BTreeSet<String>, BTreeSet<usize>, bool),
    Computer(Computer),
    /// The live session's hardware checks
    Readiness(Vec<readiness::Check>),
    /// Copy the checks as text, to ask for help on a forum
    CopyReport,
    SwitchNvidia,
    NvidiaDone(Option<bool>),
    Battery(bool),
    BatteryDone(bool, bool),
    Try(Try),
    /// A change that did not work
    Failed,
    OpenWith(&'static [&'static str]),
}

/// Application ID of each window: the name of its launcher (.desktop file),
/// which gives the window its name and icon in the dock
trait Tour: 'static {
    const APP_ID: &'static str;
}
struct FirstStepsTour;
impl Tour for FirstStepsTour {
    const APP_ID: &'static str = "io.github.celenn_os.FirstSteps";
}
struct LiveTour;
impl Tour for LiveTour {
    const APP_ID: &'static str = "io.github.celenn_os.Welcome";
}
struct OpenDialog;
impl Tour for OpenDialog {
    const APP_ID: &'static str = "io.github.celenn_os.OpenPackage";
}
struct PhoneDialog;
impl Tour for PhoneDialog {
    const APP_ID: &'static str = "io.github.celenn_os.PhoneTransfer";
}
struct BelongingsDialog;
impl Tour for BelongingsDialog {
    const APP_ID: &'static str = "io.github.celenn_os.Belongings";
}
struct ShortcutsApp;
impl Tour for ShortcutsApp {
    const APP_ID: &'static str = "io.github.celenn_os.Shortcuts";
}

struct App<T: Tour> {
    tour: std::marker::PhantomData<T>,
    core: Core,
    mode: Mode,
    origin: Option<Origin>,
    /// The short tour (kept in ~/.config/celenn/tour-level)
    experienced: bool,
    pages: Vec<Page>,
    page_i: usize,
    toasts: Toasts<Message>,
    /// Welcome illustration, or the logo when it is missing
    welcome: Option<widget::svg::Handle>,
    banner: Option<widget::svg::Handle>,
    // Look
    dark: bool,
    /// (wallpaper, its small copy)
    wallpapers: Vec<(PathBuf, widget::image::Handle)>,
    wallpaper: Option<PathBuf>,
    avatars: Vec<(PathBuf, widget::image::Handle)>,
    avatar: Option<usize>,
    bottom_bar: bool,
    look_changed: bool,
    // Apps
    installed: BTreeSet<String>,
    preinstall: Vec<String>,
    preinstall_running: bool,
    /// The apps ticked in each category's list (numbers of
    /// Category::tools()), and the lists opened once (their recommended
    /// apps are ticked the first time)
    picks: Vec<BTreeSet<usize>>,
    opened: BTreeSet<usize>,
    /// The developer tools installed with Homebrew (numbers of dev_tools())
    brew: BTreeSet<usize>,
    /// The category whose list is shown in place of the tiles
    sub: Option<usize>,
    /// Bitwarden is installing: its Firefox extension's page opens once it
    /// is there
    bitwarden_pending: bool,
    install: Install,
    /// An install that did not work, with the apps and developer tools to
    /// try again
    install_card: Option<Card<(Vec<String>, Vec<usize>)>>,
    // Computer
    computer: Option<Computer>,
    /// Live session: None while the checks run
    readiness: Option<Vec<readiness::Check>>,
    nvidia: Nvidia,
    tried: BTreeSet<Try>,
    tiling: bool,
}

impl<T: Tour> App<T> {
    fn rebuild_pages(&mut self) {
        self.pages = pages(&self.mode, self.origin, self.experienced);
        self.page_i = self.page_i.min(self.pages.len().saturating_sub(1));
    }

    fn toast(&mut self, message: &'static str) -> Task<Message> {
        self.toasts
            .push(widget::toaster::Toast::new(tr(message)))
            .map(cosmic::Action::App)
    }

    /// Progress of the first-boot apps: (installed, all), while it runs
    fn preinstall_progress(&self) -> Option<(usize, usize)> {
        if !self.preinstall_running {
            return None;
        }
        let done = self
            .preinstall
            .iter()
            .filter(|app| self.installed.contains(*app))
            .count();
        (done < self.preinstall.len()).then_some((done, self.preinstall.len()))
    }

    fn tool_installed(&self, i: usize, tool: &DevTool) -> bool {
        match tool.kind {
            ToolKind::Flatpak(app) => self.installed.contains(app),
            _ => self.brew.contains(&i),
        }
    }

    /// The apps of a category that are installed (numbers of
    /// Category::tools())
    fn category_installed(&self, n: usize) -> BTreeSet<usize> {
        CATEGORIES[n]
            .tools()
            .enumerate()
            .filter(|(i, tool)| match tool.kind {
                ToolKind::Flatpak(app) => self.installed.contains(app),
                _ => n == DEVELOPMENT && self.brew.contains(i),
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Something to install is chosen
    fn anything_chosen(&self) -> bool {
        (0..CATEGORIES.len()).any(|n| !self.picks[n].is_subset(&self.category_installed(n)))
    }

    /// What the suggested button at the bottom of a tour page does (also
    /// Enter): the next page, installing the apps chosen on the apps page;
    /// on the last page, "Get started" closes the first steps tour
    fn next_message(&self) -> Option<Message> {
        if !matches!(self.mode, Mode::FirstSteps | Mode::Live) {
            return None;
        }
        let page = self.pages.get(self.page_i)?;
        if self.page_i + 1 < self.pages.len() {
            Some(
                if matches!(page.body, Body::Profiles | Body::Essentials(_))
                    && self.anything_chosen()
                {
                    Message::InstallAndNext
                } else {
                    Message::PageOpen(self.page_i + 1)
                },
            )
        } else {
            (self.mode == Mode::FirstSteps).then_some(Message::Exit)
        }
    }

    /// Progress of the chosen apps and tools: (installed, all)
    /// Installs Flathub apps and developer tools (numbers of dev_tools()) in
    /// the background: the flatpak and the Homebrew installs run side by side
    fn start_install(&mut self, apps: Vec<String>, tools: Vec<usize>) -> Vec<Task<Message>> {
        self.install_card = None;
        let pending = u8::from(!apps.is_empty()) + u8::from(!tools.is_empty());
        if pending == 0 {
            return Vec::new();
        }
        let mut tasks = Vec::new();
        if !apps.is_empty() {
            let mut args: Vec<String> =
                ["install", "--system", "--noninteractive", "-y", "flathub"]
                    .map(str::to_owned)
                    .to_vec();
            args.extend(apps.iter().cloned());
            tasks.push(Task::perform(
                failure::run("flatpak", args, None),
                |outcome| cosmic::Action::App(Message::InstallDone(outcome)),
            ));
        }
        if !tools.is_empty() {
            let mut formulas: Vec<&'static str> = Vec::new();
            let mut casks: Vec<String> = Vec::new();
            for tool in tools.iter().filter_map(|i| dev_tools().nth(*i)) {
                match tool.kind {
                    ToolKind::Formula(names) => formulas.extend(names),
                    ToolKind::Cask(name, tap) => casks.push(devtools::cask_name(name, tap)),
                    ToolKind::Flatpak(_) => {}
                }
            }
            tasks.push(Task::perform(
                async move {
                    Some(
                        brew_install(formulas, casks)
                            .await
                            .map_err(|error| failure::failure(&error)),
                    )
                },
                |outcome| cosmic::Action::App(Message::InstallDone(outcome)),
            ));
        }
        self.install = Install::Running {
            apps,
            tools,
            pending,
            failed: None,
        };
        tasks
    }

    fn install_count(&self) -> Option<(usize, usize)> {
        let Install::Running { apps, tools, .. } = &self.install else {
            return None;
        };
        let tools_done = tools
            .iter()
            .filter(|i| {
                dev_tools()
                    .nth(**i)
                    .is_some_and(|tool| self.tool_installed(**i, tool))
            })
            .count();
        let apps_done = apps
            .iter()
            .filter(|app| self.installed.contains(*app))
            .count();
        Some((apps_done + tools_done, apps.len() + tools.len()))
    }
}

/// Runs a blocking function off the window's thread
fn blocking<R: Send + 'static>(
    function: impl FnOnce() -> R + Send + 'static,
    message: impl FnOnce(R) -> Message + Send + 'static,
) -> Task<Message> {
    Task::perform(
        async move { tokio::task::spawn_blocking(function).await },
        move |result| match result {
            Ok(value) => cosmic::Action::App(message(value)),
            Err(_) => cosmic::Action::App(Message::Failed),
        },
    )
}

fn check_apps() -> Task<Message> {
    blocking(
        || {
            (
                system::installed_apps(),
                brew_installed(),
                system::preinstall_running(),
            )
        },
        |(apps, brew, running)| Message::Apps(apps, brew, running),
    )
}

impl<T: Tour> Application for App<T> {
    type Executor = executor::Default;
    type Flags = Mode;
    type Message = Message;
    const APP_ID: &'static str = T::APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(mut core: Core, mode: Mode) -> (Self, Task<Message>) {
        if mode != Mode::Shortcuts {
            core.window.show_headerbar = false;
        }
        core.window.show_close = mode == Mode::Shortcuts;
        core.window.show_maximize = false;
        core.window.show_minimize = false;

        let first_steps = mode == Mode::FirstSteps;
        let origin = system::tour_read("tour-origin")
            .as_deref()
            .and_then(Origin::from_key);
        let experienced = system::tour_read("tour-level").as_deref() == Some(EXPERIENCED);
        let (wallpapers, avatars) = if first_steps {
            (
                system::wallpapers()
                    .into_iter()
                    .map(|(wallpaper, thumb)| (wallpaper, widget::image::Handle::from_path(thumb)))
                    .collect(),
                system::avatars()
                    .into_iter()
                    .filter_map(|path| {
                        // Twice the size shown, for sharp HiDPI screens
                        let handle = style::rounded_picture(&path, 86)?;
                        Some((path, handle))
                    })
                    .collect(),
            )
        } else {
            (Vec::new(), Vec::new())
        };
        let mut app = App {
            tour: std::marker::PhantomData,
            core,
            pages: pages(&mode, origin, experienced),
            mode: mode.clone(),
            origin,
            experienced,
            page_i: 0,
            toasts: Toasts::new(Message::CloseToast),
            welcome: [HERO, LOGO]
                .into_iter()
                .find(|path| Path::new(path).exists())
                .map(widget::svg::Handle::from_path),
            banner: Path::new(BANNER)
                .exists()
                .then(|| widget::svg::Handle::from_path(BANNER)),
            dark: system::is_dark(),
            wallpapers,
            wallpaper: system::current_wallpaper(),
            avatars,
            avatar: None,
            bottom_bar: layout::bottom_bar(),
            look_changed: false,
            installed: BTreeSet::new(),
            preinstall: if first_steps {
                system::preinstall_apps()
            } else {
                Vec::new()
            },
            preinstall_running: false,
            picks: vec![BTreeSet::new(); CATEGORIES.len()],
            opened: BTreeSet::new(),
            brew: BTreeSet::new(),
            sub: None,
            bitwarden_pending: false,
            install: Install::Idle,
            install_card: None,
            computer: None,
            readiness: None,
            nvidia: Nvidia::Idle,
            tried: BTreeSet::new(),
            tiling: system::tiling(),
        };
        let title = match &mode {
            Mode::Live => tr("Welcome to Celenn OS").to_owned(),
            Mode::FirstSteps => tr("First steps").to_owned(),
            Mode::Shortcuts => tr("Keyboard shortcuts").to_owned(),
            Mode::Phone => tr("Send without a cable").to_owned(),
            Mode::Belongings => tr("Bring your things").to_owned(),
            Mode::Open(file) => fill(tr("“{}” does not install here"), &short_name(file)),
        };
        let mut tasks = vec![match app.core.main_window_id() {
            Some(id) => app.set_window_title(title, id),
            None => Task::none(),
        }];
        if first_steps {
            tasks.push(check_apps());
            tasks.push(blocking(system::computer, Message::Computer));
        }
        if matches!(mode, Mode::Open(_) | Mode::Phone) {
            tasks.push(check_apps());
        }
        if mode == Mode::Live {
            tasks.push(blocking(readiness::run, Message::Readiness));
        }
        (app, Task::batch(tasks))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PageOpen(page_i) => {
                if page_i < self.pages.len() {
                    self.page_i = page_i;
                }
            }
            Message::Turn(delta) => {
                if let Some(page_i) = self.page_i.checked_add_signed(delta) {
                    return self.update(Message::PageOpen(page_i));
                }
            }
            Message::Enter => {
                if let Some(message) = self.next_message() {
                    return self.update(message);
                }
            }
            Message::Run(Action::Try(thing)) => return self.update(Message::Try(thing)),
            Message::Run(action) => run(&action),
            Message::OpenWith(command) => {
                if self.mode == Mode::Phone {
                    spawn(command[0], &command[1..]);
                } else if let Mode::Open(file) = &self.mode {
                    let mut args: Vec<&str> = command[1..].to_vec();
                    let file = file.to_string_lossy().into_owned();
                    args.push(&file);
                    spawn(command[0], &args);
                }
                return cosmic::iced::exit();
            }
            Message::CloseToast(id) => self.toasts.remove(id),
            Message::Exit => return cosmic::iced::exit(),
            Message::Level(experienced) => {
                self.experienced = experienced;
                system::tour_write("tour-level", if experienced { EXPERIENCED } else { "" });
                self.rebuild_pages();
            }
            Message::Origin(origin) => {
                // A second click on the chosen card takes the choice back
                self.origin = (self.origin != Some(origin)).then_some(origin);
                system::tour_write("tour-origin", self.origin.map_or("", Origin::key));
                self.rebuild_pages();
            }
            Message::Dark(dark) => {
                if system::set_dark(dark) {
                    self.dark = dark;
                    self.look_changed = true;
                } else {
                    return self.toast("This did not work.");
                }
            }
            Message::Wallpaper(i) => {
                if let Some((path, _)) = self.wallpapers.get(i) {
                    if system::set_wallpaper(path) {
                        self.wallpaper = Some(path.clone());
                        self.look_changed = true;
                    } else {
                        return self.toast("This did not work.");
                    }
                }
            }
            Message::Avatar(i) => {
                if let Some((path, _)) = self.avatars.get(i) {
                    let path = path.clone();
                    self.avatar = Some(i);
                    self.look_changed = true;
                    return blocking(
                        move || system::set_avatar(&path),
                        |ok| if ok { Message::Tick } else { Message::Failed },
                    );
                }
            }
            Message::Layout(bottom) => {
                if layout::set_bottom_bar(bottom) {
                    self.bottom_bar = bottom;
                    self.look_changed = true;
                } else {
                    return self.toast("This did not work.");
                }
            }
            Message::Pick(n, i, on) => {
                if on {
                    self.picks[n].insert(i);
                } else {
                    self.picks[n].remove(&i);
                }
            }
            Message::Sub(sub) => {
                // The first time a list opens, its recommended apps are
                // ticked (unless some of its apps are already there)
                if let Some(n) = sub
                    && self.opened.insert(n)
                    && self.category_installed(n).is_empty()
                {
                    self.picks[n].extend(CATEGORIES[n].recommended());
                }
                self.sub = sub;
            }
            Message::InstallAndNext => {
                let mut apps: Vec<String> = Vec::new();
                let mut tools = Vec::new();
                for (n, category) in CATEGORIES.iter().enumerate() {
                    let installed = self.category_installed(n);
                    for (i, tool) in category.tools().enumerate() {
                        if !self.picks[n].contains(&i) || installed.contains(&i) {
                            continue;
                        }
                        match tool.kind {
                            ToolKind::Flatpak(app) => apps.push(app.to_owned()),
                            // Homebrew's: the developer tools, numbered as
                            // dev_tools()
                            ToolKind::Formula(_) | ToolKind::Cask(..) => tools.push(i),
                        }
                    }
                }
                self.bitwarden_pending |= apps.iter().any(|app| app == BITWARDEN);
                for picks in &mut self.picks {
                    picks.clear();
                }
                self.sub = None;
                let mut tasks = vec![self.update(Message::Turn(1))];
                tasks.extend(self.start_install(apps, tools));
                return Task::batch(tasks);
            }
            Message::InstallDone(outcome) => {
                if let Install::Running {
                    apps,
                    tools,
                    pending,
                    failed,
                } = &mut self.install
                {
                    *pending = pending.saturating_sub(1);
                    if let Some(Err(failure)) = outcome {
                        failed.get_or_insert(failure);
                    }
                    if *pending == 0 {
                        let retry = (std::mem::take(apps), std::mem::take(tools));
                        self.install = match failed.take() {
                            Some(failure) => {
                                self.install_card =
                                    Some(Card::new(tr("Install and continue"), failure, retry));
                                Install::Failed
                            }
                            None => Install::Done,
                        };
                    }
                }
                return check_apps();
            }
            Message::InstallCard(message) => {
                let Some(card) = &mut self.install_card else {
                    return Task::none();
                };
                match card.update(message) {
                    CardAction::None => {}
                    CardAction::Close => self.install_card = None,
                    CardAction::Retry((apps, tools)) => {
                        // What is still missing
                        let apps = apps
                            .into_iter()
                            .filter(|app| !self.installed.contains(app))
                            .collect();
                        let tools = tools
                            .into_iter()
                            .filter(|i| {
                                dev_tools()
                                    .nth(*i)
                                    .is_some_and(|tool| !self.tool_installed(*i, tool))
                            })
                            .collect();
                        return Task::batch(self.start_install(apps, tools));
                    }
                }
            }
            Message::Tick => {
                return check_apps();
            }
            Message::Apps(apps, brew, running) => {
                // Bitwarden just installed: its Firefox extension too
                if self.bitwarden_pending && apps.contains(BITWARDEN) {
                    self.bitwarden_pending = false;
                    run(&Action::Launch(BITWARDEN_EXTENSION));
                }
                self.installed = apps;
                self.brew = brew;
                self.preinstall_running = running;
            }
            Message::Computer(computer) => {
                self.computer = Some(computer);
            }
            Message::Readiness(checks) => {
                self.readiness = Some(checks);
            }
            Message::CopyReport => {
                if let Some(checks) = &self.readiness {
                    let report = readiness::report(checks, &readiness::os_version());
                    return Task::batch([
                        cosmic::iced::clipboard::write(report),
                        self.toast("Copied"),
                    ]);
                }
            }
            Message::SwitchNvidia => {
                self.nvidia = Nvidia::Switching;
                return Task::perform(
                    async {
                        match tokio::process::Command::new("pkexec")
                            .args([system::UPDATES_HELPER, "edition", "nvidia"])
                            .status()
                            .await
                        {
                            Ok(status) if status.success() => Some(true),
                            // The password prompt was dismissed
                            Ok(status) if matches!(status.code(), Some(126 | 127)) => None,
                            _ => Some(false),
                        }
                    },
                    |result| cosmic::Action::App(Message::NvidiaDone(result)),
                );
            }
            Message::NvidiaDone(result) => {
                self.nvidia = if result == Some(true) {
                    Nvidia::Switched
                } else {
                    Nvidia::Idle
                };
                if result == Some(false) {
                    return self.toast("This did not work.");
                }
            }
            Message::Battery(on) => {
                if let Some(device) = self.computer.as_ref().and_then(|c| c.battery.clone()) {
                    return blocking(
                        move || system::set_battery_limit(&device, on),
                        move |ok| Message::BatteryDone(on, ok),
                    );
                }
            }
            Message::BatteryDone(on, ok) => {
                if ok {
                    if let Some(computer) = &mut self.computer {
                        computer.battery_limit = Some(on);
                    }
                } else {
                    return self.toast("This did not work.");
                }
            }
            Message::Try(thing) => {
                match thing.program() {
                    Some(program) => spawn(program, &[]),
                    None => {
                        let on = !self.tiling;
                        if system::set_tiling(on) {
                            self.tiling = on;
                        } else {
                            return self.toast("This did not work.");
                        }
                    }
                }
                self.tried.insert(thing);
            }
            Message::Failed => return self.toast("This did not work."),
        }
        Task::none()
    }

    fn on_escape(&mut self) -> Task<Message> {
        cosmic::iced::exit()
    }

    fn subscription(&self) -> Subscription<Message> {
        // Alt+Left and Alt+Right turn the pages (also used by the CI boot
        // test); Enter does what the suggested button does, unless a
        // widget (a focused button, a text field) took it
        let keys = event::listen_with(|event, status, _window| match event {
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: Key::Named(named),
                modifiers,
                ..
            }) if modifiers.alt() => match named {
                Named::ArrowRight => Some(Message::Turn(1)),
                Named::ArrowLeft => Some(Message::Turn(-1)),
                _ => None,
            },
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: Key::Named(Named::Enter),
                modifiers,
                ..
            }) if modifiers.is_empty() && status == event::Status::Ignored => Some(Message::Enter),
            _ => None,
        });
        // While apps install, look again every few seconds
        let installing =
            self.preinstall_progress().is_some() || matches!(self.install, Install::Running { .. });
        if installing {
            Subscription::batch([
                keys,
                cosmic::iced::time::every(Duration::from_secs(3)).map(|_| Message::Tick),
            ])
        } else {
            keys
        }
    }

    fn view(&self) -> Element<'_, Message> {
        match &self.mode {
            Mode::Open(file) => return self.open_view(file),
            Mode::Shortcuts => return self.shortcuts_view(),
            Mode::Phone => return self.phone_view(),
            Mode::Belongings => return self.belongings_view(),
            _ => {}
        }
        let cosmic_theme::Spacing {
            space_xxs,
            space_s,
            space_m,
            space_l,
            space_xl,
            ..
        } = theme::spacing();
        let page = &self.pages[self.page_i];
        let last = self.page_i + 1 == self.pages.len();

        // Buttons, as in cosmic-initial-setup: a link on the left, Back and
        // the suggested action on the right
        let close_label = if self.mode == Mode::Live {
            "Keep trying"
        } else {
            "Close"
        };
        // In the first steps tour, a clearly visible "Later" button on every
        // page but the last (where "Get started" closes it): the tour stays
        // in the app library
        let close_button = if self.mode == Mode::FirstSteps {
            if last {
                Element::from(widget::space::horizontal().width(Length::Shrink))
            } else {
                Element::from(widget::button::standard(tr("Later")).on_press(Message::Exit))
            }
        } else {
            Element::from(widget::button::link(tr(close_label)).on_press(Message::Exit))
        };
        let mut button_row = widget::row::with_capacity(4)
            .spacing(space_xxs)
            .push(close_button)
            .push(widget::space::horizontal());
        if let Some(page_i) = self.page_i.checked_sub(1) {
            button_row = button_row
                .push(widget::button::standard(tr("Back")).on_press(Message::PageOpen(page_i)));
        }
        button_row = button_row.push(if !last {
            // On the apps page, Next installs what is chosen (in the
            // background: the tour goes on meanwhile)
            let label = if matches!(page.body, Body::Profiles | Body::Essentials(_))
                && self.anything_chosen()
            {
                "Install and continue"
            } else {
                "Next"
            };
            widget::button::suggested(tr(label)).on_press_maybe(self.next_message())
        } else if self.mode == Mode::Live {
            let install = widget::button::suggested(tr("Install Celenn OS"));
            if installer().is_some() {
                install.on_press(Message::Run(Action::Install))
            } else {
                install
            }
        } else {
            widget::button::suggested(tr("Get started")).on_press(Message::Exit)
        });

        let title = text::title2(tr(page.title)).center().width(Length::Fill);
        let icon = page.icon.map(|name| -> Element<'_, Message> {
            let icon = widget::icon::from_name(name).size(64);
            match &self.banner {
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
            }
        });
        // Where we are in the tour: one dot per page, the current one in
        // the accent color
        let dots = self
            .pages
            .iter()
            .enumerate()
            .fold(
                widget::row::with_capacity(self.pages.len()).spacing(space_xxs),
                |row, (i, _)| {
                    let current = i == self.page_i;
                    row.push(
                        widget::container(widget::space::horizontal().width(Length::Fixed(0.0)))
                            .width(if current { 20 } else { 8 })
                            .height(8)
                            .class(theme::Container::custom(move |theme| {
                                let cosmic = theme.cosmic();
                                let color = if current {
                                    cosmic.accent_color()
                                } else {
                                    let mut color = cosmic.on_bg_color();
                                    color.alpha = 0.25;
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
                            })),
                    )
                },
            )
            // Where we are, in words too (read by the screen reader)
            .push(widget::space::horizontal().width(Length::Fixed(space_xxs.into())))
            .push(dim_caption(
                tr("Step {} of {}")
                    .replacen("{}", &(self.page_i + 1).to_string(), 1)
                    .replacen("{}", &self.pages.len().to_string(), 1),
            ))
            .align_y(Alignment::Center)
            .apply(widget::container)
            .center_x(Length::Fill);

        let mut content = widget::column::with_capacity(6)
            .spacing(space_m)
            .align_x(Alignment::Center);
        if let Some(description) = page.description {
            content = content.push(
                text::body(tr(description))
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
            );
        }
        content = content.push(self.body(&page.body, space_s));
        if let Some((label, action)) = &page.button_after {
            content = content.push(action_button(label, action));
        }
        if let Some(note) = page.note {
            content = content.push(
                with_keys(tr(note))
                    .apply(widget::container)
                    .center_x(Length::Fill),
            );
        }
        let content = content
            .apply(widget::scrollable)
            .apply(widget::container)
            .height(Length::Fill);

        let page_view = widget::column::with_capacity(11)
            .push(widget::space::vertical().height(if icon.is_some() { space_s } else { space_xl }))
            .push_maybe(icon)
            .push_maybe(
                page.icon
                    .map(|_| widget::space::vertical().height(space_xxs)),
            )
            .push(title)
            .push(widget::space::vertical().height(if page.icon.is_some() {
                space_s
            } else {
                space_l
            }))
            .push(content)
            .push(widget::space::vertical().height(space_s))
            .push(dots)
            .push(widget::space::vertical().height(space_s))
            .push(button_row)
            .push(widget::space::vertical().height(space_l))
            .max_width(640.0)
            .width(640.0)
            .align_x(Alignment::Center)
            .apply(widget::container)
            .center_x(Length::Fill);

        widget::toaster(&self.toasts, page_view)
    }
}

fn action_button<'a>(label: &'static str, action: &Action) -> Element<'a, Message> {
    let available = match action {
        Action::Launch(command) => in_path(command[0]),
        Action::Try(thing) => thing.program().is_none_or(in_path),
        _ => true,
    };
    let button = widget::button::standard(tr(label));
    if available {
        button.on_press(Message::Run(action.clone()))
    } else {
        button
    }
    .into()
}

/// A program the system has: an absolute path, or a name in PATH
fn in_path(program: &str) -> bool {
    if program.starts_with('/') {
        return Path::new(program).is_file();
    }
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

fn installer() -> Option<&'static str> {
    INSTALLERS.into_iter().find(|path| Path::new(path).exists())
}

fn run(action: &Action) {
    match action {
        Action::Launch(command) => spawn(command[0], &command[1..]),
        Action::OpenUri(uri) => spawn("xdg-open", &[uri]),
        Action::Install => {
            if let Some(desktop) = installer() {
                spawn("gio", &["launch", desktop]);
                std::process::exit(0);
            }
        }
        // Message::Run sends them to Message::Try
        Action::Try(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::tour_keys::{Piece, pieces};

    #[test]
    fn key_combinations() {
        use Piece::{Keys, Word};
        assert_eq!(pieces("Super + A"), vec![Keys(vec!["Super", "A"])]);
        assert_eq!(
            pieces("Screen reader: Super + Alt + S"),
            vec![
                Word("Screen"),
                Word("reader:"),
                Keys(vec!["Super", "Alt", "S"])
            ]
        );
        assert_eq!(
            pieces("Tiling: Super + Y. The app"),
            vec![
                Word("Tiling:"),
                Keys(vec!["Super", "Y"]),
                Word("."),
                Word("The"),
                Word("app")
            ]
        );
        assert_eq!(
            pieces("Super + = and Super + -"),
            vec![
                Keys(vec!["Super", "="]),
                Word("and"),
                Keys(vec!["Super", "-"])
            ]
        );
        assert_eq!(
            pieces("Super + ← ↑ ↓ →"),
            vec![Keys(vec!["Super", "←", "↑", "↓", "→"])]
        );
        assert_eq!(
            pieces("Super + 1 … 9"),
            vec![Keys(vec!["Super", "1", "…", "9"])]
        );
        assert_eq!(
            pieces("Super + Ctrl + flèches"),
            vec![Keys(vec!["Super", "Ctrl"]), Word("+"), Word("flèches")]
        );
        assert_eq!(
            pieces("Press Super, type"),
            vec![Word("Press"), Word("Super,"), Word("type")]
        );
    }
}
