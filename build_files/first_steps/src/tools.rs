// Celenn OS Tools: what people would otherwise do with commands (`ujust`,
// bootc...), as a toolbox. The first page shows where things stand in one
// line, a search, and the tools as tiles named after what people want to
// do, each saying how things are (overview.rs). A tile opens its tool: a
// short header (its symbol, its name, what it does, a "?" that opens its
// part of the documentation), its rows, then at the bottom the way back to
// the toolbox and, for experienced users, the matching command. The tools
// follow the first steps tour's words: what the tour shows once, the
// toolbox keeps, under the same name.
//
//   Every day              Update, Install apps, Web apps (webapps.rs),
//                          Free up space, Protect your files, Print and
//                          scan, Change computers
//   My computer            What's inside, Spare the battery, Other disks,
//                          Make it yours, NVIDIA driver, Microsoft fonts
//   When something is wrong  Go back, Repair, Get help
//   For experienced users  (a button under the tiles) the update channel,
//                          an older version, the firmware settings, virtual
//                          machines, FlatPark
//
// The tools come from the pages that hold their code and state:
// updates_page.rs, maintenance.rs, more.rs (more_disks.rs,
// more_transfer.rs), help.rs and overview.rs; a tool may show parts of
// several. The window keeps one toaster for all of them.
// Each button runs the same program as the matching `ujust` command, so the
// terminal and the window do the same thing. Changes that need root go
// through small helpers run with pkexec (an administrator password).
// No notifications about updates: the system looks after itself, this is
// for when people want to look or act. The one notification is a disk almost
// full (housekeeping.rs), which people have to act on.
//
//   celenn-tools               the window, on the toolbox
//   celenn-tools --tool NAME   the window, on a tool (Tool::name)
//   celenn-tools --updates     the window, on Update (also when run as
//                                   /usr/libexec/celenn-updates)
//   celenn-tools --maintenance / --computer / --software /
//                     --appearance / --help-page   the tools that replace
//                                   the pages of the same names
//   celenn-tools --restore     the window, on Change computers,
//                                   choosing a settings file to take back
//                                   (transfer.rs)
//   celenn-tools --housekeeping  empties the old trash, warns when the
//                                   disk is almost full (no window; run by
//                                   celenn-housekeeping.timer)
//   celenn-tools --status / --older   see updates_page.rs (tests)
//   celenn-tools --check       image build test: every text has its
//                                   translations (no window)
//
// Text is in English, translated from the PO files of po/tools/.

use common::{Summary, ToastAsk, dim_caption, fill};
use cosmic::app::{Core, Settings, Task};
use cosmic::iced::core::text::Wrapping;
use cosmic::iced::{Alignment, Length, Limits};
use cosmic::widget::{self, text, toaster::Toasts};
use cosmic::{Application, ApplicationExt, Apply, Element, cosmic_theme, executor, theme};

/// The first steps tour, opened again from the toolbox
const TOUR: &str = "/usr/libexec/celenn-tour";

mod apps;
mod common;
mod devtools;
mod disks;
mod edition;
mod failure;
mod hardware;
mod help;
mod housekeeping;
mod i18n;
mod layout;
mod maintenance;
mod more;
mod network;
mod overview;
mod process;
mod style;
// What the tour reads from the system; Tools uses the settings folder and
// the first-boot apps (transfer.rs)
#[allow(dead_code)]
mod system;
mod transfer;
mod units;
mod update_progress;
mod updates_page;
mod webapps;
use i18n::tr;

pub(crate) const LANGUAGES: &[(&str, &str)] = &[
    ("de", include_str!("../po/tools/de.po")),
    ("es", include_str!("../po/tools/es.po")),
    ("fr", include_str!("../po/tools/fr.po")),
    ("it", include_str!("../po/tools/it.po")),
    ("pt_BR", include_str!("../po/tools/pt_BR.po")),
];
pub(crate) const TEMPLATE: &str = include_str!("../po/tools/celenn-tools.pot");
pub(crate) const PO_DIR: &str = "po/tools";
pub(crate) const TEMPLATE_NAME: &str = "celenn-tools.pot";

