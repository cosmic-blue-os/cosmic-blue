// cosmic-blue tours, built with libcosmic so they look exactly like COSMIC's
// own first-run assistant (cosmic-initial-setup): same window, title, text,
// lists and buttons, frosted glass and accent color from the COSMIC theme.
//
//   cosmic-blue-tour             "First steps": the installed system, opened once
//                                at the first login of each new account
//                                (/usr/libexec/cosmic-blue-first-steps-autostart)
//                                and from the "First steps" launcher
//   cosmic-blue-tour --live      Welcome tour of the live ISO, with a button
//                                that starts the installer
//   cosmic-blue-tour --open FILE What a downloaded .exe, .msi, .deb or .rpm
//                                file opens: why it does not install here, and
//                                a search for the app in COSMIC Store
//   cosmic-blue-tour --shortcuts The "Keyboard shortcuts" app: COSMIC's main
//                                shortcuts on one page
//   cosmic-blue-tour --check     Image build test: every text has its
//                                translations (no window)
//
// The first steps tour adapts to the person. Its first page asks where they
// come from: Windows and macOS get their bearings, Linux gets the short
// version (the commands). Every change applies at once (look, apps, the
// computer's settings) and nothing is installed or changed without a click.
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
use std::process::Command;
use std::time::Duration;

mod devtools;
mod i18n;
mod system;
use devtools::{DEV_TOOLS, DevTool, ToolKind, brew_install, brew_installed, dev_tools};
use i18n::{language, tr};
use system::{Computer, Package};

const SITE: &str = "https://cosmic-blue-os.github.io/cosmic-blue/";
const ISSUES: &str = "https://github.com/cosmic-blue-os/cosmic-blue/issues";
const LOGO: &str = "/usr/share/icons/hicolor/scalable/apps/cosmic-blue-logo.svg";
/// Welcome page illustration of the live tour: the default wallpaper in
/// miniature
const HERO: &str = "/usr/share/cosmic-blue/tour-welcome.svg";
/// The same sky (600 x 100), behind the icon of the other pages
/// (both from branding/tour/build.py)
const BANNER: &str = "/usr/share/cosmic-blue/tour-banner.svg";
const INSTALLERS: [&str; 2] = [
    "/usr/share/applications/liveinst.desktop",
    "/usr/share/applications/anaconda.desktop",
];
const UPDATES_APP: &str = "/usr/libexec/cosmic-blue-updates";

/// One shortcut per line (the Settings button next to it opens the rest)
const ACCESSIBILITY: &str = "Screen reader: Super + Alt + S\nZoom: Super + = and Super + -";

/// Translations, one gettext PO file per language in po/ (copies of
/// po/cosmic-blue-tour.pot). To add a language, add its file here: pt_BR is
/// a regional variant, also used for the other variants of its language when
/// they have no file of their own.
pub(crate) const LANGUAGES: &[(&str, &str)] = &[
    ("de", include_str!("../po/de.po")),
    ("es", include_str!("../po/es.po")),
    ("fr", include_str!("../po/fr.po")),
    ("it", include_str!("../po/it.po")),
    ("pt_BR", include_str!("../po/pt_BR.po")),
];
pub(crate) const TEMPLATE: &str = include_str!("../po/cosmic-blue-tour.pot");

/// Where the PO files are, for the messages of the translation check
pub(crate) const PO_DIR: &str = "po";
pub(crate) const TEMPLATE_NAME: &str = "cosmic-blue-tour.pot";

/// Where the person comes from (first page of the first steps tour), kept
/// in ~/.config/cosmic-blue/tour-origin for the next time
#[derive(Clone, Copy, Debug, PartialEq)]
enum Origin {
    Windows,
    Mac,
    Linux,
}

impl Origin {
    const ALL: [Origin; 3] = [Origin::Windows, Origin::Mac, Origin::Linux];

    fn key(self) -> &'static str {
        match self {
            Origin::Windows => "windows",
            Origin::Mac => "macos",
            Origin::Linux => "linux",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|origin| origin.key() == key)
    }

    /// Icon, name and what the tour does for them
    fn card(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Origin::Windows => ("computer-symbolic", "Windows", "Your bearings from Windows"),
            Origin::Mac => (
                "computer-laptop-symbolic",
                "macOS",
                "Your bearings from the Mac",
            ),
            Origin::Linux => (
                "utilities-terminal-symbolic",
                "Linux",
                "What is different from your distribution",
            ),
        }
    }
}

/// Usage profiles: apps from Flathub, installed for everyone like the
/// first-boot apps (flatpak's polkit rules let administrators do it without
/// a password)
struct Profile {
    title: &'static str,
    caption: &'static str,
    apps: &'static [&'static str],
}

const PROFILES: &[Profile] = &[
    Profile {
        title: "Office",
        caption: "LibreOffice: documents, spreadsheets and presentations, Word and Excel files included",
        apps: &["org.libreoffice.LibreOffice"],
    },
    Profile {
        title: "Games",
        caption: "Steam, and Heroic Games Launcher for Epic Games and GOG",
        apps: &["com.valvesoftware.Steam", "com.heroicgameslauncher.hgl"],
    },
    Profile {
        title: "Graphics and drawing",
        caption: "GIMP for photos, Inkscape for illustrations and logos, Krita for digital painting",
        apps: &["org.gimp.GIMP", "org.inkscape.Inkscape", "org.kde.krita"],
    },
    Profile {
        title: "Videos and streaming",
        caption: "OBS Studio to record and stream, Kdenlive to edit videos, Audacity for sound",
        apps: &[
            "com.obsproject.Studio",
            "org.kde.kdenlive",
            "org.audacityteam.Audacity",
        ],
    },
];

/// Things to try on the "Try it" page: each button does the real thing
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Try {
    Launcher,
    AppLibrary,
    Workspaces,
    Tiling,
    Screenshot,
}

impl Try {
    const ALL: [Try; 5] = [
        Try::Launcher,
        Try::AppLibrary,
        Try::Workspaces,
        Try::Tiling,
        Try::Screenshot,
    ];

    fn texts(self) -> (&'static str, &'static str) {
        match self {
            Try::Launcher => (
                "Open the launcher",
                "Press Super, type the name of an app, then Enter.",
            ),
            Try::AppLibrary => ("See all your apps", "Super + A"),
            Try::Workspaces => (
                "See your workspaces",
                "Super + W: one desktop per activity. Super + Ctrl + arrows goes from one to the next.",
            ),
            Try::Tiling => (
                "Tile your windows",
                "Windows side by side, without overlap. Super + Y turns it on or off.",
            ),
            Try::Screenshot => (
                "Take a screenshot",
                "Super + Shift + S, or the Print Screen key",
            ),
        }
    }

    /// The COSMIC program behind the shortcut (cosmic-settings-daemon's
    /// system actions)
    fn program(self) -> Option<&'static str> {
        match self {
            Try::Launcher => Some("cosmic-launcher"),
            Try::AppLibrary => Some("cosmic-app-library"),
            Try::Workspaces => Some("cosmic-workspaces"),
            Try::Screenshot => Some("cosmic-screenshot"),
            Try::Tiling => None,
        }
    }
}

#[derive(Clone, Debug)]
enum Action {
    /// A program and its arguments
    Launch(&'static [&'static str]),
    OpenUri(String),
    Install,
}

