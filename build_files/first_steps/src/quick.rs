// The quick menu of celenn Tools (`celenn-tools --quick`, on
// Super + Alt + Space): a small window with what people do most, a search,
// and the whole toolbox one click away. The same shortcut, Escape or a click
// elsewhere closes it. It opens the tools themselves (tools.rs), so they
// stay in one place.

use crate::common::{dim_caption, row_title, spawn, tool_icon};
use crate::i18n::tr;
use crate::{TOUR, Tool};
use cosmic::app::{Core, Settings, Task};
use cosmic::iced::core::text::{Ellipsize, EllipsizeHeightLimit};
use cosmic::iced::keyboard::{self, Key, key::Named};
use cosmic::iced::{Alignment, Event, Length, Subscription, event, window};
use cosmic::widget::{self, text};
use cosmic::{Application, ApplicationExt, Apply, Element, executor, theme};

const TOOLS: &str = "/usr/libexec/celenn-tools";

pub const TEXTS: &[&str] = &[
    "Quick menu",
    "What do you want to do?",
    "Right away",
    "Take a screenshot",
    "The whole screen, a window or an area",
    "See the keyboard shortcuts",
    "Every key on one page",
    "Lock the screen",
    "Your windows stay open",
    "The whole toolbox…",
    "Nothing found.",
    "Shift",
    "Escape",
];

/// What the menu offers
#[derive(Clone, Copy, Debug, PartialEq)]
enum Entry {
    Screenshot,
    Shortcuts,
    Lock,
    Tool(Tool),
}

/// The entries, in order: first what happens right away, then the tools
const ENTRIES: &[Entry] = &[
    Entry::Screenshot,
    Entry::Shortcuts,
    Entry::Lock,
    Entry::Tool(Tool::Update),
    Entry::Tool(Tool::Apps),
    Entry::Tool(Tool::WebApps),
    Entry::Tool(Tool::Backup),
    Entry::Tool(Tool::Help),
];

impl Entry {
    /// Its title, its caption, its symbol and its shortcut
    fn texts(
        self,
    ) -> (
        &'static str,
        &'static str,
        &'static str,
        Option<&'static str>,
    ) {
        match self {
            Entry::Screenshot => (
                "Take a screenshot",
                "The whole screen, a window or an area",
                "camera-photo-symbolic",
                Some("Super + Shift + S"),
            ),
            Entry::Shortcuts => (
                "See the keyboard shortcuts",
                "Every key on one page",
                "input-keyboard-symbolic",
                Some("Super + F1"),
            ),
            Entry::Lock => (
                "Lock the screen",
                "Your windows stay open",
                "system-lock-screen-symbolic",
                Some("Super + Escape"),
            ),
            Entry::Tool(tool) => {
                let (title, _, caption) = tool.texts();
                (title, caption, tool.icon(), None)
            }
        }
    }

    fn run(self) {
        match self {
            Entry::Screenshot => spawn("cosmic-screenshot", &[]),
            Entry::Shortcuts => spawn(TOUR, &["--shortcuts"]),
            Entry::Lock => spawn("loginctl", &["lock-session"]),
            Entry::Tool(tool) => spawn(TOOLS, &["--tool", tool.name()]),
        }
    }
}

/// Lower case, without accents, to search
fn fold(text: &str) -> String {
    crate::overview::fold(text)
}

#[derive(Clone, Debug)]
pub enum Message {
    Search(String),
    /// Up (-1) or down (1)
    Move(isize),
    /// The chosen entry (None: the one selected)
    Run(Option<usize>),
    Toolbox,
    Close,
}

pub struct App {
    core: Core,
    query: String,
    /// The selected entry among those shown
    selected: usize,
}

impl App {
    /// The entries the search finds (all of them without a search)
    fn shown(&self) -> Vec<Entry> {
        let words: Vec<String> = fold(&self.query)
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        ENTRIES
            .iter()
            .copied()
            .filter(|entry| {
                let (title, caption, _, _) = entry.texts();
                let haystack = fold(&format!("{} {} {title} {caption}", tr(title), tr(caption)));
                words.iter().all(|word| haystack.contains(word.as_str()))
            })
            .collect()
    }

    fn row<'a>(&self, entry: Entry, i: usize) -> Element<'a, Message> {
        let space_s = theme::spacing().space_s;
        let one_line = Ellipsize::End(EllipsizeHeightLimit::Lines(1));
        let (title, caption, icon, keys) = entry.texts();
        let mut row = widget::row::with_capacity(3)
            .spacing(space_s)
            .align_y(Alignment::Center)
            .push(tool_icon(icon, 32))
            .push(
                widget::column::with_capacity(2)
                    .spacing(2)
                    .push(row_title(tr(title)).ellipsize(one_line))
                    .push(dim_caption(tr(caption)).ellipsize(one_line))
                    .width(Length::Fill)
                    .clip(true),
            );
        if let Some(keys) = keys {
            row = row.push(key_combo(keys));
        }
        let selected = i == self.selected;
        row.padding([0, space_s])
            .height(Length::Fill)
            .apply(widget::button::custom)
            .class(if selected {
                crate::style::tile_style(true)
            } else {
                theme::Button::MenuItem
            })
            .padding(0)
            .width(Length::Fill)
            .height(Length::Fixed(46.0))
            .on_press(Message::Run(Some(i)))
            .into()
    }
}