/// The window's own texts and the tools'; each page lists its own
const TEXTS: &[&str] = &[
    "Celenn OS Tools",
    "Some changes ask for the password of a parent or an administrator.",
    "Toolbox",
    "Every day",
    "My computer",
    "When something is wrong",
    "For experienced users…",
    "First steps again",
    "Help on this tool",
    "In a terminal:",
    "Copy",
    "Update",
    "Celenn OS updates itself in the background. A new version starts at the next restart, without interrupting you.",
    "The system, the apps and the devices",
    "Install apps",
    "Ready in one click. In each list, the apps most used on Linux are already ticked.",
    "Office, games, school, development…",
    "Web apps",
    "Your favourite sites in their own window, with their icon in your apps and the dock.",
    "WhatsApp, Netflix, Gmail…",
    "Free up space",
    "Remove what nothing uses any more. Your files stay as they are.",
    "Disk space and the trash",
    "Protect your files",
    "A copy of your files on a USB drive or another disk, made by itself every day.",
    "A copy of your files",
    "Print and scan",
    "Printers on the network or plugged in by USB show up by themselves. Add one or set it up here.",
    "Printers and scanners",
    "Change computers",
    "Your settings and the list of your apps in a file, to take back on another Celenn OS computer.",
    "Your settings and apps, in one file.",
    "What's inside",
    "What this computer is made of, and how it starts.",
    "Processor, memory, graphics card",
    "Spare the battery",
    "Charging stops at 80 %: the battery lasts longer when the laptop stays plugged in.",
    "The battery charge limit",
    "Other disks",
    "Turn on the ones you want open at every startup, without clicking them in Files.",
    "Disks open at startup",
    "Make it yours",
    "Dark or light, and the layout of the desktop.",
    "Theme and layout",
    "NVIDIA driver",
    "Use the NVIDIA edition of Celenn OS, with NVIDIA's own driver, for games and graphics apps.",
    "The graphics card's driver",
    "Microsoft fonts",
    "Arial, Times New Roman, Verdana… for documents made on Windows.",
    "Arial, Times New Roman, Verdana…",
    "Installed.",
    "Go back",
    "Something wrong since the last update? Go back to the previous version. Your files and settings stay as they are.",
    "To the previous version",
    "Repair",
    "Put back what is missing: the apps that come with Celenn OS, the Celenn OS look, your COSMIC settings.",
    "Apps, look, settings",
    "Get help",
    "Find answers, ask a question, or tell us what goes wrong.",
    "Guide, questions, report a problem",
    "For experienced users",
    "Useful when you know what you are doing. Each one is also a ujust command.",
    "Channel, versions, firmware, virtual machines",
];

/// A tool: a tile on the toolbox, then its own page
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Update,
    Apps,
    WebApps,
    Space,
    Backup,
    Print,
    Transfer,
    Inside,
    Battery,
    Disks,
    Look,
    Nvidia,
    Fonts,
    Rollback,
    Repair,
    Help,
    Advanced,
}

/// The toolbox's groups of tiles, in order (Advanced: a button under them)
pub const GROUPS: &[(&str, &[Tool])] = &[
    (
        "Every day",
        &[
            Tool::Update,
            Tool::Apps,
            Tool::WebApps,
            Tool::Space,
            Tool::Backup,
            Tool::Print,
        ],
    ),
    (
        "My computer",
        &[
            Tool::Inside,
            Tool::Transfer,
            Tool::Battery,
            Tool::Disks,
            Tool::Look,
            Tool::Nvidia,
            Tool::Fonts,
        ],
    ),
    (
        "When something is wrong",
        &[Tool::Rollback, Tool::Repair, Tool::Help],
    ),
];