/// A row: title, caption and maybe a button
struct Row {
    title: &'static str,
    caption: &'static str,
    button: Option<(&'static str, Action)>,
}

const fn row(title: &'static str, caption: &'static str) -> Row {
    Row {
        title,
        caption,
        button: None,
    }
}

fn row_button(
    title: &'static str,
    caption: &'static str,
    label: &'static str,
    action: Action,
) -> Row {
    Row {
        title,
        caption,
        button: Some((label, action)),
    }
}

/// What a page shows under its description
enum Body {
    /// The welcome illustration (HERO, or the logo when it is missing),
    /// with the accessibility row under it (live tour)
    Welcome,
    /// Where the person comes from, the accessibility row and the progress
    /// of the first-boot apps (first steps tour)
    Origin,
    /// Theme, wallpaper, avatar, dock: applied at once
    Look,
    /// Usage profiles
    Profiles,
    /// What the tour finds about the computer
    Computer,
    /// Buttons that do the real thing
    Try,
    /// What was done, backups and help
    Ready,
    /// A large icon of the icon theme, as an illustration
    Icon(&'static str),
    /// Title and subtitle rows, without action
    Features(Vec<(&'static str, &'static str)>),
    /// Title and subtitle rows, some with a button
    Rows(Vec<Row>),
}

struct Page {
    /// Icon of the icon theme shown above the title on the BANNER sky, in
    /// color like the app icons (cosmic-blue's Tela circle theme)
    icon: Option<&'static str>,
    title: &'static str,
    description: Option<&'static str>,
    body: Body,
    /// Button under the rows
    button_after: Option<(&'static str, Action)>,
    /// Short dimmed line at the bottom
    note: Option<&'static str>,
}

impl Page {
    fn new(title: &'static str, description: Option<&'static str>, body: Body) -> Self {
        Self {
            icon: None,
            title,
            description,
            body,
            button_after: None,
            note: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Mode {
    FirstSteps,
    Live,
    Open(PathBuf),
    Shortcuts,
}

/// The cosmic-blue site in the session language
fn docs_url() -> String {
    let lang = match language() {
        Some("pt_BR") => "pt-br/",
        Some("fr") => "fr/",
        Some("de") => "de/",
        Some("es") => "es/",
        Some("it") => "it/",
        _ => "",
    };
    format!("{SITE}{lang}docs/")
}

const DISTROSHELF: &[&str] = &["flatpak", "run", "com.ranfdev.DistroShelf"];
const FLATSEAL: &[&str] = &["flatpak", "run", "com.github.tchx84.Flatseal"];
const FIREFOX: &[&str] = &["flatpak", "run", "org.mozilla.firefox"];
const THUNDERBIRD: &[&str] = &["flatpak", "run", "org.mozilla.thunderbird"];
const FIRMWARE: &[&str] = &["flatpak", "run", "org.gnome.Firmware"];

fn bearings(origin: Origin) -> Page {
    let rows = match origin {
        Origin::Linux => vec![
            row_button(
                "The system is an image",
                "Signed and read-only, it updates as a whole: no dnf on the system, and the previous version is one click away.",
                "Updates",
                Action::Launch(&[UPDATES_APP]),
            ),
            row_button(
                "Apps → COSMIC Store",
                "Flatpak apps from Flathub, kept apart from the system",
                "Open COSMIC Store",
                Action::Launch(&["cosmic-store"]),
            ),
            row_button(
                "Packages from other distributions → DistroShelf",
                "Arch, Debian, Ubuntu… in a container that shares your home folder, set up in a few clicks",
                "Open DistroShelf",
                Action::Launch(DISTROSHELF),
            ),
            row_button(
                "If you like the terminal",
                "ujust lists the system's recipes, brew installs command line tools: the documentation has them all.",
                "Documentation",
                Action::OpenUri(docs_url()),
            ),
        ],
        Origin::Mac => vec![
            row(
                "Command key → Ctrl, most of the time",
                "Copy, paste, save: Ctrl + C, Ctrl + V, Ctrl + S",
            ),
            row(
                "Spotlight → the Super key",
                "Super opens the launcher: type the name of an app",
            ),
            row(
                "App Store → COSMIC Store",
                "Apps install and update from there",
            ),
            row("Finder → Files", "Super + E opens it"),
            row(
                "Mission Control → Super + W",
                "Your windows and workspaces at a glance",
            ),
            row(
                "System Settings → Settings",
                "Everything in one place, Settings → Desktop for the look",
            ),
        ],
        _ => vec![
            row(
                "Windows key → the Super key",
                "It opens the launcher, like the Start menu: type the name of an app",
            ),
            row(
                "Microsoft Store → COSMIC Store",
                "Apps install and update from there",
            ),
            row("File Explorer → Files", "Super + E opens it, as on Windows"),
            row(
                "Settings and Control Panel → Settings",
                "Everything in one place",
            ),
            row(
                "Office → LibreOffice",
                "It opens and saves Word, Excel and PowerPoint files",
            ),
            row(
                "Same as on Windows",
                "Alt + Tab, Ctrl + C and Ctrl + V, Super + Shift + S for a screenshot, Ctrl + Shift + Esc for the task manager",
            ),
        ],
    };
    Page {
        icon: Some(if origin == Origin::Linux {
            "cosmic-blue-welcome"
        } else {
            "cosmic-blue-tour-bearings"
        }),
        ..Page::new(
            match origin {
                Origin::Mac => "Your bearings from the Mac",
                Origin::Windows => "Your bearings from Windows",
                Origin::Linux => "What is different here",
            },
            Some(match origin {
                Origin::Linux => "Another Linux, with its own habits, and an app for each of them.",
                _ => "What changes, and what stays the same.",
            }),
            Body::Rows(rows),
        )
    }
}

fn pages(mode: &Mode, origin: Option<Origin>) -> Vec<Page> {
    match mode {
        Mode::FirstSteps => {
            let welcome = Page::new(
                "Welcome to cosmic-blue",
                Some(
                    "Your system is installed and ready. Where do you come from? The tour adapts to you.",
                ),
                Body::Origin,
            );
            // No banner nor description: the wallpapers and pictures need
            // the room (each choice applies at once, which says it all)
            let look = Page::new("Make it yours", None, Body::Look);
            let profiles = Page {
                icon: Some("cosmic-blue-tour-store"),
                note: Some("Thousands of other apps are in COSMIC Store."),
                ..Page::new(
                    "What do you use your computer for?",
                    Some(
                        "Pick what concerns you: nothing installs without a click, and everything can be removed later.",
                    ),
                    Body::Profiles,
                )
            };
            let computer = Page {
                icon: Some("cosmic-blue-tour-computer"),
                ..Page::new(
                    "Your computer",
                    Some("Checked for you: only what concerns this computer shows up."),
                    Body::Computer,
                )
            };
            let try_it = Page {
                icon: Some("cosmic-blue-tour-try"),
                ..Page::new(
                    "Try it, it is safe",
                    Some("Each button does the real thing."),
                    Body::Try,
                )
            };
            let belongings = Page {
                icon: Some("cosmic-blue-tour-files"),
                ..Page::new(
                    "Bring your things",
                    Some("Your files, bookmarks and email from before."),
                    Body::Rows(vec![
                        row_button(
                            "Files from an old disk",
                            "Plug it in: it shows up in Files, on the left. A Windows partition of this computer too.",
                            "Open Files",
                            Action::Launch(&["cosmic-files"]),
                        ),
                        row(
                            "Photos from a phone",
                            "Plug it in with its cable and allow file transfer on the phone: it shows up in Files.",
                        ),
                        row_button(
                            "Bookmarks and passwords",
                            "Export them from your old browser (an HTML file for bookmarks, a CSV file for passwords), then import them in Firefox.",
                            "Open Firefox",
                            Action::Launch(FIREFOX),
                        ),
                        row_button(
                            "Email",
                            "Thunderbird sets up your account with just your address and password.",
                            "Open Thunderbird",
                            Action::Launch(THUNDERBIRD),
                        ),
                    ]),
                )
            };
            let good_to_know = Page {
                icon: Some("cosmic-blue-tour-tips"),
                ..Page::new(
                    "Good to know",
                    Some("Three things that work differently here."),
                    Body::Rows(vec![
                        row_button(
                            "Updates apply at the next restart",
                            "The system updates itself in the background, without interrupting you. Something wrong after an update? Go back to the previous version in one click.",
                            "Updates",
                            Action::Launch(&[UPDATES_APP]),
                        ),
                        row_button(
                            "An app cannot see a folder or a USB drive?",
                            "Apps run apart from the system and only see what they need. Flatseal gives them access to more.",
                            "Open Flatseal",
                            Action::Launch(FLATSEAL),
                        ),
                        row_button(
                            "A downloaded .exe, .deb or .rpm file?",
                            "They do not install on cosmic-blue: look for the app in COSMIC Store, where it updates by itself.",
                            "Open COSMIC Store",
                            Action::Launch(&["cosmic-store"]),
                        ),
                    ]),
                )
            };
            let ready = Page::new(
                "You are ready",
                Some("One more thing worth doing: backups."),
                Body::Ready,
            );
            match origin {
                Some(origin) => vec![
                    welcome,
                    look,
                    profiles,
                    computer,
                    bearings(origin),
                    try_it,
                    belongings,
                    good_to_know,
                    ready,
                ],
                None => vec![
                    welcome,
                    look,
                    profiles,
                    computer,
                    try_it,
                    belongings,
                    good_to_know,
                    ready,
                ],
            }
        }
        Mode::Live => vec![
            Page::new(
                "Welcome to cosmic-blue",
                Some(
                    "cosmic-blue is Fedora Atomic with the COSMIC desktop, made in the spirit of Bluefin: a desktop that stays out of your way and keeps itself up to date.",
                ),
                Body::Welcome,
            ),
            Page::new(
                "Try it live",
                Some(
                    "cosmic-blue is running from the live media. Nothing is written to your disk, and your changes are lost when you shut down. You can start the installer at any time from the desktop or the dock.",
                ),
                Body::Icon("cosmic-blue-welcome"),
            ),
            Page::new(
                "What you get",
                None,
                Body::Features(vec![
                    (
                        "Automatic updates",
                        "The whole system updates in the background and applies at the next restart. If something goes wrong, boot the previous version.",
                    ),
                    (
                        "Apps from Flathub and COSMIC",
                        "Install apps with COSMIC Store, from Flathub and the COSMIC repository. They run sandboxed and update on their own.",
                    ),
                    (
                        "Ready for the terminal",
                        "Homebrew for command line tools, distrobox for other distributions, and ujust for common tasks.",
                    ),
                ]),
            ),
            Page::new(
                "Ready to install?",
                Some(
                    "The installer copies cosmic-blue to your disk. After that, it updates itself from the cosmic-blue image registry.",
                ),
                Body::Icon("cosmic-blue-installer"),
            ),
        ],
        Mode::Open(_) | Mode::Shortcuts => Vec::new(),
    }
}

/// The keyboard shortcuts app: (heading, [(keys, what they do)])
const SHORTCUTS: &[(&str, &[(&str, &str)])] = &[
    (
        "Desktop",
        &[
            ("Super", "Open the launcher"),
            ("Super + A", "See all your apps"),
            ("Super + W", "See your windows and workspaces"),
            ("Super + Shift + S", "Take a screenshot"),
            ("Super + Escape", "Lock the screen"),
        ],
    ),
    (
        "Windows",
        &[
            ("Alt + Tab", "Go from one window to the next"),
            ("Super + Q", "Close the window"),
            ("Super + M", "Maximize the window"),
            ("Super + Y", "Turn tiling on or off"),
            (
                "Super + ← ↑ ↓ →",
                "Go to the window on the left, right, above or below",
            ),
            (
                "Super + Shift + ← ↑ ↓ →",
                "Move the window to the left, right, up or down",
            ),
            ("Super + G", "Let a window float above the others"),
        ],
    ),
    (
        "Workspaces",
        &[
            (
                "Super + Ctrl + ← ↑ ↓ →",
                "Go to the next or previous workspace",
            ),
            ("Super + 1 … 9", "Go to a workspace"),
            ("Super + Shift + 1 … 9", "Move the window to a workspace"),
        ],
    ),
    (
        "Apps",
        &[
            ("Ctrl + Alt + T", "Terminal"),
            ("Super + E", "Files"),
            ("Ctrl + Shift + Escape", "System monitor"),
        ],
    ),
    (
        "Accessibility",
        &[
            ("Super + Alt + S", "Turn the screen reader on or off"),
            ("Super + = / Super + -", "Zoom in and out"),
        ],
    ),
];

/// Texts that are not in a page (buttons, states, the other windows)
const TEXTS: &[&str] = &[
    "Back",
    "Next",
    "Close",
    "Later",
    "First steps",
    "Open",
    "Get started",
    "Install cosmic-blue",
    "Keep trying",
    "Accessibility",
    ACCESSIBILITY,
    "Settings",
    // First page
    "Installing your apps: {} of {}",
    "They show up in the app library as they arrive.",
    // Look
    "Theme",
    "Dark",
    "Light",
    "Wallpaper",
    "Your picture",
    "Dock always visible",
    "This did not work.",
    // Profiles
    "Installed",
    "Development",
    "Docker, VS Code, Zed, Neovim…: choose your tools",
    "Chosen: {}",
    "Choose…",
    "Install and continue",
    "Installing: {} of {}",
    "Some apps did not install. Are you online? They are also in COSMIC Store.",
    "Some apps did not install",
    "Are you online? They are also in COSMIC Store.",
    "Containers",
    "Code editors",
    "In the terminal",
    "Ticked and grey: already installed. Lima and virtual machines: ujust install-dev-tools and ujust install-vms, in a terminal.",
    "Homebrew is still being set up: the code editors and terminal tools can be chosen in a few minutes.",
    "Back to the list",
    // Computer
    "Checking your computer…",
    "Connected to the internet",
    "No internet connection",
    "Apps and updates need it.",
    "Network settings",
    "NVIDIA graphics card",
    "Its driver is installed: this is the NVIDIA edition of cosmic-blue.",
    "The NVIDIA edition of cosmic-blue has its driver: about 4 GB to download, then it starts at the next restart.",
    "Switch",
    "Downloading the NVIDIA edition… This can take a while.",
    "The NVIDIA edition starts at the next restart.",
    "At the restart, a blue screen asks to enroll a key: choose Enroll MOK, Continue, Yes, then type the password cosmicblue (the keyboard is QWERTY there).",
    "Firmware updates are available",
    "Firmware is up to date",
    "Open Firmware",
    "Take care of the battery",
    "Stop charging at 80 % when the laptop stays plugged in: the battery lasts longer.",
    "Screen scale",
    "Text and windows too small or too big? Change the scale of the screen.",
    "Display settings",
    // Try
    "Try",
    "Turn on",
    "Turn off",
    // Ready
    "Your look is set",
    "Your apps are installed",
    "Protect your files",
    "Plug in a USB drive and turn on automatic backups: Déjà Dup keeps a copy of your home folder.",
    "Set up backups",
    "Documentation",
    "Updates, software, commands and tips",
    "Report a problem",
    "Found a bug or something unclear? Tell us on GitHub",
    "This tour and the Keyboard shortcuts app stay in the app library.",
    // Files that do not install here
    "“{}” does not install here",
    "This is a Windows program. On cosmic-blue, apps come from COSMIC Store: most well-known apps are there, or a good alternative.",
    "This is a package for Debian and Ubuntu. On cosmic-blue, apps come from COSMIC Store: most well-known apps are there, or a good alternative.",
    "This is a package for Fedora. cosmic-blue's system is a read-only image: apps come from COSMIC Store instead. Most well-known apps are there, or a good alternative.",
    "Search for “{}” in COSMIC Store",
    "Open COSMIC Store",
    "A Windows program with no equivalent? Bottles, in COSMIC Store, runs some of them.",
    "Open with Bottles",
    "For advanced users: distrobox installs it in a container of another distribution.",
    // Keyboard shortcuts
    "Keyboard shortcuts",
    "Super is the key with the Windows logo on most keyboards.",
    "Change them in Settings",
];

/// Every text of the tours and windows, for the translation check
fn all_texts() -> Vec<&'static str> {
    let mut texts: Vec<&'static str> = TEXTS.to_vec();
    for origin in Origin::ALL {
        // (names of operating systems are not translated)
        texts.push(origin.card().2);
    }
    for profile in PROFILES {
        texts.extend([profile.title, profile.caption]);
    }
    for thing in Try::ALL {
        let (title, caption) = thing.texts();
        texts.extend([title, caption]);
    }
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
    for (mode, origin) in origins
        .iter()
        .map(|origin| (Mode::FirstSteps, *origin))
        .chain([(Mode::Live, None)])
    {
        for page in pages(&mode, origin) {
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
                Body::Rows(rows) => {
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
        Some("--shortcuts") => {
            let settings = Settings::default()
                .size(cosmic::iced::Size::new(760.0, 720.0))
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
        failed: bool,
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
    Run(Action),
    CloseToast(widget::ToastId),
    Exit,
    Origin(Origin),
    Dark(bool),
    Wallpaper(usize),
    Avatar(usize),
    Dock(bool),
    Profile(usize, bool),
    /// Show the developer tools (true) or the profiles again
    DevOpen(bool),
    DevTool(usize, bool),
    /// Install what was chosen, then turn the page
    InstallAndNext,
    InstallDone(bool),
    /// Every few seconds: which apps are installed now
    Tick,
    /// Flatpak apps, Homebrew tools, first-boot apps still installing
    Apps(BTreeSet<String>, BTreeSet<usize>, bool),
    Computer(Computer),
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
    const APP_ID: &'static str = "io.github.cosmic_blue_os.FirstSteps";
}
struct LiveTour;
impl Tour for LiveTour {
    const APP_ID: &'static str = "io.github.cosmic_blue_os.Welcome";
}
struct OpenDialog;
impl Tour for OpenDialog {
    const APP_ID: &'static str = "io.github.cosmic_blue_os.OpenPackage";
}
struct ShortcutsApp;
impl Tour for ShortcutsApp {
    const APP_ID: &'static str = "io.github.cosmic_blue_os.Shortcuts";
}

struct App<T: Tour> {
    tour: std::marker::PhantomData<T>,
    core: Core,
    mode: Mode,
    origin: Option<Origin>,
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
    dock_visible: bool,
    look_changed: bool,
    // Apps
    installed: BTreeSet<String>,
    preinstall: Vec<String>,
    preinstall_running: bool,
    chosen: BTreeSet<usize>,
    brew: BTreeSet<usize>,
    dev_chosen: BTreeSet<usize>,
    dev_open: bool,
    install: Install,
    // Computer
    computer: Option<Computer>,
    nvidia: Nvidia,
    tried: BTreeSet<Try>,
    tiling: bool,
}

impl<T: Tour> App<T> {
    fn rebuild_pages(&mut self) {
        self.pages = pages(&self.mode, self.origin);
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

    /// Something to install is chosen
    fn anything_chosen(&self) -> bool {
        !self.chosen.is_empty() || !self.dev_chosen.is_empty()
    }

    /// Progress of the chosen apps and tools: (installed, all)
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

    fn profile_installed(&self, profile: &Profile) -> bool {
        profile.apps.iter().all(|app| self.installed.contains(*app))
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
        let (wallpapers, avatars) = if first_steps {
            (
                system::wallpapers()
                    .into_iter()
                    .map(|(wallpaper, thumb)| (wallpaper, widget::image::Handle::from_path(thumb)))
                    .collect(),
                system::avatars()
                    .into_iter()
                    .map(|path| {
                        let handle = widget::image::Handle::from_path(&path);
                        (path, handle)
                    })
                    .collect(),
            )
        } else {
            (Vec::new(), Vec::new())
        };
        let mut app = App {
            tour: std::marker::PhantomData,
            core,
            pages: pages(&mode, origin),
            mode: mode.clone(),
            origin,
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
            dock_visible: system::dock_always_visible(),
            look_changed: false,
            installed: BTreeSet::new(),
            preinstall: if first_steps {
                system::preinstall_apps()
            } else {
                Vec::new()
            },
            preinstall_running: false,
            chosen: BTreeSet::new(),
            brew: BTreeSet::new(),
            dev_chosen: BTreeSet::new(),
            dev_open: false,
            install: Install::Idle,
            computer: None,
            nvidia: Nvidia::Idle,
            tried: BTreeSet::new(),
            tiling: system::tiling(),
        };
        let title = match &mode {
            Mode::Live => tr("Welcome to cosmic-blue").to_owned(),
            Mode::FirstSteps => tr("First steps").to_owned(),
            Mode::Shortcuts => tr("Keyboard shortcuts").to_owned(),
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
        if matches!(mode, Mode::Open(_)) {
            tasks.push(check_apps());
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
            Message::Run(action) => run(&action),
            Message::OpenWith(command) => {
                if let Mode::Open(file) = &self.mode {
                    let mut args: Vec<&str> = command[1..].to_vec();
                    let file = file.to_string_lossy().into_owned();
                    args.push(&file);
                    spawn(command[0], &args);
                }
                return cosmic::iced::exit();
            }
            Message::CloseToast(id) => self.toasts.remove(id),
            Message::Exit => return cosmic::iced::exit(),
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
            Message::Dock(visible) => {
                if system::set_dock_always_visible(visible) {
                    self.dock_visible = visible;
                    self.look_changed = true;
                } else {
                    return self.toast("This did not work.");
                }
            }
            Message::Profile(i, chosen) => {
                if chosen {
                    self.chosen.insert(i);
                } else {
                    self.chosen.remove(&i);
                }
            }
            Message::DevOpen(open) => self.dev_open = open,
            Message::DevTool(i, chosen) => {
                if chosen {
                    self.dev_chosen.insert(i);
                } else {
                    self.dev_chosen.remove(&i);
                }
            }
            Message::InstallAndNext => {
                let mut apps: Vec<String> = self
                    .chosen
                    .iter()
                    .flat_map(|i| PROFILES[*i].apps.iter())
                    .filter(|app| !self.installed.contains(**app))
                    .map(|app| (*app).to_owned())
                    .collect();
                let mut tools = Vec::new();
                let mut formulas: Vec<&'static str> = Vec::new();
                let mut casks: Vec<String> = Vec::new();
                for (i, tool) in dev_tools().enumerate() {
                    if !self.dev_chosen.contains(&i) || self.tool_installed(i, tool) {
                        continue;
                    }
                    match tool.kind {
                        ToolKind::Flatpak(app) => apps.push(app.to_owned()),
                        ToolKind::Formula(names) => {
                            tools.push(i);
                            formulas.extend(names);
                        }
                        ToolKind::Cask(name, tap) => {
                            tools.push(i);
                            casks.push(devtools::cask_name(name, tap));
                        }
                    }
                }
                self.chosen.clear();
                self.dev_chosen.clear();
                self.dev_open = false;
                let mut tasks = vec![self.update(Message::Turn(1))];
                let pending = u8::from(!apps.is_empty()) + u8::from(!tools.is_empty());
                if pending == 0 {
                    return Task::batch(tasks);
                }
                if !apps.is_empty() {
                    let mut args: Vec<String> =
                        ["install", "--system", "--noninteractive", "-y", "flathub"]
                            .map(str::to_owned)
                            .to_vec();
                    args.extend(apps.iter().cloned());
                    tasks.push(Task::perform(
                        async move {
                            tokio::process::Command::new("flatpak")
                                .args(&args)
                                .stdin(std::process::Stdio::null())
                                .status()
                                .await
                                .is_ok_and(|status| status.success())
                        },
                        |ok| cosmic::Action::App(Message::InstallDone(ok)),
                    ));
                }
                if !tools.is_empty() {
                    tasks.push(Task::perform(brew_install(formulas, casks), |ok| {
                        cosmic::Action::App(Message::InstallDone(ok))
                    }));
                }
                self.install = Install::Running {
                    apps,
                    tools,
                    pending,
                    failed: false,
                };
                return Task::batch(tasks);
            }
            Message::InstallDone(ok) => {
                if let Install::Running {
                    pending, failed, ..
                } = &mut self.install
                {
                    *pending = pending.saturating_sub(1);
                    *failed |= !ok;
                    if *pending == 0 {
                        self.install = if *failed {
                            Install::Failed
                        } else {
                            Install::Done
                        };
                    }
                }
                return check_apps();
            }
            Message::Tick => {
                return check_apps();
            }
            Message::Apps(apps, brew, running) => {
                self.installed = apps;
                self.brew = brew;
                self.preinstall_running = running;
            }
            Message::Computer(computer) => {
                self.computer = Some(computer);
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
        // Alt+Left and Alt+Right turn the pages (also used by the CI boot test)
        let keys = event::listen_with(|event, _status, _window| match event {
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: Key::Named(named),
                modifiers,
                ..
            }) if modifiers.alt() => match named {
                Named::ArrowRight => Some(Message::Turn(1)),
                Named::ArrowLeft => Some(Message::Turn(-1)),
                _ => None,
            },
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
        // On the first page of the first steps tour, a clearly visible
        // "Later" button: the tour stays in the app library
        let close_button = if self.mode == Mode::FirstSteps && self.page_i == 0 {
            Element::from(widget::button::standard(tr("Later")).on_press(Message::Exit))
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
            if matches!(page.body, Body::Profiles) && self.anything_chosen() {
                widget::button::suggested(tr("Install and continue"))
                    .on_press(Message::InstallAndNext)
            } else {
                widget::button::suggested(tr("Next")).on_press(Message::PageOpen(self.page_i + 1))
            }
        } else if self.mode == Mode::Live {
            let install = widget::button::suggested(tr("Install cosmic-blue"));
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

/// A file name short enough for a title: long ones keep their start and
/// their end ("google-chrome-st…amd64.deb")
fn short_name(file: &Path) -> String {
    let name = file.file_name().unwrap_or_default().to_string_lossy();
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= 32 {
        return name.into_owned();
    }
    let start: String = chars[..18].iter().collect();
    let end: String = chars[chars.len() - 12..].iter().collect();
    format!("{start}…{end}")
}

/// Puts a value in place of "{}" in a translated text
fn fill(text: &str, value: &str) -> String {
    text.replacen("{}", value, 1)
}

fn accessibility_row<'a>(space_s: u16) -> Element<'a, Message> {
    // Accessibility, from the first page: what COSMIC's first-run assistant
    // (skipped, see build_files/20-cosmic.sh) offered
    settings::item_row(vec![
        widget::icon::from_name("cosmic-blue-tour-accessibility")
            .size(40)
            .into(),
        two_lines(text::heading(tr("Accessibility")).into(), tr(ACCESSIBILITY)),
        action_button(
            "Settings",
            &Action::Launch(&["cosmic-settings", "accessibility"]),
        ),
    ])
    .spacing(space_s)
    .into()
}

/// A state symbol in front of a row: done (accent color) or to look at
/// A row's symbolic icon, the size of state_icon(), so that the texts of
/// all rows start at the same place
fn row_icon<'a>(name: &'static str) -> Element<'a, Message> {
    widget::icon::from_name(name).size(20).into()
}

fn state_icon<'a>(done: bool) -> Element<'a, Message> {
    let icon = widget::icon::from_name(if done {
        "object-select-symbolic"
    } else {
        "dialog-information-symbolic"
    })
    .size(20)
    .apply(widget::container);
    if done {
        icon.class(theme::Container::custom(|theme| {
            cosmic::iced::widget::container::Style {
                icon_color: Some(theme.cosmic().accent_color().into()),
                ..Default::default()
            }
        }))
        .into()
    } else {
        icon.into()
    }
}

fn heading<'a>(label: &'static str) -> Element<'a, Message> {
    text::heading(tr(label)).width(Length::Fill).into()
}

impl<T: Tour> App<T> {
    fn body<'a>(&'a self, body: &'a Body, space_s: u16) -> Element<'a, Message> {
        let rows: Vec<Element<'a, Message>> = match body {
            Body::Welcome => {
                let illustration: Element<'a, Message> = match &self.welcome {
                    // HERO is 600 x 220 (the logo is square: it keeps its ratio)
                    Some(handle) => widget::svg(handle.clone()).width(600).height(220).into(),
                    None => widget::icon::from_name("computer-symbolic")
                        .size(128)
                        .into(),
                };
                return widget::column::with_capacity(2)
                    .spacing(theme::spacing().space_m)
                    .push(
                        widget::container(illustration)
                            .padding([space_s, 0])
                            .center_x(Length::Fill),
                    )
                    .push(settings::section().add(accessibility_row(space_s)))
                    .into();
            }
            Body::Origin => return self.origin_view(space_s),
            Body::Look => return self.look_view(space_s),
            Body::Profiles => {
                return widget::column::with_children(self.profile_rows(space_s))
                    .spacing(space_s)
                    .into();
            }
            Body::Computer => self.computer_rows(space_s),
            Body::Try => Try::ALL
                .iter()
                .map(|thing| {
                    let (title, caption) = thing.texts();
                    let label = match thing {
                        Try::Tiling if self.tiling => "Turn off",
                        Try::Tiling => "Turn on",
                        _ => "Try",
                    };
                    let available = thing.program().is_none_or(in_path);
                    let button = widget::button::standard(tr(label))
                        .on_press_maybe(available.then_some(Message::Try(*thing)));
                    settings::item_row(vec![
                        state_icon(self.tried.contains(thing)),
                        two_lines(text::heading(tr(title)).into(), tr(caption)),
                        button.into(),
                    ])
                    .spacing(space_s)
                    .into()
                })
                .collect(),
            Body::Ready => return self.ready_view(space_s),
            Body::Icon(name) => {
                return widget::container(widget::icon::from_name(*name).size(128))
                    .padding([space_s, 0])
                    .center_x(Length::Fill)
                    .into();
            }
            Body::Features(rows) => rows
                .iter()
                .map(|(title, subtitle)| {
                    settings::item_row(vec![two_lines(
                        text::heading(tr(title)).into(),
                        tr(subtitle),
                    )])
                    .into()
                })
                .collect(),
            Body::Rows(rows) => rows
                .iter()
                .map(|row| {
                    let mut items = vec![two_lines(title_line(tr(row.title)), tr(row.caption))];
                    if let Some((label, action)) = &row.button {
                        items.push(action_button(label, action));
                    }
                    settings::item_row(items).spacing(space_s).into()
                })
                .collect(),
        };
        section(rows)
    }

    fn origin_view<'a>(&'a self, space_s: u16) -> Element<'a, Message> {
        let cards = Origin::ALL.iter().fold(
            widget::row::with_capacity(3).spacing(space_s).height(128),
            |row, origin| {
                let (icon, name, caption) = origin.card();
                let card = widget::column::with_capacity(3)
                    .spacing(4)
                    .align_x(Alignment::Center)
                    .push(widget::icon::from_name(icon).size(32))
                    .push(text::heading(name))
                    .push(
                        text::caption(tr(caption))
                            .align_x(Alignment::Center)
                            .wrapping(Wrapping::Word),
                    )
                    .width(Length::Fill)
                    .padding(space_s);
                row.push(
                    widget::button::custom(card)
                        .class(card_style(self.origin == Some(*origin)))
                        .height(Length::Fill)
                        .width(Length::Fill)
                        .on_press(Message::Origin(*origin)),
                )
            },
        );
        let mut column = widget::column::with_capacity(3)
            .spacing(theme::spacing().space_m)
            .push(cards)
            .push(settings::section().add(accessibility_row(space_s)));
        if let Some((done, all)) = self.preinstall_progress() {
            column = column.push(install_progress(
                fill(
                    &fill(tr("Installing your apps: {} of {}"), &done.to_string()),
                    &all.to_string(),
                ),
                tr("They show up in the app library as they arrive."),
                done,
                all,
                space_s,
            ));
        }
        column.into()
    }

    fn look_view<'a>(&'a self, space_s: u16) -> Element<'a, Message> {
        let theme_buttons = [(true, "Dark"), (false, "Light")].into_iter().fold(
            widget::row::with_capacity(2).spacing(space_s),
            |row, (dark, label)| {
                let button = if self.dark == dark {
                    widget::button::suggested(tr(label))
                } else {
                    widget::button::standard(tr(label))
                };
                row.push(button.on_press(Message::Dark(dark)))
            },
        );
        // (otherwise it hides when a window covers it)
        let dock = settings::item_row(vec![
            text::body(tr("Dock always visible"))
                .width(Length::Fill)
                .into(),
            widget::toggler(self.dock_visible)
                .on_toggle(Message::Dock)
                .into(),
        ])
        .spacing(space_s);
        let mut column = widget::column::with_capacity(5).spacing(space_s).push(
            settings::section()
                .add(settings::item_row(vec![
                    text::body(tr("Theme")).width(Length::Fill).into(),
                    theme_buttons.into(),
                ]))
                .add(dock),
        );
        if !self.wallpapers.is_empty() {
            let thumbs: Vec<Element<'a, Message>> = self
                .wallpapers
                .iter()
                .enumerate()
                .map(|(i, (path, handle))| {
                    widget::button::image(handle.clone())
                        .width(92)
                        .height(52)
                        .selected(self.wallpaper.as_deref() == Some(path.as_path()))
                        .on_press(Message::Wallpaper(i))
                        .into()
                })
                .collect();
            column = column.push(heading("Wallpaper")).push(
                widget::flex_row(thumbs)
                    .row_spacing(space_s)
                    .column_spacing(space_s),
            );
        }
        if !self.avatars.is_empty() {
            let faces: Vec<Element<'a, Message>> = self
                .avatars
                .iter()
                .enumerate()
                .map(|(i, (_, handle))| {
                    widget::button::image(handle.clone())
                        .width(40)
                        .height(40)
                        .selected(self.avatar == Some(i))
                        .on_press(Message::Avatar(i))
                        .into()
                })
                .collect();
            column = column.push(heading("Your picture")).push(
                widget::flex_row(faces)
                    .row_spacing(space_s)
                    .column_spacing(space_s),
            );
        }
        column.into()
    }

    fn profile_rows<'a>(&'a self, space_s: u16) -> Vec<Element<'a, Message>> {
        if self.dev_open {
            return self.dev_rows(space_s);
        }
        let running = matches!(self.install, Install::Running { .. });
        let mut rows: Vec<Element<'a, Message>> = PROFILES
            .iter()
            .enumerate()
            .map(|(i, profile)| {
                let installed = self.profile_installed(profile);
                let check: Element<'a, Message> = if installed {
                    text::caption(tr("Installed")).into()
                } else {
                    widget::checkbox(self.chosen.contains(&i))
                        .on_toggle_maybe((!running).then_some(move |on| Message::Profile(i, on)))
                        .into()
                };
                settings::item_row(vec![
                    two_lines(text::heading(tr(profile.title)).into(), tr(profile.caption)),
                    check,
                ])
                .spacing(space_s)
                .into()
            })
            .collect();
        let chosen = self.dev_chosen.len();
        let caption = if chosen == 0 {
            tr("Docker, VS Code, Zed, Neovim…: choose your tools").to_owned()
        } else {
            fill(tr("Chosen: {}"), &chosen.to_string())
        };
        rows.push(
            settings::item_row(vec![
                widget::column::with_capacity(2)
                    .spacing(2)
                    .push(text::heading(tr("Development")))
                    .push(dim_caption(caption).wrapping(Wrapping::Word))
                    .width(Length::Fill)
                    .into(),
                widget::button::standard(tr("Choose…"))
                    .on_press_maybe((!running).then_some(Message::DevOpen(true)))
                    .into(),
            ])
            .spacing(space_s)
            .into(),
        );
        let mut body = vec![section(rows)];
        if let Some((done, all)) = self.install_count() {
            body.push(install_progress(
                fill(
                    &fill(tr("Installing: {} of {}"), &done.to_string()),
                    &all.to_string(),
                ),
                tr("They show up in the app library as they arrive."),
                done,
                all,
                space_s,
            ));
        } else if self.install == Install::Failed {
            body.push(
                text::caption(tr(
                    "Some apps did not install. Are you online? They are also in COSMIC Store.",
                ))
                .align_x(Alignment::Center)
                .width(Length::Fill)
                .into(),
            );
        }
        body
    }

