// Tools of Celenn OS Tools (tools.rs) that turn `ujust` commands into
// buttons. Each button runs the matching command (`ujust <recipe>`, without
// a terminal: the recipes then skip their questions), so the terminal and
// the window do exactly the same thing. Each part is one tool:
//
//   Print       the printers added, Print Settings and Document Scanner
//               (COSMIC Settings has no printer page)
//   Inside      what the computer is made of (hardware.rs)
//   Battery     the battery charge limit (toggle-battery-limit)
//   Disks       the other disks to open at every startup (more_disks.rs,
//               disks.rs)
//   Look        dark or light, the layout, the dock
//   Nvidia      the NVIDIA edition for NVIDIA graphics cards (the Updates
//               helper, as the first steps tour) and, with Secure Boot, its
//               driver key (Tools helper, as `ujust enroll-secure-boot-key`)
//   Fonts       Microsoft fonts
//   Apps        the kinds of apps of the first steps tour, as tiles that
//               open a list to tick (apps.rs)
//   Transfer    settings and apps taken to another computer
//               (more_transfer.rs, transfer.rs)
//   Repair      the Celenn OS look back, every COSMIC setting back to the
//               defaults
//   Advanced    restarting into the firmware settings (reboot-to-bios),
//               virtual machines, the FlatPark app source
//
// Changes that cannot be undone in one click (restart, reset, the NVIDIA
// edition, a license) ask first, in the row itself. A change that does not
// work shows a card at the top of its tool (failure.rs).

use crate::apps::{self, CATEGORIES, DEVELOPMENT, PASSWORDS};
use crate::common::{Summary, ToastAsk, fill, label, spawn, state_icon};
use crate::devtools::{self, ToolKind};
use crate::failure::{self, Card, CardAction, CardMessage, Outcome};
use crate::hardware::Boot;
use crate::i18n::tr;
use cosmic::app::Task;
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, settings, text};
use cosmic::{Element, cosmic_theme, theme};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Stdio};

#[path = "more_transfer.rs"]
mod transfer_view;
pub use transfer_view::TransferMessage;
#[path = "more_disks.rs"]
mod disks_view;
pub use disks_view::DisksMessage;
// The tools' pages (Inside, Battery, NVIDIA, Advanced, Apps, Look, Repair)
#[path = "more_views.rs"]
mod views;

const UPDATES_HELPER: &str = "/usr/libexec/celenn-updates-helper";
const TOOLS_HELPER: &str = "/usr/libexec/celenn-tools-helper";
const FONTS_LICENSE: &str = "https://corefonts.sourceforge.net/eula.htm";
const VMS: &str = "org.virt_manager.virt-manager";
const SCANNER: &str = "org.gnome.SimpleScan";

pub const TEXTS: &[&str] = &[
    // The Install apps tool
    "Ready in one click. In each list, the apps most used on Linux are already ticked.",
    // Common
    "Install",
    "Remove",
    "Cancel",
    "Working…",
    // Computer
    "Settings",
    "Printers and scanners",
    "No printer added yet. Recent printers on the network or plugged in by USB show up by themselves in the print window.",
    "{} (default)",
    "Printers",
    "Scan",
    "Model",
    "Virtual machine ({})",
    "Unknown",
    "Processor and memory",
    "{} cores",
    "{} GB of memory",
    "Graphics card",
    "{}. The open driver built into Linux works with it: nothing to install.",
    "Startup",
    "UEFI, Secure Boot on",
    "UEFI, Secure Boot off",
    "UEFI",
    "Legacy BIOS",
    "No battery on this computer: nothing to set.",
    "This computer starts in legacy BIOS mode. To open its settings, press the key shown when it powers on, often F2, F10 or Del.",
    "Battery charge limit",
    "Charging stops at 80 %: the battery lasts longer when the laptop stays plugged in.",
    "NVIDIA graphics card",
    "Use the NVIDIA edition of Celenn OS, with NVIDIA's own driver, for games and graphics apps.",
    "You use the NVIDIA edition, with NVIDIA's own driver.",
    "Use it",
    "The NVIDIA edition is downloaded now and starts at the next restart. With Secure Boot on, a blue screen asks for the password celennos once.",
    "Switch",
    "The NVIDIA edition starts at the next restart.",
    "Secure Boot does not let NVIDIA's driver load yet. Allow it once: at the next restart, a blue screen asks for the password celennos.",
    "Allow",
    "At the next restart, a blue screen asks for the password celennos: choose Enroll MOK, Continue, Yes, type celennos, then Reboot. The keyboard may be in QWERTY.",
    "Firmware settings (BIOS/UEFI)",
    "Restart into the computer's own settings: boot order, virtualization, Secure Boot…",
    "Restart",
    "The computer restarts now. Save your work first.",
    "Restart now",
    // Extra software
    "Microsoft fonts",
    "Arial, Times New Roman, Verdana… for documents made on Windows.",
    "Installed. Restart open apps to see them.",
    "Microsoft's license applies to these fonts.",
    "Read it",
    "Accept and install",
    "Virtual machines",
    "virt-manager, to run Windows or another Linux in a window.",
    "FlatPark",
    "More apps in COSMIC Store, from outside Flathub. Less checked.",
    "Tick the ones you want, untick the ones to remove.",
    "Apply",
    "Back to the list",
    // Appearance
    "Dark theme",
    "Light text on dark backgrounds. Off: the light theme.",
    "Light theme",
    "Undo",
    "Layout",
    "Top bar and dock: like the Mac. One bar at the bottom: like Windows.",
    "Top bar and dock",
    "Bar at the bottom",
    "Dock always visible",
    "Off: it hides when a window covers it.",
    "The dock hides when a window covers it.",
    "This did not work.",
    "More in Settings",
    "Accent color, wallpaper, panel, dock, fonts…",
    "Open",
    "The Celenn OS look",
    "Back to the Celenn OS colors, dark theme, icons, fonts and wallpaper. Panel, dock, keyboard and shortcuts stay.",
    "Restore",
    "All COSMIC settings",
    "Back to the Celenn OS defaults, for every setting. A copy of the current ones is kept.",
    "Reset",
    "Every COSMIC setting goes back to the defaults. A copy is kept in your settings folder.",
    "Log out and back in to finish.",
    "Save",
    "No printer added yet.",
    "Charging stops at 80 %.",
    "Off: charges to 100 %.",
    "None opens at startup yet.",
    "Its driver waits for your OK.",
    "NVIDIA's own driver.",
    "The open Linux driver.",
];