impl Tool {
    pub const ALL: [Tool; 17] = [
        Tool::Update,
        Tool::Apps,
        Tool::WebApps,
        Tool::Space,
        Tool::Backup,
        Tool::Print,
        Tool::Transfer,
        Tool::Inside,
        Tool::Battery,
        Tool::Disks,
        Tool::Look,
        Tool::Nvidia,
        Tool::Fonts,
        Tool::Rollback,
        Tool::Repair,
        Tool::Help,
        Tool::Advanced,
    ];

    /// Its name on the command line (--tool NAME)
    pub fn name(self) -> &'static str {
        match self {
            Tool::Update => "update",
            Tool::Apps => "apps",
            Tool::WebApps => "web-apps",
            Tool::Space => "space",
            Tool::Backup => "backup",
            Tool::Print => "print",
            Tool::Transfer => "transfer",
            Tool::Inside => "inside",
            Tool::Battery => "battery",
            Tool::Disks => "disks",
            Tool::Look => "look",
            Tool::Nvidia => "nvidia",
            Tool::Fonts => "fonts",
            Tool::Rollback => "rollback",
            Tool::Repair => "repair",
            Tool::Help => "help",
            Tool::Advanced => "advanced",
        }
    }

    /// Its title, what it does (under the title), and its tile's caption
    /// until its state is known. The first steps tour's titles where it
    /// shows the same thing (Protect your files, Make it yours).
    pub fn texts(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Tool::Update => (
                "Update",
                "Celenn OS updates itself in the background. A new version starts at the next restart, without interrupting you.",
                "The system, the apps and the devices",
            ),
            Tool::Apps => (
                "Install apps",
                "Ready in one click. In each list, the apps most used on Linux are already ticked.",
                "Office, games, school, development…",
            ),
            Tool::WebApps => (
                "Web apps",
                "Your favourite sites in their own window, with their icon in your apps and the dock.",
                "WhatsApp, Netflix, Gmail…",
            ),
            Tool::Space => (
                "Free up space",
                "Remove what nothing uses any more. Your files stay as they are.",
                "Disk space and the trash",
            ),
            Tool::Backup => (
                "Protect your files",
                "A copy of your files on a USB drive or another disk, made by itself every day.",
                "A copy of your files",
            ),
            Tool::Print => (
                "Print and scan",
                "Printers on the network or plugged in by USB show up by themselves. Add one or set it up here.",
                "Printers and scanners",
            ),
            Tool::Transfer => (
                "Change computers",
                "Your settings and the list of your apps in a file, to take back on another Celenn OS computer.",
                "Your settings and apps, in one file.",
            ),
            Tool::Inside => (
                "What's inside",
                "What this computer is made of, and how it starts.",
                "Processor, memory, graphics card",
            ),
            Tool::Battery => (
                "Spare the battery",
                "Charging stops at 80 %: the battery lasts longer when the laptop stays plugged in.",
                "The battery charge limit",
            ),
            Tool::Disks => (
                "Other disks",
                "Turn on the ones you want open at every startup, without clicking them in Files.",
                "Disks open at startup",
            ),
            Tool::Look => (
                "Make it yours",
                "Dark or light, and the layout of the desktop.",
                "Theme and layout",
            ),
            Tool::Nvidia => (
                "NVIDIA driver",
                "Use the NVIDIA edition of Celenn OS, with NVIDIA's own driver, for games and graphics apps.",
                "The graphics card's driver",
            ),
            Tool::Fonts => (
                "Microsoft fonts",
                "Arial, Times New Roman, Verdana… for documents made on Windows.",
                "Arial, Times New Roman, Verdana…",
            ),
            Tool::Rollback => (
                "Go back",
                "Something wrong since the last update? Go back to the previous version. Your files and settings stay as they are.",
                "To the previous version",
            ),
            Tool::Repair => (
                "Repair",
                "Put back what is missing: the apps that come with Celenn OS, the Celenn OS look, your COSMIC settings.",
                "Apps, look, settings",
            ),
            Tool::Help => (
                "Get help",
                "Find answers, ask a question, or tell us what goes wrong.",
                "Guide, questions, report a problem",
            ),
            Tool::Advanced => (
                "For experienced users",
                "Useful when you know what you are doing. Each one is also a ujust command.",
                "Channel, versions, firmware, virtual machines",
            ),
        }
    }

    pub fn title(self) -> &'static str {
        self.texts().0
    }

    /// Its symbol, on its tile and at the top of its page
    pub fn icon(self) -> &'static str {
        match self {
            Tool::Update => "software-update-available-symbolic",
            Tool::Apps => "system-software-install-symbolic",
            Tool::WebApps => "web-browser-symbolic",
            Tool::Space => "edit-clear-all-symbolic",
            Tool::Backup => "document-save-symbolic",
            Tool::Print => "printer-symbolic",
            Tool::Transfer => "document-send-symbolic",
            Tool::Inside => "computer-symbolic",
            Tool::Battery => "battery-symbolic",
            Tool::Disks => "drive-multidisk-symbolic",
            Tool::Look => "preferences-appearance-symbolic",
            Tool::Nvidia => "video-display-symbolic",
            Tool::Fonts => "font-symbolic",
            Tool::Rollback => "edit-undo-symbolic",
            Tool::Repair => "emblem-system-symbolic",
            Tool::Help => "help-browser-symbolic",
            Tool::Advanced => "preferences-other-symbolic",
        }
    }

    /// Its part of the documentation on the site (the "?" button)
    pub fn docs(self) -> &'static str {
        match self {
            Tool::Update => "docs/#updates",
            Tool::Rollback => "docs/troubleshooting/#bad-update",
            Tool::Apps | Tool::WebApps | Tool::Fonts => "docs/#apps",
            Tool::Space => "docs/#tools",
            Tool::Repair => "docs/troubleshooting/#settings",
            Tool::Backup | Tool::Transfer => "docs/#data",
            Tool::Print | Tool::Inside | Tool::Battery | Tool::Disks | Tool::Nvidia => {
                "docs/#hardware"
            }
            Tool::Look => "docs/#look",
            Tool::Help => "docs/#help",
            Tool::Advanced => "docs/#ujust",
        }
    }

    /// The command that does the same, for experienced users
    pub fn command(self) -> Option<&'static str> {
        match self {
            Tool::Space => Some("ujust clean-system"),
            Tool::Battery => Some("ujust toggle-battery-limit"),
            Tool::Look => Some("ujust toggle-dark-mode"),
            Tool::Fonts => Some("ujust install-microsoft-fonts"),
            Tool::Nvidia => Some("ujust enroll-secure-boot-key"),
            Tool::Rollback => Some("sudo bootc rollback"),
            Tool::Repair => Some("ujust reset-cosmic-theme"),
            Tool::Help => Some("ujust report-bug"),
            Tool::Advanced => Some("ujust"),
            _ => None,
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let options = &args[1..];
    if options.iter().any(|arg| arg == "--check") {
        let texts: Vec<&str> = [
            TEXTS,
            updates_page::TEXTS,
            update_progress::TEXTS,
            units::TEXTS,
            maintenance::TEXTS,
            more::TEXTS,
            help::TEXTS,
            housekeeping::TEXTS,
            failure::TEXTS,
            overview::TEXTS,
            transfer::TEXTS,
            disks::TEXTS,
            webapps::TEXTS,
        ]
        .concat();
        let mut sorted = texts.clone();
        sorted.extend(apps::texts());
        sorted.sort_unstable();
        sorted.dedup();
        std::process::exit(if i18n::check(&sorted) { 0 } else { 1 });
    }
    if options.iter().any(|arg| arg == "--housekeeping") {
        std::process::exit(housekeeping::run());
    }
    if let Some(code) = updates_page::command_line(options) {
        std::process::exit(code);
    }
    let settings = Settings::default()
        .size(cosmic::iced::Size::new(1000.0, 760.0))
        .size_limits(Limits::NONE.min_width(520.0).min_height(480.0));
    cosmic::app::run::<App>(settings, first_tool(&args))?;
    Ok(())
}