    /// The developer tools, by group, each with its checkbox
    fn dev_rows<'a>(&'a self, space_s: u16) -> Vec<Element<'a, Message>> {
        let brew = Path::new(devtools::BREW).exists();
        let mut i = 0;
        let mut rows = Vec::with_capacity(DEV_TOOLS.len());
        for (heading, tools) in DEV_TOOLS {
            let mut grid: Vec<Element<'a, Message>> = Vec::new();
            for tool in tools.iter() {
                let n = i;
                i += 1;
                let installed = self.tool_installed(n, tool);
                let available = brew || matches!(tool.kind, ToolKind::Flatpak(_));
                let checkbox = widget::checkbox(installed || self.dev_chosen.contains(&n))
                    .on_toggle_maybe(
                        (!installed && available).then_some(move |on| Message::DevTool(n, on)),
                    );
                // The tool's name, aligned with its checkbox: clicking it
                // ticks the box too
                let label = text::body(tool.name).apply(widget::mouse_area);
                let label = if installed || !available {
                    label
                } else {
                    label.on_press(Message::DevTool(n, !self.dev_chosen.contains(&n)))
                };
                grid.push(
                    widget::row::with_capacity(2)
                        .spacing(space_s / 2)
                        .align_y(Alignment::Center)
                        .push(checkbox)
                        .push(label)
                        .width(Length::Fixed(148.0))
                        .into(),
                );
            }
            // The group's name on the left, its tools in columns on the right
            rows.push(
                settings::item_row(vec![
                    text::heading(tr(heading))
                        .width(Length::Fixed(140.0))
                        .into(),
                    // Three columns
                    {
                        let mut lines = widget::column::with_capacity(grid.len().div_ceil(3))
                            .spacing(space_s / 2)
                            .width(Length::Fill);
                        let mut cells = grid.into_iter().peekable();
                        while cells.peek().is_some() {
                            lines = lines.push(cells.by_ref().take(3).fold(
                                widget::row::with_capacity(3).spacing(space_s),
                                |row, cell| row.push(cell),
                            ));
                        }
                        lines.into()
                    },
                ])
                .spacing(space_s)
                .align_y(Alignment::Start)
                .into(),
            );
        }
        let note = if brew {
            tr(
                "Ticked and grey: already installed. Lima and virtual machines: ujust install-dev-tools and ujust install-vms, in a terminal.",
            )
        } else {
            tr(
                "Homebrew is still being set up: the code editors and terminal tools can be chosen in a few minutes.",
            )
        };
        vec![
            section(rows),
            text::caption(note)
                .wrapping(Wrapping::Word)
                .align_x(Alignment::Center)
                .width(Length::Fill)
                .into(),
            widget::button::standard(tr("Back to the list"))
                .on_press(Message::DevOpen(false))
                .apply(widget::container)
                .center_x(Length::Fill)
                .into(),
        ]
    }