/// The parts of these pages, each one tool (tools.rs)
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Part {
    Print,
    Inside,
    Battery,
    Disks,
    Look,
    Nvidia,
    Fonts,
    Apps,
    Transfer,
    Repair,
    Advanced,
}

/// What the rows show: read in the background, again after each change
#[derive(Clone, Debug, Default)]
pub struct State {
    /// The printers added (CUPS), the default one first
    printers: Vec<Printer>,
    /// The internal data disks (disks.rs)
    disks: Vec<crate::disks::Disk>,
    /// Document Scanner is installed
    scanner: bool,
    /// None: no laptop battery whose charge can be limited
    battery_limit: Option<bool>,
    /// None: no NVIDIA graphics card. Some(true): the NVIDIA edition runs
    nvidia: Option<bool>,
    /// The NVIDIA edition with Secure Boot on: its driver key, when it is
    /// not enrolled yet
    driver_key: Option<DriverKey>,
    efi: bool,
    fonts: bool,
    vms: bool,
    flatpark: bool,
    dark: bool,
    bottom_bar: bool,
    dock_visible: bool,
    /// The apps of each category installed (numbers of Category::tools())
    installed: Vec<BTreeSet<usize>>,
    brew: bool,
}

/// A printer added to CUPS: its name for people, and whether it is the
/// default one
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Printer {
    name: String,
    default: bool,
}

/// The printers in `LC_ALL=C lpstat -l -p -d`: each "printer QUEUE ..."
/// line, its "Description:" (the name people gave it, else the queue's
/// name), and the "system default destination:" line. The default first.
fn parse_printers(lpstat: &str) -> Vec<Printer> {
    let mut queues: Vec<(String, String)> = Vec::new();
    let mut default = None;
    for line in lpstat.lines() {
        if let Some(rest) = line.strip_prefix("printer ") {
            if let Some(queue) = rest.split_whitespace().next() {
                queues.push((queue.to_owned(), queue.replace('_', " ")));
            }
        } else if let Some(description) = line.trim().strip_prefix("Description:") {
            let description = description.trim();
            if let (Some(last), false) = (queues.last_mut(), description.is_empty()) {
                last.1 = description.to_owned();
            }
        } else if let Some(queue) = line.strip_prefix("system default destination:") {
            default = Some(queue.trim().to_owned());
        }
    }
    let mut printers: Vec<Printer> = queues
        .into_iter()
        .map(|(queue, name)| Printer {
            default: default.as_deref() == Some(queue.as_str()),
            name,
        })
        .collect();
    printers.sort_by_key(|printer| !printer.default);
    printers
}

fn printers() -> Vec<Printer> {
    Command::new("lpstat")
        .args(["-l", "-p", "-d"])
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|output| parse_printers(&String::from_utf8_lossy(&output.stdout)))
        .unwrap_or_default()
}

/// The key that signs NVIDIA's driver, when Secure Boot does not know it yet
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DriverKey {
    Missing,
    /// Queued: the firmware asks for it at the next restart
    Queued,
}