/// The tool the window opens on (None: the toolbox), from its options
fn first_tool(args: &[String]) -> Option<Tool> {
    // Run as /usr/libexec/celenn-updates (a link): Update
    if args
        .first()
        .is_some_and(|name| name.ends_with("celenn-updates"))
    {
        return Some(Tool::Update);
    }
    let options = &args[1..];
    if let Some(name) = options
        .iter()
        .position(|arg| arg == "--tool")
        .and_then(|at| options.get(at + 1))
    {
        return Tool::ALL.into_iter().find(|tool| tool.name() == name);
    }
    // The options of the pages before the toolbox
    options.iter().find_map(|arg| match arg.as_str() {
        "--updates" => Some(Tool::Update),
        "--maintenance" => Some(Tool::Space),
        "--computer" => Some(Tool::Inside),
        "--software" => Some(Tool::Apps),
        "--appearance" => Some(Tool::Look),
        "--restore" => Some(Tool::Transfer),
        "--help-page" => Some(Tool::Help),
        _ => None,
    })
}

#[derive(Clone, Debug)]
pub enum Message {
    Overview(overview::Message),
    Updates(updates_page::Message),
    Maintenance(maintenance::Message),
    More(more::Message),
    Help(help::Message),
    WebApps(webapps::Message),
    /// A tool, or the toolbox (None)
    Open(Option<Tool>),
    /// The documentation of a tool, or of the toolbox (None)
    Docs(Option<Tool>),
    /// The first steps tour again
    Tour,
    /// A tool's command, to the clipboard
    CopyCommand(&'static str),
    CloseToast(widget::ToastId),
    /// A toast's button: closes it, then sends its message
    ToastAction(widget::ToastId, Box<Message>),
}

struct App {
    core: Core,
    /// The tool shown; None: the toolbox
    tool: Option<Tool>,
    overview: overview::Page,
    updates: updates_page::Page,
    maintenance: maintenance::Page,
    more: more::Page,
    help: help::Page,
    webapps: webapps::Page,
    toasts: Toasts<Message>,
}

impl App {
    /// A tool or the toolbox was opened: what it shows is read again
    fn shown(&mut self) -> Task<Message> {
        let opened = match self.tool {
            None => overview::refresh().map(|action| action.map(Message::Overview)),
            Some(Tool::Disks) => self
                .more
                .opened(more::Part::Disks)
                .map(|action| action.map(Message::More)),
            Some(Tool::Apps) => self
                .more
                .opened(more::Part::Apps)
                .map(|action| action.map(Message::More)),
            Some(Tool::WebApps) => self
                .webapps
                .opened()
                .map(|action| action.map(Message::WebApps)),
            Some(_) => Task::none(),
        };
        // From the top
        let id = cosmic::iced::widget::Id::new(SCROLL);
        Task::batch([
            opened,
            cosmic::iced::widget::scrollable::snap_to(
                id,
                cosmic::iced::widget::scrollable::RelativeOffset {
                    x: Some(0.0),
                    y: Some(0.0),
                },
            ),
        ])
    }