    fn computer_rows<'a>(&'a self, space_s: u16) -> Vec<Element<'a, Message>> {
        let Some(computer) = &self.computer else {
            return vec![
                text::body(tr("Checking your computer…"))
                    .align_x(Alignment::Center)
                    .width(Length::Fill)
                    .into(),
            ];
        };
        let item =
            |done: bool, title: String, caption: String, end: Option<Element<'a, Message>>| {
                let mut text_column = widget::column::with_capacity(2)
                    .spacing(2)
                    .push(text::heading(title))
                    .width(Length::Fill);
                if !caption.is_empty() {
                    text_column = text_column.push(dim_caption(caption).wrapping(Wrapping::Word));
                }
                let mut items = vec![state_icon(done), text_column.into()];
                items.extend(end);
                Element::from(settings::item_row(items).spacing(space_s))
            };
        let mut rows = Vec::new();
        match computer.online {
            Some(true) => rows.push(item(
                true,
                tr("Connected to the internet").to_owned(),
                String::new(),
                None,
            )),
            Some(false) => rows.push(item(
                false,
                tr("No internet connection").to_owned(),
                tr("Apps and updates need it.").to_owned(),
                Some(action_button(
                    "Network settings",
                    &Action::Launch(&["cosmic-settings", "network"]),
                )),
            )),
            None => {}
        }
        match (computer.nvidia, &self.nvidia) {
            (Some(true), _) => rows.push(item(
                true,
                tr("NVIDIA graphics card").to_owned(),
                tr("Its driver is installed: this is the NVIDIA edition of cosmic-blue.")
                    .to_owned(),
                None,
            )),
            (Some(false), Nvidia::Switched) => {
                let mut caption = tr("The NVIDIA edition starts at the next restart.").to_owned();
                if computer.secure_boot {
                    caption.push(' ');
                    caption.push_str(tr("At the restart, a blue screen asks to enroll a key: choose Enroll MOK, Continue, Yes, then type the password cosmicblue (the keyboard is QWERTY there)."));
                }
                rows.push(item(
                    true,
                    tr("NVIDIA graphics card").to_owned(),
                    caption,
                    None,
                ));
            }
            (Some(false), state) => {
                let switching = *state == Nvidia::Switching;
                rows.push(item(
                    false,
                    tr("NVIDIA graphics card").to_owned(),
                    tr(if switching {
                        "Downloading the NVIDIA edition… This can take a while."
                    } else {
                        "The NVIDIA edition of cosmic-blue has its driver: about 4 GB to download, then it starts at the next restart."
                    })
                    .to_owned(),
                    Some(
                        widget::button::suggested(tr("Switch"))
                            .on_press_maybe(
                                (!switching && Path::new(system::UPDATES_HELPER).exists())
                                    .then_some(Message::SwitchNvidia),
                            )
                            .into(),
                    ),
                ));
            }
            (None, _) => {}
        }
        match &computer.firmware {
            Some(devices) if !devices.is_empty() => rows.push(item(
                false,
                tr("Firmware updates are available").to_owned(),
                devices.join(", "),
                Some(action_button("Open Firmware", &Action::Launch(FIRMWARE))),
            )),
            Some(_) => rows.push(item(
                true,
                tr("Firmware is up to date").to_owned(),
                String::new(),
                None,
            )),
            None => {}
        }
        if let Some(on) = computer.battery_limit {
            rows.push(
                settings::item_row(vec![
                    row_icon("battery-good-symbolic"),
                    two_lines(
                        text::heading(tr("Take care of the battery")).into(),
                        tr("Stop charging at 80 % when the laptop stays plugged in: the battery lasts longer."),
                    ),
                    widget::toggler(on).on_toggle(Message::Battery).into(),
                ])
                .spacing(space_s)
                .into(),
            );
        }
        rows.push(
            settings::item_row(vec![
                row_icon("video-display-symbolic"),
                two_lines(
                    text::heading(tr("Screen scale")).into(),
                    tr("Text and windows too small or too big? Change the scale of the screen."),
                ),
                action_button(
                    "Display settings",
                    &Action::Launch(&["cosmic-settings", "displays"]),
                ),
            ])
            .spacing(space_s)
            .into(),
        );
        vec![section(rows)]
    }

    fn ready_view<'a>(&'a self, space_s: u16) -> Element<'a, Message> {
        let mut done = Vec::new();
        let check = |title: String, caption: Option<String>| -> Element<'a, Message> {
            let mut column = widget::column::with_capacity(2)
                .spacing(2)
                .push(text::heading(title));
            if let Some(caption) = caption {
                column = column.push(dim_caption(caption).wrapping(Wrapping::Word));
            }
            settings::item_row(vec![state_icon(true), column.width(Length::Fill).into()])
                .spacing(space_s)
                .into()
        };
        if self.look_changed {
            done.push(check(tr("Your look is set").to_owned(), None));
        }
        match (&self.install, self.install_count()) {
            (_, Some((finished, all))) => {
                done.push(check(
                    fill(
                        &fill(tr("Installing: {} of {}"), &finished.to_string()),
                        &all.to_string(),
                    ),
                    Some(tr("They show up in the app library as they arrive.").to_owned()),
                ));
            }
            (Install::Done, _) => done.push(check(tr("Your apps are installed").to_owned(), None)),
            (Install::Failed, _) => done.push(check(
                tr("Some apps did not install").to_owned(),
                Some(tr("Are you online? They are also in COSMIC Store.").to_owned()),
            )),
            _ => {}
        }
        if self.nvidia == Nvidia::Switched {
            done.push(check(
                tr("The NVIDIA edition starts at the next restart.").to_owned(),
                None,
            ));
        }
        let backups = settings::item_row(vec![
            widget::icon::from_name("org.gnome.DejaDup").size(40).into(),
            two_lines(
                text::heading(tr("Protect your files")).into(),
                tr("Plug in a USB drive and turn on automatic backups: Déjà Dup keeps a copy of your home folder."),
            ),
            widget::button::suggested(tr("Set up backups"))
                .on_press_maybe(in_path("deja-dup").then_some(Message::Run(Action::Launch(&["deja-dup"]))))
                .into(),
        ])
        .spacing(space_s);
        let links = [
            (
                "Documentation",
                "Updates, software, commands and tips",
                docs_url(),
            ),
            (
                "Report a problem",
                "Found a bug or something unclear? Tell us on GitHub",
                ISSUES.to_owned(),
            ),
        ]
        .into_iter()
        .map(|(title, caption, uri)| {
            settings::item_row(vec![
                two_lines(text::heading(tr(title)).into(), tr(caption)),
                widget::button::standard(tr("Open"))
                    .trailing_icon(widget::button::link::icon())
                    .on_press(Message::Run(Action::OpenUri(uri)))
                    .into(),
            ])
            .into()
        })
        .collect::<Vec<Element<'a, Message>>>();
        let mut column = widget::column::with_capacity(4).spacing(theme::spacing().space_m);
        if !done.is_empty() {
            column = column.push(section(done));
        }
        column
            .push(settings::section().add(backups))
            .push(section(links))
            .push(
                text::caption(tr(
                    "This tour and the Keyboard shortcuts app stay in the app library.",
                ))
                .align_x(Alignment::Center)
                .width(Length::Fill),
            )
            .into()
    }

    /// The window a downloaded .exe, .msi, .deb or .rpm file opens
    fn open_view<'a>(&'a self, file: &'a Path) -> Element<'a, Message> {
        let cosmic_theme::Spacing {
            space_s,
            space_m,
            space_l,
            ..
        } = theme::spacing();
        let kind = system::package_kind(file);
        let name = short_name(file);
        let term = system::search_term(file);
        let explanation = match kind {
            Package::Windows => {
                "This is a Windows program. On cosmic-blue, apps come from COSMIC Store: most well-known apps are there, or a good alternative."
            }
            Package::Debian => {
                "This is a package for Debian and Ubuntu. On cosmic-blue, apps come from COSMIC Store: most well-known apps are there, or a good alternative."
            }
            Package::Rpm => {
                "This is a package for Fedora. cosmic-blue's system is a read-only image: apps come from COSMIC Store instead. Most well-known apps are there, or a good alternative."
            }
        };
        let hint = match kind {
            Package::Windows => {
                "A Windows program with no equivalent? Bottles, in COSMIC Store, runs some of them."
            }
            _ => {
                "For advanced users: distrobox installs it in a container of another distribution."
            }
        };
        let search_label = if term.is_empty() {
            tr("Open COSMIC Store").to_owned()
        } else {
            fill(tr("Search for “{}” in COSMIC Store"), &term)
        };
        let search: &'static [&'static str] = if term.is_empty() {
            &["cosmic-store"]
        } else {
            Box::leak(Box::new(["cosmic-store", Box::leak(term.into_boxed_str())]))
        };
        let mut buttons = widget::row::with_capacity(4)
            .spacing(space_s)
            .push(widget::space::horizontal())
            .push(widget::button::standard(tr("Close")).on_press(Message::Exit));
        if kind == Package::Windows && self.installed.contains("com.usebottles.bottles") {
            buttons = buttons.push(widget::button::standard(tr("Open with Bottles")).on_press(
                Message::OpenWith(&["flatpak", "run", "com.usebottles.bottles"]),
            ));
        }
        buttons = buttons
            .push(widget::button::suggested(search_label).on_press(Message::OpenWith(search)));
        widget::column::with_capacity(6)
            .spacing(space_m)
            .push(
                text::title3(fill(tr("“{}” does not install here"), &name))
                    .wrapping(Wrapping::Word),
            )
            .push(text::body(tr(explanation)).wrapping(Wrapping::Word))
            .push(text::caption(tr(hint)).wrapping(Wrapping::Word))
            .push(widget::space::vertical())
            .push(buttons)
            .padding(space_l)
            .apply(widget::container)
            .into()
    }

    /// The "Keyboard shortcuts" app
    fn shortcuts_view(&self) -> Element<'_, Message> {
        let cosmic_theme::Spacing {
            space_s,
            space_m,
            space_l,
            ..
        } = theme::spacing();
        let mut column = widget::column::with_capacity(SHORTCUTS.len() + 3)
            .spacing(space_m)
            .push(
                text::caption(tr(
                    "Super is the key with the Windows logo on most keyboards.",
                ))
                .width(Length::Fill),
            );
        for (title, rows) in SHORTCUTS {
            let mut section = settings::section().title(tr(title));
            for (keys, description) in rows.iter() {
                section = section.add(
                    settings::item_row(vec![
                        text::body(tr(description))
                            .wrapping(Wrapping::Word)
                            .width(Length::Fill)
                            .into(),
                        with_keys_end(keys),
                    ])
                    .spacing(space_s),
                );
            }
            column = column.push(section);
        }
        column
            .push(
                widget::button::standard(tr("Change them in Settings"))
                    .on_press_maybe(in_path("cosmic-settings").then_some(Message::Run(
                        Action::Launch(&["cosmic-settings", "keyboard"]),
                    )))
                    .apply(widget::container)
                    .center_x(Length::Fill),
            )
            .padding([space_s, space_l, space_l, space_l])
            .max_width(720.0)
            .apply(widget::scrollable)
            .apply(widget::container)
            .center_x(Length::Fill)
            .into()
    }
}

