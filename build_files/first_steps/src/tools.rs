// cosmic-blue Tools: what people would otherwise do with commands (`ujust`,
// bootc...), as buttons, in one window with a sidebar like COSMIC Settings.
//
//   Updates       the system version, check now, go back, channel
//                 (updates_page.rs; the Updates launcher opens this page)
//   Maintenance   disk space and cleaning up, the apps that come with
//                 cosmic-blue, firmware (maintenance.rs)
//   Computer      battery charge limit, NVIDIA edition, firmware settings
//   Extra software  Microsoft fonts, games, virtual machines, FlatPark,
//                 developer tools
//   Appearance    dark theme, the cosmic-blue look, reset, save settings
//                 (these three: more.rs)
//   Help          documentation, questions, bug report (help.rs)
//
// Each button runs the same program as the matching `ujust` command, so the
// terminal and the window do the same thing. Changes that need root go
// through small helpers run with pkexec (an administrator password).
// No notifications: the system looks after itself, this is for when people
// want to look or act.
//
//   cosmic-blue-tools               the window, on its first page
//   cosmic-blue-tools --updates     the window, on Updates (also when run as
//                                   /usr/libexec/cosmic-blue-updates)
//   cosmic-blue-tools --maintenance / --computer / --software /
//                     --appearance / --help-page   the window, on that page
//   cosmic-blue-tools --status / --older   see updates_page.rs (tests)
//   cosmic-blue-tools --check       image build test: every text has its
//                                   translations (no window)
//
// Text is in English, translated from the PO files of po/tools/.

use cosmic::app::{Core, Settings, Task};
use cosmic::iced::Limits;
use cosmic::widget::nav_bar;
use cosmic::{Application, ApplicationExt, Element, executor};

mod common;
mod devtools;
mod help;
mod i18n;
mod maintenance;
mod more;
mod updates_page;
use i18n::tr;

pub(crate) const LANGUAGES: &[(&str, &str)] = &[
    ("de", include_str!("../po/tools/de.po")),
    ("es", include_str!("../po/tools/es.po")),
    ("fr", include_str!("../po/tools/fr.po")),
    ("it", include_str!("../po/tools/it.po")),
    ("pt_BR", include_str!("../po/tools/pt_BR.po")),
];
pub(crate) const TEMPLATE: &str = include_str!("../po/tools/cosmic-blue-tools.pot");
pub(crate) const PO_DIR: &str = "po/tools";
pub(crate) const TEMPLATE_NAME: &str = "cosmic-blue-tools.pot";

/// The window's own texts; each page lists its own
const TEXTS: &[&str] = &["cosmic-blue Tools"];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PageId {
    Updates,
    Maintenance,
    Computer,
    Software,
    Appearance,
    Help,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let options = &args[1..];
    if options.iter().any(|arg| arg == "--check") {
        let texts: Vec<&str> = [
            TEXTS,
            updates_page::TEXTS,
            maintenance::TEXTS,
            more::TEXTS,
            help::TEXTS,
        ]
        .concat();
        let mut sorted = texts.clone();
        sorted.sort_unstable();
        sorted.dedup();
        std::process::exit(if i18n::check(&sorted) { 0 } else { 1 });
    }
    if let Some(code) = updates_page::command_line(options) {
        std::process::exit(code);
    }
    // Run as /usr/libexec/cosmic-blue-updates (a link): the Updates page
    let as_updates = args
        .first()
        .is_some_and(|name| name.ends_with("cosmic-blue-updates"));
    let page = if as_updates || options.iter().any(|arg| arg == "--updates") {
        PageId::Updates
    } else if options.iter().any(|arg| arg == "--maintenance") {
        PageId::Maintenance
    } else if options.iter().any(|arg| arg == "--computer") {
        PageId::Computer
    } else if options.iter().any(|arg| arg == "--software") {
        PageId::Software
    } else if options.iter().any(|arg| arg == "--appearance") {
        PageId::Appearance
    } else if options.iter().any(|arg| arg == "--help-page") {
        PageId::Help
    } else {
        PageId::Updates
    };
    let settings = Settings::default()
        .size(cosmic::iced::Size::new(1000.0, 760.0))
        .size_limits(Limits::NONE.min_width(520.0).min_height(480.0));
    cosmic::app::run::<App>(settings, page)?;
    Ok(())
}

#[derive(Clone, Debug)]
pub enum Message {
    Updates(updates_page::Message),
    Maintenance(maintenance::Message),
    More(more::Message),
    Help(help::Message),
}

struct App {
    core: Core,
    nav: nav_bar::Model,
    updates: updates_page::Page,
    maintenance: maintenance::Page,
    more: more::Page,
    help: help::Page,
}

impl App {
    fn page(&self) -> PageId {
        self.nav
            .active_data::<PageId>()
            .copied()
            .unwrap_or(PageId::Updates)
    }
}

impl Application for App {
    type Executor = executor::Default;
    type Flags = PageId;
    type Message = Message;
    const APP_ID: &'static str = "io.github.cosmic_blue_os.Tools";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(mut core: Core, first: PageId) -> (Self, Task<Message>) {
        core.nav_bar_set_toggled(true);
        let mut nav = nav_bar::Model::default();
        for (id, name, icon) in [
            (
                PageId::Updates,
                "Updates",
                "software-update-available-symbolic",
            ),
            (
                PageId::Maintenance,
                "Maintenance",
                "applications-utilities-symbolic",
            ),
            (PageId::Computer, "Computer", "computer-symbolic"),
            (
                PageId::Software,
                "Extra software",
                "system-software-install-symbolic",
            ),
            (
                PageId::Appearance,
                "Appearance",
                "preferences-desktop-appearance-symbolic",
            ),
            (PageId::Help, "Help", "help-browser-symbolic"),
        ] {
            let entity = nav
                .insert()
                .text(tr(name))
                .icon(cosmic::widget::icon::from_name(icon))
                .data(id);
            if id == first {
                entity.activate();
            }
        }
        let (updates, updates_task) = updates_page::Page::new();
        let (maintenance, maintenance_task) = maintenance::Page::new();
        let (more, more_task) = more::Page::new();
        let (help, help_task) = help::Page::new();
        let mut app = App {
            core,
            nav,
            updates,
            maintenance,
            more,
            help,
        };
        let title = match app.core.main_window_id() {
            Some(id) => app.set_window_title(tr("cosmic-blue Tools").to_owned(), id),
            None => Task::none(),
        };
        (
            app,
            Task::batch([
                title,
                updates_task.map(|action| action.map(Message::Updates)),
                maintenance_task.map(|action| action.map(Message::Maintenance)),
                more_task.map(|action| action.map(Message::More)),
                help_task.map(|action| action.map(Message::Help)),
            ]),
        )
    }

    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav)
    }

    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<Message> {
        self.nav.activate(id);
        Task::none()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
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

    fn view(&self) -> Element<'_, Message> {
        match self.page() {
            PageId::Updates => self.updates.view().map(Message::Updates),
            PageId::Maintenance => self.maintenance.view().map(Message::Maintenance),
            PageId::Computer => self.more.view(more::Kind::Computer).map(Message::More),
            PageId::Software => self.more.view(more::Kind::Software).map(Message::More),
            PageId::Appearance => self.more.view(more::Kind::Appearance).map(Message::More),
            PageId::Help => self.help.view().map(Message::Help),
        }
    }
}