/// Every change a button makes
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Job {
    BatteryLimit(bool),
    Nvidia,
    DriverKey,
    Firmware,
    Fonts(bool),
    Vms(bool),
    FlatPark(bool),
    Dark(bool),
    ResetTheme,
    ResetConfig,
}

impl Job {
    /// The program that does it, and its arguments
    fn command(self) -> (&'static str, Vec<&'static str>) {
        let ujust = |args: &[&'static str]| ("ujust", args.to_vec());
        let on_off = |on: bool| if on { "on" } else { "off" };
        match self {
            Job::BatteryLimit(on) => ujust(&["toggle-battery-limit", on_off(on)]),
            Job::Nvidia => ("pkexec", vec![UPDATES_HELPER, "edition", "nvidia"]),
            Job::DriverKey => ("pkexec", vec![TOOLS_HELPER, "driver-key"]),
            Job::Firmware => ("systemctl", vec!["reboot", "--firmware-setup"]),
            Job::Fonts(true) => ujust(&["install-microsoft-fonts", "yes"]),
            Job::Fonts(false) => ujust(&["remove-microsoft-fonts"]),
            Job::Vms(true) => ujust(&["install-vms"]),
            Job::Vms(false) => ujust(&["remove-vms"]),
            Job::FlatPark(on) => ujust(&["toggle-flatpark", on_off(on)]),
            Job::Dark(true) => ujust(&["toggle-dark-mode", "dark"]),
            Job::Dark(false) => ujust(&["toggle-dark-mode", "light"]),
            Job::ResetTheme => ujust(&["reset-cosmic-theme", "yes"]),
            Job::ResetConfig => ujust(&["reset-cosmic-config", "yes"]),
        }
    }

    /// The title of its row, for the card when it does not work
    fn title(self) -> &'static str {
        match self {
            Job::BatteryLimit(_) => "Battery charge limit",
            Job::Nvidia | Job::DriverKey => "NVIDIA graphics card",
            Job::Firmware => "Firmware settings (BIOS/UEFI)",
            Job::Fonts(_) => "Microsoft fonts",
            Job::Vms(_) => "Virtual machines",
            Job::FlatPark(_) => "FlatPark",
            Job::Dark(_) => "Dark theme",
            Job::ResetTheme => "The Celenn OS look",
            Job::ResetConfig => "All COSMIC settings",
        }
    }

    /// The tool whose row it is
    fn part(self) -> Part {
        match self {
            Job::BatteryLimit(_) => Part::Battery,
            Job::Nvidia | Job::DriverKey => Part::Nvidia,
            Job::Firmware | Job::Vms(_) | Job::FlatPark(_) => Part::Advanced,
            Job::Fonts(_) => Part::Fonts,
            Job::Dark(_) => Part::Look,
            Job::ResetTheme | Job::ResetConfig => Part::Repair,
        }
    }

    /// Asked first, in the row: what happens
    fn question(self) -> Option<&'static str> {
        match self {
            Job::Nvidia => Some(
                "The NVIDIA edition is downloaded now and starts at the next restart. With Secure Boot on, a blue screen asks for the password celennos once.",
            ),
            Job::Firmware => Some("The computer restarts now. Save your work first."),
            Job::Fonts(true) => Some("Microsoft's license applies to these fonts."),
            Job::ResetConfig => Some(
                "Every COSMIC setting goes back to the defaults. A copy is kept in your settings folder.",
            ),
            _ => None,
        }
    }

    /// The button that confirms the question
    fn confirm(self) -> &'static str {
        match self {
            Job::Nvidia => "Switch",
            Job::Firmware => "Restart now",
            Job::Fonts(true) => "Accept and install",
            _ => "Reset",
        }
    }

    /// Said when it worked (the rows show the rest)
    fn done(self) -> Option<&'static str> {
        match self {
            Job::Nvidia => Some("The NVIDIA edition starts at the next restart."),
            Job::Fonts(true) => Some("Installed. Restart open apps to see them."),
            Job::ResetConfig => Some("Log out and back in to finish."),
            Job::ResetTheme => Some("Log out and back in to finish."),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    State(State),
    /// A button: asks first when the change needs it
    Ask(Job),
    Cancel,
    Run(Job),
    Done(Job, Outcome),
    OpenSettings,
    /// Print Settings (system-config-printer): add or set up a printer
    OpenPrinters,
    /// Document Scanner
    OpenScanner,
    /// One bar at the bottom (true), or the top bar and the dock
    Layout(bool),
    DockVisible(bool),
    FontsLicense,
    /// Open a category's list of apps (a number of CATEGORIES), or go back
    /// to the tiles (None)
    ListOpen(Option<usize>),
    Tick(usize, usize, bool),
    ListApply(usize),
    ListDone(usize, Outcome),
    Card(CardMessage),
    /// A toast for the window to show (tools.rs); its button (Undo, Show)
    /// sends its message back here
    Toast(ToastAsk<Message>),
    /// Change computers (more_transfer.rs)
    Transfer(TransferMessage),
    /// Other disks (more_disks.rs)
    Disks(DisksMessage),
}