/// The origin cards: a settings-row background, an accent border when
/// chosen, a little lighter under the mouse (a custom style is told whether
/// the button has the keyboard focus, not whether it is selected)
fn card_style(selected: bool) -> theme::Button {
    fn style(selected: bool, theme: &cosmic::Theme, hover: bool) -> widget::button::Style {
        let cosmic = theme.cosmic();
        let component = &theme.current_container().component;
        let mut style = widget::button::Style::new();
        style.background = Some(cosmic::iced::Background::Color(
            if hover {
                component.hover
            } else {
                component.base
            }
            .into(),
        ));
        style.border_radius = cosmic.corner_radii.radius_s.into();
        style.text_color = Some(component.on.into());
        style.icon_color = Some(if selected {
            cosmic.accent.base.into()
        } else {
            component.on.into()
        });
        if selected {
            style.border_width = 2.0;
            style.border_color = cosmic.accent.base.into();
        }
        style
    }
    theme::Button::Custom {
        active: Box::new(move |_, theme| style(selected, theme, false)),
        disabled: Box::new(move |theme| style(selected, theme, false)),
        hovered: Box::new(move |_, theme| style(selected, theme, true)),
        pressed: Box::new(move |_, theme| style(selected, theme, true)),
    }
}

/// Rows in a COSMIC settings section
fn section<'a>(rows: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    rows.into_iter()
        .fold(settings::section(), |section, row| section.add(row))
        .into()
}