    /// A toast a page asked for, its button sending the page's message
    fn toast<M: Clone + Send + 'static>(
        &mut self,
        ask: ToastAsk<M>,
        wrap: fn(M) -> Message,
    ) -> Task<Message> {
        let mut toast = widget::toaster::Toast::new(ask.text);
        if let Some((label, message)) = ask.action {
            toast = toast.action(label, move |id| {
                Message::ToastAction(id, Box::new(wrap((*message).clone())))
            });
        }
        self.toasts.push(toast).map(cosmic::Action::App)
    }

    /// What a tool's tile says: the parts of the pages that know
    fn summary(&self, tool: Tool) -> Summary {
        match tool {
            Tool::Update | Tool::Backup | Tool::Space => self.overview.summary(tool),
            Tool::Rollback => match self.updates.previous() {
                Some(version) => Summary::Says(fill(tr("Version {}"), version)),
                None => Summary::Unknown,
            },
            Tool::Repair => match self.maintenance.missing() {
                0 => Summary::Unknown,
                1 => Summary::Warns(tr("One of them was removed. Put it back?").to_owned()),
                count => Summary::Warns(fill(
                    tr("{} of them were removed. Put them back?"),
                    &count.to_string(),
                )),
            },
            Tool::WebApps => self
                .webapps
                .summary()
                .map_or(Summary::Unknown, Summary::Says),
            Tool::Help | Tool::Advanced => Summary::Unknown,
            _ => self.more.summary(more_part(tool)),
        }
    }