/// A key's name on screen ("Shift" is "Maj" in French)
fn key_name(name: &'static str) -> &'static str {
    match name {
        "Shift" | "Escape" => tr(name),
        _ => name,
    }
}

/// "Super + Shift + S" as keys
fn key_combo<'a>(keys: &'static str) -> Element<'a, Message> {
    let mut row = widget::row::with_capacity(5)
        .spacing(3)
        .align_y(Alignment::Center);
    for (i, key) in keys.split(" + ").enumerate() {
        if i > 0 {
            row = row.push(text::caption("+"));
        }
        row = row.push(
            text::caption(key_name(key))
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
                })),
        );
    }
    row.into()
}

fn search_id() -> widget::Id {
    widget::Id::new("quick-search")
}

impl Application for App {
    type Executor = executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = "io.github.celenn_os.QuickMenu";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(mut core: Core, _: ()) -> (Self, Task<Message>) {
        core.window.show_headerbar = false;
        let mut app = App {
            core,
            query: String::new(),
            selected: 0,
        };
        let title = match app.core.main_window_id() {
            Some(id) => app.set_window_title(tr("Quick menu").to_owned(), id),
            None => Task::none(),
        };
        (
            app,
            Task::batch([title, widget::text_input::focus(search_id())]),
        )
    }

    fn on_escape(&mut self) -> Task<Message> {
        cosmic::iced::exit()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Search(query) => {
                self.query = query;
                self.selected = 0;
                Task::none()
            }
            Message::Move(step) => {
                let count = self.shown().len();
                if count > 0 {
                    self.selected = self.selected.saturating_add_signed(step).min(count - 1);
                }
                Task::none()
            }
            Message::Run(i) => {
                let shown = self.shown();
                match shown.get(i.unwrap_or(self.selected)) {
                    Some(entry) => {
                        entry.run();
                        cosmic::iced::exit()
                    }
                    None => Task::none(),
                }
            }
            Message::Toolbox => {
                spawn(TOOLS, &[]);
                cosmic::iced::exit()
            }
            Message::Close => cosmic::iced::exit(),
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        // Up and down choose; a click elsewhere closes it, like a menu
        event::listen_with(|event, _status, _window| match event {
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: Key::Named(Named::ArrowDown),
                ..
            }) => Some(Message::Move(1)),
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: Key::Named(Named::ArrowUp),
                ..
            }) => Some(Message::Move(-1)),
            Event::Window(window::Event::Unfocused) => Some(Message::Close),
            _ => None,
        })
    }

    fn view(&self) -> Element<'_, Message> {
        let cosmic_theme::Spacing {
            space_xxs,
            space_xs,
            space_s,
            space_m,
            ..
        } = theme::spacing();
        let search = widget::search_input(tr("What do you want to do?"), self.query.as_str())
            .id(search_id())
            .on_input(Message::Search)
            .on_clear(Message::Search(String::new()))
            .on_submit(|_| Message::Run(None));
        let shown = self.shown();
        let mut list = widget::column::with_capacity(shown.len() + 2).spacing(space_xxs);
        let mut group = None;
        for (i, entry) in shown.iter().enumerate() {
            let this = if matches!(entry, Entry::Tool(_)) {
                "Toolbox"
            } else {
                "Right away"
            };
            if group != Some(this) {
                group = Some(this);
                list = list.push(
                    dim_caption(tr(this))
                        .apply(widget::container)
                        .padding([space_xxs, space_s, 0, space_s]),
                );
            }
            list = list.push(self.row(*entry, i));
        }
        if shown.is_empty() {
            list = list.push(
                dim_caption(tr("Nothing found."))
                    .apply(widget::container)
                    .padding(space_s),
            );
        }
        let footer = widget::row::with_capacity(2)
            .align_y(Alignment::Center)
            .push(widget::space::horizontal().width(Length::Fill))
            .push(widget::button::standard(tr("The whole toolbox…")).on_press(Message::Toolbox));
        widget::column::with_capacity(4)
            .spacing(space_xs)
            .push(search)
            .push(list.apply(widget::scrollable).height(Length::Fill))
            .push(widget::divider::horizontal::light())
            .push(footer)
            .padding([space_m, space_m, space_s, space_m])
            .into()
    }
}

use cosmic::cosmic_theme;

/// The menu, unless it is open: then it closes
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    if crate::toggle::close_if_open("celenn-quick-menu", "--quick") {
        return Ok(());
    }
    let settings = Settings::default()
        .size(cosmic::iced::Size::new(540.0, 660.0))
        .resizable(None);
    cosmic::app::run::<App>(settings, ())?;
    Ok(())
}