/// A line of progress: text, caption and bar
fn install_progress<'a>(
    title: String,
    caption: &'static str,
    done: usize,
    all: usize,
    space_s: u16,
) -> Element<'a, Message> {
    let progress = if all == 0 {
        0.0
    } else {
        done as f32 / all as f32
    };
    settings::section()
        .add(
            widget::column::with_capacity(3)
                .spacing(space_s)
                .push(text::heading(title))
                .push(widget::determinate_linear(progress).width(Length::Fill))
                .push(dim_caption(caption).wrapping(Wrapping::Word))
                .apply(widget::container)
                .padding([space_s, space_s + 8]),
        )
        .into()
}

/// A row's text: a first line, and a caption under it (like the screen
/// reader row of cosmic-initial-setup's accessibility page)
fn two_lines<'a>(first: Element<'a, Message>, caption: &'static str) -> Element<'a, Message> {
    widget::column::with_capacity(2)
        .spacing(2)
        .push(first)
        .push(with_keys(caption))
        .width(Length::Fill)
        .into()
}

/// Names of keys found in the texts (and their translations), drawn as
/// keyboard keys when they are part of a combination such as "Super + A"
const KEY_NAMES: &[&str] = &[
    "Super", "Alt", "Ctrl", "Strg", "Shift", "Maj", "Umschalt", "Mayús", "Maiusc", "Tab", "Esc",
    "Escape", "Échap", "Enter", "Print", "←", "↑", "↓", "→",
];