    /// A tool's rows: the parts of the pages it shows
    fn tool_view(&self, tool: Tool) -> Element<'_, Message> {
        let space_s = theme::spacing().space_s;
        let more = |part| self.more.view(part).map(Message::More);
        let parts: Vec<Element<'_, Message>> = match tool {
            Tool::Update => {
                let mut parts = vec![
                    self.updates
                        .view(updates_page::Part::Main)
                        .map(Message::Updates),
                ];
                if self.maintenance.has_firmware() {
                    parts.push(
                        self.maintenance
                            .view(maintenance::Part::Firmware)
                            .map(Message::Maintenance),
                    );
                }
                parts
            }
            Tool::Space => vec![
                self.maintenance
                    .view(maintenance::Part::Space)
                    .map(Message::Maintenance),
            ],
            Tool::Backup => vec![self.overview.backup_view().map(Message::Overview)],
            Tool::Rollback => vec![
                self.updates
                    .view(updates_page::Part::Rollback)
                    .map(Message::Updates),
            ],
            Tool::Repair => vec![
                self.maintenance
                    .view(maintenance::Part::Repair)
                    .map(Message::Maintenance),
                more(more::Part::Repair),
            ],
            Tool::Help => vec![self.help.view().map(Message::Help)],
            Tool::WebApps => vec![self.webapps.view().map(Message::WebApps)],
            Tool::Advanced => vec![
                self.updates
                    .view(updates_page::Part::Advanced)
                    .map(Message::Updates),
                more(more::Part::Advanced),
            ],
            _ => vec![more(more_part(tool))],
        };
        parts
            .into_iter()
            .fold(
                widget::column::with_capacity(3).spacing(space_s),
                |column, part| column.push(part),
            )
            .into()
    }

    /// A tool's page: its header, its rows, then the way back and its
    /// command
    fn tool_page(&self, tool: Tool) -> Element<'_, Message> {
        let cosmic_theme::Spacing {
            space_xxs,
            space_s,
            space_m,
            ..
        } = theme::spacing();
        let (title, lead, _) = tool.texts();
        // A child or a standard account: the changes that need root ask for
        // someone else's password, said once under the purpose of the tools
        // that have such changes
        let asks_root = !matches!(
            tool,
            Tool::Backup | Tool::Transfer | Tool::Inside | Tool::Look | Tool::Help | Tool::WebApps
        );
        let lead = if common::is_admin() || !asks_root {
            tr(lead).to_owned()
        } else {
            format!(
                "{}\n{}",
                tr(lead),
                tr("Some changes ask for the password of a parent or an administrator.")
            )
        };
        let help = widget::tooltip(
            widget::button::icon(widget::icon::from_name("help-browser-symbolic"))
                .on_press(Message::Docs(Some(tool))),
            text::body(tr("Help on this tool")),
            widget::tooltip::Position::Bottom,
        );
        let header = widget::row::with_capacity(3)
            .spacing(space_s)
            .align_y(Alignment::Center)
            .push(common::tool_icon(tool.icon(), 44))
            .push(
                widget::column::with_capacity(2)
                    .spacing(2)
                    .push(text::title3(tr(title)))
                    .push(dim_caption(lead).wrapping(Wrapping::Word))
                    .width(Length::Fill),
            )
            .push(help);
        let body = widget::column::with_capacity(3)
            .push(header)
            .push(widget::space::vertical().height(space_m))
            .push(self.tool_view(tool))
            .push(widget::space::vertical().height(space_m));
        let mut footer = widget::row::with_capacity(5)
            .spacing(space_xxs)
            .align_y(Alignment::Center)
            .push(
                widget::button::standard(tr("Toolbox"))
                    .leading_icon(widget::icon::from_name("go-previous-symbolic"))
                    .on_press(Message::Open(None)),
            )
            .push(widget::space::horizontal().width(Length::Fill));
        if let Some(command) = tool.command() {
            footer = footer
                .push(dim_caption(tr("In a terminal:")))
                .push(text::monotext(command).size(13))
                .push(
                    widget::button::icon(widget::icon::from_name("edit-copy-symbolic"))
                        .tooltip(tr("Copy"))
                        .on_press(Message::CopyCommand(command)),
                );
        }
        // Web apps: its tiles as wide as the toolbox's
        let width = if tool == Tool::WebApps { WIDE } else { 640.0 };
        frame(body.into(), footer.into(), width)
    }
}