/// What a card's "Try again" runs again
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Retry {
    Job(Job),
    List(usize),
}

pub struct Page {
    state: Option<State>,
    busy: Option<Job>,
    asking: Option<Job>,
    /// The category whose list is shown in place of the tiles, the apps
    /// ticked in each list, the list being applied
    open: Option<usize>,
    /// A list to open once the state is read (screenshot test)
    preview_list: Option<usize>,
    chosen: Vec<BTreeSet<usize>>,
    applying: Option<usize>,
    /// What did not work, until closed or done again
    card: Option<Card<Retry>>,
    /// Change computers: the settings file being saved, the dialog that
    /// takes one back and the file it shows
    saving: bool,
    restore: Option<transfer_view::Step>,
    restored: Option<(crate::transfer::Restore, BTreeSet<crate::transfer::Group>)>,
    /// Other disks: the switches and what Apply said
    disks: disks_view::DisksView,
}

fn output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default()
}

/// Printers and scanners: the printers added, and buttons to Print Settings
/// and Document Scanner (COSMIC Settings has no printer page)
fn printers_row<'a>(state: &State) -> Element<'a, Message> {
    let caption = if state.printers.is_empty() {
        tr("No printer added yet. Recent printers on the network or plugged in by USB show up by themselves in the print window.").to_owned()
    } else {
        state
            .printers
            .iter()
            .map(|printer| {
                if printer.default {
                    fill(tr("{} (default)"), &printer.name)
                } else {
                    printer.name.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut buttons = widget::row::with_capacity(2)
        .spacing(theme::spacing().space_xxs)
        .push(widget::button::standard(tr("Printers")).on_press(Message::OpenPrinters));
    if state.scanner {
        buttons = buttons.push(widget::button::standard(tr("Scan")).on_press(Message::OpenScanner));
    }
    settings::item_row(vec![
        state_icon("printer-symbolic", false),
        label(tr("Printers and scanners"), caption),
        buttons.into(),
    ])
    .spacing(theme::spacing().space_s)
    .into()
}

/// A row that only tells: its symbol, its title, what was found
fn info_row<'a>(icon: &'static str, title: &'static str, value: String) -> Element<'a, Message> {
    settings::item_row(vec![state_icon(icon, false), label(tr(title), value)])
        .spacing(theme::spacing().space_s)
        .into()
}

/// With Secure Boot on, whether the key that signs NVIDIA's driver is
/// enrolled (mokutil reads the firmware's lists, no root needed)
fn driver_key() -> Option<DriverKey> {
    const KEY: &str = "/etc/pki/akmods/certs/akmods-ublue.der";
    if !Path::new(KEY).exists()
        || !output("mokutil", &["--sb-state"]).contains("SecureBoot enabled")
    {
        return None;
    }
    if output("mokutil", &["--test-key", KEY]).contains("already enrolled") {
        return None;
    }
    // Queued for the next restart: listed among the keys to enroll
    let queued = Command::new("mokutil")
        .args(["--list-new"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    Some(if queued {
        DriverKey::Queued
    } else {
        DriverKey::Missing
    })
}

/// The COSMIC dark theme setting: the person's, else the system default
fn dark() -> bool {
    let key = "cosmic/com.system76.CosmicTheme.Mode/v1/is_dark";
    let config = crate::housekeeping::xdg("XDG_CONFIG_HOME", ".config");
    std::fs::read_to_string(config.join(key))
        .or_else(|_| std::fs::read_to_string(Path::new("/usr/share").join(key)))
        .map_or(true, |value| value.trim() == "true")
}

pub fn read_state() -> State {
    // `ujust toggle-battery-limit status` says whether the limit is on, or
    // why there is none
    let battery = output("ujust", &["toggle-battery-limit", "status"]);
    let battery_limit = if battery.contains("Battery limit on") {
        Some(true)
    } else if battery.contains("Battery limit off") {
        Some(false)
    } else {
        None
    };
    let apps = output(
        "flatpak",
        &["list", "--system", "--app", "--columns=application"],
    );
    let has = |id: &str| apps.lines().any(|line| line.trim() == id);
    let data = crate::housekeeping::xdg("XDG_DATA_HOME", ".local/share");
    let nvidia = crate::edition::nvidia();
    State {
        printers: printers(),
        disks: crate::disks::read(),
        scanner: has(SCANNER),
        battery_limit,
        nvidia,
        driver_key: if nvidia == Some(true) {
            driver_key()
        } else {
            None
        },
        efi: Path::new("/sys/firmware/efi").exists(),
        fonts: data.join("fonts/microsoft-core-fonts").is_dir(),
        vms: has(VMS),
        flatpark: output("flatpak", &["remotes", "--user", "--columns=name"])
            .lines()
            .any(|line| line.trim() == "flatpark"),
        dark: dark(),
        bottom_bar: crate::layout::bottom_bar(),
        dock_visible: crate::layout::dock_always_visible(),
        installed: CATEGORIES
            .iter()
            .enumerate()
            .map(|(n, category)| {
                let mut installed = if n == DEVELOPMENT {
                    devtools::brew_installed()
                } else {
                    BTreeSet::new()
                };
                installed.extend(category.tools().enumerate().filter_map(
                    |(i, tool)| match tool.kind {
                        ToolKind::Flatpak(app) if has(app) => Some(i),
                        _ => None,
                    },
                ));
                installed
            })
            .collect(),
        brew: Path::new(devtools::BREW).exists(),
    }
}

fn refresh() -> Task<Message> {
    Task::perform(
        async {
            tokio::task::spawn_blocking(read_state)
                .await
                .unwrap_or_default()
        },
        |state| cosmic::Action::App(Message::State(state)),
    )
}

impl Page {
    pub fn new() -> (Self, Task<Message>) {
        let page = Page {
            state: None,
            busy: None,
            asking: None,
            open: None,
            chosen: vec![BTreeSet::new(); CATEGORIES.len()],
            applying: None,
            card: None,
            preview_list: crate::common::preview("developer-tools").then_some(DEVELOPMENT),
            saving: false,
            restore: None,
            restored: None,
            disks: Default::default(),
        };
        // `celenn-tools --restore` (the tour's "Bring your things"):
        // the file chooser at once
        let restore = if std::env::args().any(|arg| arg == "--restore") {
            Task::done(cosmic::Action::App(Message::Transfer(
                TransferMessage::Open,
            )))
        } else {
            Task::none()
        };
        (page, Task::batch([refresh(), restore]))
    }

    /// A change of the desktop's look, said with a button to take it back
    fn undo_toast(&mut self, message: &'static str, undo: Message) -> Task<Message> {
        crate::common::toast(
            tr(message).to_owned(),
            Some((tr("Undo").to_owned(), undo)),
            Message::Toast,
        )
    }

    fn toast(&mut self, message: &'static str) -> Task<Message> {
        crate::common::toast(tr(message).to_owned(), None, Message::Toast)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::State(state) => {
                self.disks_reread(&state.disks);
                self.state = Some(state);
                if let Some(list) = self.preview_list.take() {
                    return self.update(Message::ListOpen(Some(list)));
                }
            }
            Message::Ask(job) => {
                if job.question().is_some() {
                    self.asking = Some(job);
                } else {
                    return self.update(Message::Run(job));
                }
            }
            Message::Cancel => self.asking = None,
            Message::Run(job) => {
                self.asking = None;
                self.card = None;
                self.busy = Some(job);
                // The switch and the theme show the change at once
                if let (Job::Dark(on), Some(state)) = (job, self.state.as_mut()) {
                    state.dark = on;
                }
                let (program, args) = job.command();
                let args = args.into_iter().map(str::to_owned).collect();
                return Task::perform(failure::run(program, args, None), move |result| {
                    cosmic::Action::App(Message::Done(job, result))
                });
            }
            Message::Done(job, result) => {
                self.busy = None;
                let toast = match result {
                    None => Task::none(),
                    Some(Err(failure)) => {
                        self.card = Some(Card::new(tr(job.title()), failure, Retry::Job(job)));
                        Task::none()
                    }
                    Some(Ok(())) => match (job, job.done()) {
                        (Job::Dark(on), _) => self.undo_toast(
                            if on { "Dark theme" } else { "Light theme" },
                            Message::Run(Job::Dark(!on)),
                        ),
                        (_, Some(text)) => self.toast(text),
                        (_, None) => Task::none(),
                    },
                };
                return Task::batch([toast, refresh()]);
            }
            Message::OpenSettings => spawn("cosmic-settings", &["appearance"]),
            Message::OpenPrinters => spawn("system-config-printer", &[]),
            Message::OpenScanner => spawn("flatpak", &["run", SCANNER]),
            Message::Layout(bottom) => {
                if !crate::layout::set_bottom_bar(bottom) {
                    return self.toast("This did not work.");
                }
                if let Some(state) = &mut self.state {
                    state.bottom_bar = bottom;
                }
                return self.undo_toast(
                    if bottom {
                        "Bar at the bottom"
                    } else {
                        "Top bar and dock"
                    },
                    Message::Layout(!bottom),
                );
            }
            Message::DockVisible(visible) => {
                if !crate::layout::set_dock_always_visible(visible) {
                    return self.toast("This did not work.");
                }
                if let Some(state) = &mut self.state {
                    state.dock_visible = visible;
                }
                return self.undo_toast(
                    if visible {
                        "Dock always visible"
                    } else {
                        "The dock hides when a window covers it."
                    },
                    Message::DockVisible(!visible),
                );
            }
            // The window shows it (tools.rs)
            Message::Toast(_) => {}
            Message::FontsLicense => spawn("xdg-open", &[FONTS_LICENSE]),
            Message::ListOpen(open) => {
                self.open = open;
                if let (Some(n), Some(state)) = (open, &self.state) {
                    self.chosen[n] = state.installed[n].clone();
                    // Nothing installed yet: the recommended apps ticked
                    if self.chosen[n].is_empty() {
                        self.chosen[n] = CATEGORIES[n].recommended();
                    }
                }
            }
            Message::Tick(list, i, on) => {
                let chosen = &mut self.chosen[list];
                if on {
                    chosen.insert(i);
                } else {
                    chosen.remove(&i);
                }
            }
            Message::ListApply(list) => {
                let Some(state) = &self.state else {
                    return Task::none();
                };
                let n = list;
                self.applying = Some(list);
                self.card = None;
                return Task::perform(
                    apply_list(list, state.installed[n].clone(), self.chosen[n].clone()),
                    move |result| cosmic::Action::App(Message::ListDone(list, result)),
                );
            }
            Message::ListDone(list, result) => {
                self.applying = None;
                let toast = match result {
                    None => Task::none(),
                    Some(Err(failure)) => {
                        self.card = Some(Card::new(
                            tr(CATEGORIES[list].title),
                            failure,
                            Retry::List(list),
                        ));
                        Task::none()
                    }
                    Some(Ok(())) => {
                        let n = list;
                        self.open = None;
                        // Bitwarden just installed: its Firefox extension
                        // too (devtools.rs)
                        if list == PASSWORDS
                            && let Some(state) = &self.state
                        {
                            let bitwarden = CATEGORIES[n].tools().position(|tool| {
                                tool.kind == ToolKind::Flatpak(devtools::BITWARDEN)
                            });
                            if bitwarden.is_some_and(|i| {
                                self.chosen[n].contains(&i) && !state.installed[n].contains(&i)
                            }) {
                                let command = devtools::BITWARDEN_EXTENSION;
                                spawn(command[0], &command[1..]);
                            }
                        }
                        self.toast(CATEGORIES[n].done)
                    }
                };
                return Task::batch([toast, refresh()]);
            }
            Message::Card(message) => {
                let Some(card) = &mut self.card else {
                    return Task::none();
                };
                match card.update(message) {
                    CardAction::None => {}
                    CardAction::Close => self.card = None,
                    CardAction::Retry(Retry::Job(job)) => return self.update(Message::Run(job)),
                    CardAction::Retry(Retry::List(list)) => {
                        return self.update(Message::ListApply(list));
                    }
                }
            }
            Message::Transfer(message) => return self.transfer_update(message),
            Message::Disks(message) => return self.disks_update(message),
        }
        Task::none()
    }

    /// The button of a row: a spinner while its change runs, else the
    /// button (disabled while another change runs)
    fn button<'a>(&self, job: Job, name: &'static str, suggested: bool) -> Element<'a, Message> {
        if self.busy == Some(job) {
            return widget::row::with_capacity(2)
                .spacing(theme::spacing().space_xxs)
                .align_y(Alignment::Center)
                .push(widget::progress_bar::indeterminate_circular().size(20.0))
                .push(text::body(tr("Working…")))
                .into();
        }
        let message = self.busy.is_none().then_some(Message::Ask(job));
        if suggested {
            widget::button::suggested(tr(name))
                .on_press_maybe(message)
                .into()
        } else {
            widget::button::standard(tr(name))
                .on_press_maybe(message)
                .into()
        }
    }

    /// Install or Remove, after what is there
    fn install<'a>(&self, installed: bool, job: fn(bool) -> Job) -> Element<'a, Message> {
        if installed {
            self.button(job(false), "Remove", false)
        } else {
            self.button(job(true), "Install", false)
        }
    }

    fn toggler<'a>(&self, on: bool, job: fn(bool) -> Job) -> Element<'a, Message> {
        let toggler = widget::toggler(on);
        if self.busy.is_none() {
            toggler.on_toggle(move |on| Message::Run(job(on))).into()
        } else {
            toggler.into()
        }
    }

    /// A row: its symbol, its text, its button. While its change waits for
    /// an answer, the question and Cancel / confirm under it
    fn row<'a>(
        &self,
        icon: &'static str,
        title: &'static str,
        caption: &'static str,
        control: Element<'a, Message>,
        job: Option<Job>,
    ) -> Element<'a, Message> {
        let space_s = theme::spacing().space_s;
        let main = settings::item_row(vec![
            state_icon(icon, false),
            label(tr(title), tr(caption).to_owned()),
            control,
        ])
        .spacing(space_s);
        let Some(job) = job.filter(|job| self.asking == Some(*job)) else {
            return main.into();
        };
        let mut actions = widget::row::with_capacity(4)
            .spacing(theme::spacing().space_xxs)
            .align_y(Alignment::Center)
            .push(text::caption(tr(job.question().unwrap_or_default())).width(Length::Fill));
        if job == Job::Fonts(true) {
            actions = actions.push(
                widget::button::link(tr("Read it"))
                    .trailing_icon(true)
                    .on_press(Message::FontsLicense),
            );
        }
        actions = actions
            .push(widget::button::standard(tr("Cancel")).on_press(Message::Cancel))
            .push(if job == Job::ResetConfig {
                // Settings lost (a copy kept): the warning style
                widget::button::destructive(tr(job.confirm())).on_press(Message::Run(job))
            } else {
                widget::button::suggested(tr(job.confirm())).on_press(Message::Run(job))
            });
        widget::column::with_capacity(2)
            .push(main)
            .push(widget::container(actions).padding([0, space_s, space_s, space_s + 36]))
            .into()
    }

    /// One tool (tools.rs gives it its title and frame); the card of a
    /// change that did not work above it
    pub fn view(&self, part: Part) -> Element<'_, Message> {
        let cosmic_theme::Spacing { space_s, .. } = theme::spacing();
        let mut content = widget::column::with_capacity(3)
            .spacing(space_s)
            .align_x(Alignment::Center);
        let card_part = self.card.as_ref().map(|card| match card.job {
            Retry::Job(job) => job.part(),
            Retry::List(_) => Part::Apps,
        });
        if let Some(card) = self.card.as_ref().filter(|_| card_part == Some(part)) {
            content = content.push(card.view(Message::Card));
        }
        let Some(state) = &self.state else {
            return content.into();
        };
        fn section(row: Element<'_, Message>) -> Element<'_, Message> {
            settings::section().add(row).into()
        }
        content
            .push(match part {
                Part::Print => section(printers_row(state)),
                Part::Inside => self.inside(state),
                Part::Battery => section(self.battery(state)),
                Part::Disks => self.disks_list(&state.disks),
                Part::Look => self.look(state),
                Part::Nvidia => self.nvidia(state),
                Part::Fonts => section(self.row(
                    "font-x-generic-symbolic",
                    "Microsoft fonts",
                    "Arial, Times New Roman, Verdana… for documents made on Windows.",
                    self.install(state.fonts, Job::Fonts),
                    Some(Job::Fonts(true)),
                )),
                Part::Apps => self.software(state),
                Part::Transfer => section(self.transfer_row()),
                Part::Repair => self.repair(),
                Part::Advanced => self.advanced(state),
            })
            .into()
    }

    /// What the toolbox's tile says about a tool (tools.rs)
    pub fn summary(&self, part: Part) -> Summary {
        let Some(state) = &self.state else {
            return Summary::Unknown;
        };
        match part {
            Part::Print => match state.printers.iter().find(|printer| printer.default) {
                _ if state.printers.is_empty() => {
                    Summary::Says(tr("No printer added yet.").to_owned())
                }
                Some(printer) => Summary::Says(printer.name.clone()),
                None => Summary::Says(state.printers[0].name.clone()),
            },
            Part::Inside => {
                let hw = crate::hardware::hardware();
                let model = match (&hw.virtual_machine, hw.model.is_empty()) {
                    (Some(hypervisor), _) => fill(tr("Virtual machine ({})"), hypervisor),
                    (None, false) => hw.model.clone(),
                    (None, true) => tr("Unknown").to_owned(),
                };
                if hw.memory_gb > 0 {
                    // Memory first: a long model name is cut at the end
                    Summary::Says(format!(
                        "{}, {model}",
                        fill(tr("{} GB of memory"), &hw.memory_gb.to_string())
                    ))
                } else {
                    Summary::Says(model)
                }
            }
            Part::Battery => match state.battery_limit {
                None => Summary::Hidden,
                Some(true) => Summary::Says(tr("Charging stops at 80 %.").to_owned()),
                Some(false) => Summary::Says(tr("Off: charges to 100 %.").to_owned()),
            },
            Part::Disks => {
                if state.disks.is_empty() {
                    Summary::Hidden
                } else if state.disks.iter().any(crate::disks::Disk::problem) {
                    Summary::Warns(crate::disks::summary(&state.disks))
                } else if state.disks.iter().any(crate::disks::Disk::at_startup) {
                    Summary::Says(crate::disks::summary(&state.disks))
                } else {
                    Summary::Says(tr("None opens at startup yet.").to_owned())
                }
            }
            Part::Look => Summary::Says(
                tr(if state.dark {
                    "Dark theme"
                } else {
                    "Light theme"
                })
                .to_owned(),
            ),
            Part::Nvidia => match (state.nvidia, state.driver_key) {
                (None, _) => Summary::Hidden,
                (Some(true), Some(DriverKey::Missing)) => {
                    Summary::Warns(tr("Its driver waits for your OK.").to_owned())
                }
                (Some(true), _) => Summary::Says(tr("NVIDIA's own driver.").to_owned()),
                (Some(false), _) => Summary::Says(tr("The open Linux driver.").to_owned()),
            },
            Part::Fonts => Summary::Says(
                tr(if state.fonts {
                    "Installed."
                } else {
                    "Arial, Times New Roman, Verdana…"
                })
                .to_owned(),
            ),
            Part::Apps => Summary::Says(tr("Office, games, school…").to_owned()),
            Part::Transfer => Summary::Says(tr("Your settings and apps").to_owned()),
            Part::Repair | Part::Advanced => Summary::Unknown,
        }
    }

    /// The tool opened (tools.rs): lists show what is installed now
    pub fn opened(&mut self, part: Part) -> Task<Message> {
        match part {
            Part::Disks => self.update(Message::Disks(DisksMessage::Open)),
            Part::Apps => self.update(Message::ListOpen(None)),
            _ => Task::none(),
        }
    }
}

/// Installs the apps ticked and removes the ones unticked, as the first
/// steps tour installs developer tools (devtools.rs): Homebrew for the user,
/// Flathub for everyone. None: the password prompt was dismissed; the
/// last failure otherwise
pub(crate) async fn apply_list(
    list: usize,
    installed: BTreeSet<usize>,
    chosen: BTreeSet<usize>,
) -> Outcome {
    let (mut formulas, mut casks, mut apps) = (Vec::new(), Vec::new(), Vec::new());
    let (mut old_formulas, mut old_casks, mut old_apps) = (Vec::new(), Vec::new(), Vec::new());
    for (i, tool) in CATEGORIES[list].tools().enumerate() {
        let add = chosen.contains(&i) && !installed.contains(&i);
        let remove = !chosen.contains(&i) && installed.contains(&i);
        if !add && !remove {
            continue;
        }
        match tool.kind {
            ToolKind::Formula(names) if add => formulas.extend(names),
            ToolKind::Formula(names) => old_formulas.push(names[0]),
            ToolKind::Cask(name, tap) if add => casks.push(devtools::cask_name(name, tap)),
            ToolKind::Cask(name, tap) => old_casks.push(devtools::cask_name(name, tap)),
            ToolKind::Flatpak(app) if add => apps.push(app.to_owned()),
            ToolKind::Flatpak(app) => old_apps.push(app.to_owned()),
        }
    }
    let mut result = Ok(());
    if (!formulas.is_empty() || !casks.is_empty())
        && let Err(error) = devtools::brew_install(formulas, casks).await
    {
        result = Err(failure::failure(&error));
    }
    if (!old_formulas.is_empty() || !old_casks.is_empty())
        && let Err(error) = devtools::brew_remove(old_formulas, old_casks).await
    {
        result = Err(failure::failure(&error));
    }
    for (verb, apps) in [("install", apps), ("uninstall", old_apps)] {
        if apps.is_empty() {
            continue;
        }
        let mut args: Vec<String> = [verb, "--system", "--noninteractive", "-y"]
            .map(str::to_owned)
            .to_vec();
        if verb == "install" {
            args.push("flathub".to_owned());
        }
        args.extend(apps);
        match failure::run("flatpak", args, None).await {
            Some(Ok(())) => {}
            Some(Err(failure)) => result = Err(failure),
            None => return None,
        }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printers() {
        assert!(parse_printers("").is_empty());
        assert!(parse_printers("no system default destination\n").is_empty());
        let lpstat = "printer Brother_HL_L2350DW is idle.  enabled since Wed 25 Nov 2026\n\
            \tForm mounts:\n\
            \tDescription: Brother HL-L2350DW\n\
            \tLocation: Office\n\
            printer HP_LaserJet_M110w is idle.  enabled since Wed 25 Nov 2026\n\
            \tDescription: HP LaserJet M110w\n\
            printer Label is idle.  enabled since Wed 25 Nov 2026\n\
            \tDescription:\n\
            system default destination: HP_LaserJet_M110w\n";
        assert_eq!(
            parse_printers(lpstat),
            vec![
                Printer {
                    name: "HP LaserJet M110w".into(),
                    default: true
                },
                Printer {
                    name: "Brother HL-L2350DW".into(),
                    default: false
                },
                Printer {
                    name: "Label".into(),
                    default: false
                },
            ]
        );
    }
}