fn is_key(word: &str) -> bool {
    KEY_NAMES.contains(&word) || (word.chars().count() == 1 && !matches!(word, "+" | "/" | "…"))
}

/// The pieces of a line: plain words, and key combinations (their keys)
#[derive(Debug, PartialEq)]
enum Piece<'s> {
    Word(&'s str),
    Keys(Vec<&'s str>),
}

fn pieces(line: &str) -> Vec<Piece<'_>> {
    let words: Vec<&str> = line.split(' ').filter(|word| !word.is_empty()).collect();
    let mut pieces = Vec::new();
    let mut i = 0;
    while i < words.len() {
        // A key, then "+" and a key, maybe more: a combination
        let strip = |word: &str| -> usize {
            let trimmed = word.trim_end_matches([',', '.', ':', ';', ')']);
            let key = if trimmed.is_empty() { word } else { trimmed };
            key.len()
        };
        let starts = is_key(words[i])
            && words.get(i + 1) == Some(&"+")
            && words
                .get(i + 2)
                .is_some_and(|next| is_key(&next[..strip(next)]));
        if !starts {
            pieces.push(Piece::Word(words[i]));
            i += 1;
            continue;
        }
        let mut keys = vec![words[i]];
        let mut rest = None;
        i += 1;
        while words.get(i) == Some(&"+") {
            let Some(next) = words.get(i + 1) else { break };
            let len = strip(next);
            if !is_key(&next[..len]) {
                break;
            }
            keys.push(&next[..len]);
            i += 2;
            if len < next.len() {
                rest = Some(&next[len..]);
                break;
            }
            // Arrows or a range after the last "+": "← ↑ ↓ →", "1 … 9"
            while rest.is_none()
                && words.get(i).is_some_and(|word| {
                    matches!(*word, "←" | "↑" | "↓" | "→" | "…")
                        || (keys.last().is_some_and(|last| *last == "…")
                            && word.chars().all(|c| c.is_ascii_digit()))
                })
            {
                keys.push(words[i]);
                i += 1;
            }
        }
        pieces.push(Piece::Keys(keys));
        if let Some(rest) = rest {
            pieces.push(Piece::Word(rest));
        }
    }
    pieces
}