/// The width of the toolbox (and of the pages with tiles)
const WIDE: f32 = 880.0;

/// The scroll area of the pages (back to the top when one opens)
const SCROLL: &str = "page";

/// A page: its content, which scrolls only when the window is made smaller
/// than its default size, and its footer always at the bottom
fn frame<'a>(
    body: Element<'a, Message>,
    footer: Element<'a, Message>,
    width: f32,
) -> Element<'a, Message> {
    let cosmic_theme::Spacing {
        space_xxs, space_s, ..
    } = theme::spacing();
    let center = |element: Element<'a, Message>| {
        element
            .apply(widget::container)
            .max_width(width)
            .apply(widget::container)
            .center_x(Length::Fill)
    };
    widget::column::with_capacity(2)
        .push(
            center(body)
                .padding([space_xxs, 0])
                .apply(widget::scrollable)
                .id(cosmic::iced::widget::Id::new(SCROLL))
                .height(Length::Fill),
        )
        .push(center(footer).padding([space_s, 0, 0, 0]))
        .padding([0, 0, space_s, 0])
        .into()
}

/// The part of more.rs that is a tool
fn more_part(tool: Tool) -> more::Part {
    match tool {
        Tool::Print => more::Part::Print,
        Tool::Transfer => more::Part::Transfer,
        Tool::Inside => more::Part::Inside,
        Tool::Battery => more::Part::Battery,
        Tool::Disks => more::Part::Disks,
        Tool::Look => more::Part::Look,
        Tool::Nvidia => more::Part::Nvidia,
        Tool::Fonts => more::Part::Fonts,
        Tool::Repair => more::Part::Repair,
        Tool::Advanced => more::Part::Advanced,
        _ => more::Part::Apps,
    }
}