/// A keyboard key: its name in a small rounded box
fn keycap<'a>(name: &'a str) -> Element<'a, Message> {
    text::caption(name)
        .apply(widget::container)
        .padding([1, 6])
        .class(theme::Container::custom(|theme| {
            let cosmic = theme.cosmic();
            let mut background = cosmic.on_bg_color();
            background.alpha = 0.1;
            let mut border = cosmic.on_bg_color();
            border.alpha = 0.35;
            cosmic::iced::widget::container::Style {
                background: Some(cosmic::iced::Color::from(background).into()),
                border: cosmic::iced::Border {
                    color: border.into(),
                    width: 1.0,
                    radius: 5.0.into(),
                },
                ..Default::default()
            }
        }))
        .into()
}

/// A combination of keys: "Super + A" as two keys joined by "+"
fn key_combo<'a>(keys: &[&'a str]) -> Element<'a, Message> {
    let mut row = widget::row::with_capacity(keys.len() * 2)
        .spacing(3)
        .align_y(Alignment::Center);
    let mut previous: Option<&str> = None;
    for key in keys {
        // "+" between the keys, but not inside a group of arrows or a range
        let grouped = previous.is_some_and(|p| matches!(p, "←" | "↑" | "↓" | "→" | "…"))
            || matches!(*key, "↑" | "↓" | "→" | "…")
            || previous == Some("…");
        if previous.is_some() && !grouped {
            row = row.push(text::caption("+"));
        }
        row = row.push(if *key == "…" {
            text::caption("…").into()
        } else {
            keycap(key)
        });
        previous = Some(key);
    }
    row.into()
}

/// A row's description: smaller and dimmer than its title
fn dim_caption<'a>(
    caption: impl Into<std::borrow::Cow<'a, str>> + 'a,
) -> widget::Text<'a, cosmic::Theme> {
    text::caption(caption).class(theme::Text::Custom(|theme| {
        let mut color = theme.cosmic().on_bg_color();
        color.alpha = 0.72;
        cosmic::iced::widget::text::Style {
            color: Some(color.into()),
            ..Default::default()
        }
    }))
}

/// A row's title; "A → B" shows A, an arrow and B, and keys as keys
fn title_line<'a>(title: &'a str) -> Element<'a, Message> {
    let part = |part: &'a str| -> Element<'a, Message> {
        let pieces = pieces(part);
        if !pieces.iter().any(|piece| matches!(piece, Piece::Keys(_))) {
            return text::heading(part).into();
        }
        pieces
            .into_iter()
            .fold(
                widget::row::with_capacity(3)
                    .spacing(4)
                    .align_y(Alignment::Center),
                |row, piece| match piece {
                    Piece::Word(word) => row.push(text::heading(word)),
                    Piece::Keys(keys) => row.push(key_combo(&keys)),
                },
            )
            .into()
    };
    match title.split_once(" → ") {
        Some((from, to)) => widget::row::with_capacity(3)
            .spacing(8)
            .align_y(Alignment::Center)
            .push(part(from))
            .push(widget::icon::from_name("go-next-symbolic").size(16))
            .push(part(to))
            .into(),
        None => part(title),
    }
}

/// The keys of a shortcut, as keys, at the end of a row
fn with_keys_end<'a>(keys: &'a str) -> Element<'a, Message> {
    pieces(keys)
        .into_iter()
        .fold(
            widget::row::with_capacity(3)
                .spacing(6)
                .align_y(Alignment::Center),
            |row, piece| match piece {
                Piece::Word(word) => row.push(dim_caption(word)),
                Piece::Keys(keys) => row.push(key_combo(&keys)),
            },
        )
        .into()
}

/// A caption where key combinations show as keys; each line of the text is
/// a line on screen, and long lines wrap between words
fn with_keys<'a>(caption: &'a str) -> Element<'a, Message> {
    if !caption
        .lines()
        .any(|line| pieces(line).iter().any(|p| matches!(p, Piece::Keys(_))))
    {
        return dim_caption(caption).wrapping(Wrapping::Word).into();
    }
    caption
        .lines()
        .fold(
            widget::column::with_capacity(2).spacing(4),
            |column, line| {
                let words: Vec<Element<'a, Message>> = pieces(line)
                    .into_iter()
                    .map(|piece| match piece {
                        Piece::Word(word) => dim_caption(word).into(),
                        Piece::Keys(keys) => key_combo(&keys),
                    })
                    .collect();
                column.push(
                    widget::flex_row(words)
                        .column_spacing(4)
                        .row_spacing(4)
                        .align_items(Alignment::Center),
                )
            },
        )
        .width(Length::Fill)
        .into()
}

fn action_button<'a>(label: &'static str, action: &Action) -> Element<'a, Message> {
    let available = match action {
        Action::Launch(command) => in_path(command[0]),
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

fn spawn(program: &str, args: &[&str]) {
    if let Err(error) = Command::new(program).args(args).spawn() {
        eprintln!("Could not start {program}: {error}");
    }
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
    }
}

#[cfg(test)]
mod tests {
    use super::{Piece, pieces};

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