impl Application for App {
    type Executor = executor::Default;
    type Flags = Option<Tool>;
    type Message = Message;
    const APP_ID: &'static str = "io.github.celenn_os.Tools";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, first: Option<Tool>) -> (Self, Task<Message>) {
        let (overview, overview_task) = overview::Page::new();
        let (updates, updates_task) = updates_page::Page::new();
        let (maintenance, maintenance_task) = maintenance::Page::new();
        let (more, more_task) = more::Page::new();
        let (help, help_task) = help::Page::new();
        // (opened on Web apps: its sites' icons are fetched now)
        let mut webapps = webapps::Page::new();
        let webapps_task = if first == Some(Tool::WebApps) {
            webapps.opened()
        } else {
            Task::none()
        };
        let mut app = App {
            core,
            tool: first,
            overview,
            updates,
            maintenance,
            more,
            help,
            webapps,
            toasts: Toasts::new(Message::CloseToast),
        };
        let title = match app.core.main_window_id() {
            Some(id) => app.set_window_title(tr("Celenn OS Tools").to_owned(), id),
            None => Task::none(),
        };
        (
            app,
            Task::batch([
                title,
                overview_task.map(|action| action.map(Message::Overview)),
                updates_task.map(|action| action.map(Message::Updates)),
                maintenance_task.map(|action| action.map(Message::Maintenance)),
                more_task.map(|action| action.map(Message::More)),
                help_task.map(|action| action.map(Message::Help)),
                webapps_task.map(|action| action.map(Message::WebApps)),
            ]),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Open(tool) => {
                self.tool = tool;
                self.shown()
            }
            Message::Overview(overview::Message::Go(tool)) => {
                self.update(Message::Open(Some(tool)))
            }
            Message::Docs(tool) => {
                let path = tool.map_or("docs/", Tool::docs);
                common::spawn("xdg-open", &[&common::site_url(path)]);
                Task::none()
            }
            Message::Tour => {
                common::spawn(TOUR, &[]);
                Task::none()
            }
            Message::CopyCommand(command) => cosmic::iced::clipboard::write(command.to_owned()),
            Message::CloseToast(id) => {
                self.toasts.remove(id);
                Task::none()
            }
            Message::ToastAction(id, message) => {
                self.toasts.remove(id);
                self.update(*message)
            }
            Message::Updates(updates_page::Message::Toast(ask)) => {
                self.toast(ask, Message::Updates)
            }
            Message::Maintenance(maintenance::Message::Toast(ask)) => {
                self.toast(ask, Message::Maintenance)
            }
            Message::More(more::Message::Toast(ask)) => self.toast(ask, Message::More),
            Message::WebApps(webapps::Message::Toast(ask)) => self.toast(ask, Message::WebApps),
            Message::WebApps(message) => self
                .webapps
                .update(message)
                .map(|action| action.map(Message::WebApps)),
            Message::Overview(message) => self
                .overview
                .update(message)
                .map(|action| action.map(Message::Overview)),
            Message::Updates(message) => self
                .updates
                .update(message)
                .map(|action| action.map(Message::Updates)),
            Message::Maintenance(message) => self
                .maintenance
                .update(message)
                .map(|action| action.map(Message::Maintenance)),
            Message::More(message) => self
                .more
                .update(message)
                .map(|action| action.map(Message::More)),
            Message::Help(message) => self
                .help
                .update(message)
                .map(|action| action.map(Message::Help)),
        }
    }

    /// Change computers: the dialog that takes a settings file back
    fn dialog(&self) -> Option<Element<'_, Message>> {
        self.more.dialog().map(|dialog| dialog.map(Message::More))
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Message> {
        self.updates.subscription().map(Message::Updates)
    }

    fn view(&self) -> Element<'_, Message> {
        let page = match self.tool {
            Some(tool) => self.tool_page(tool),
            None => {
                let tiles: Vec<(Tool, Summary)> = GROUPS
                    .iter()
                    .flat_map(|(_, tools)| tools.iter())
                    .map(|&tool| (tool, self.summary(tool)))
                    .collect();
                let (body, footer) = self.overview.view(&tiles);
                frame(
                    body.map(Message::Overview),
                    footer.map(Message::Overview),
                    WIDE,
                )
            }
        };
        widget::toaster(&self.toasts, page)
    }
}
