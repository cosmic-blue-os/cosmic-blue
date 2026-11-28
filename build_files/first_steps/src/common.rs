// What the tools of Celenn OS Tools share (tools.rs): their toasts, their
// round symbol, the rows' text and symbols, and running a command in the
// background.

use cosmic::iced::Length;
use cosmic::iced::core::text::Wrapping;
use cosmic::widget;
use cosmic::{Apply, Element, theme};
use std::sync::OnceLock;

/// A short message a page asks the window to show at the bottom (tools.rs
/// keeps one toaster for every page, so that pages made of several parts
/// show their messages the same way): its text, and maybe a button, with
/// its label and the page's message it sends
#[derive(Clone, Debug)]
pub struct ToastAsk<M> {
    pub text: String,
    pub action: Option<(String, Box<M>)>,
}

/// Asks the window for a toast; `wrap` is the page's message that carries it
pub fn toast<M: Send + 'static>(
    text: String,
    action: Option<(String, M)>,
    wrap: fn(ToastAsk<M>) -> M,
) -> cosmic::app::Task<M> {
    let action = action.map(|(label, message)| (label, Box::new(message)));
    cosmic::app::Task::done(cosmic::Action::App(wrap(ToastAsk { text, action })))
}

/// What a tool's tile on the toolbox says (tools.rs): nothing known yet
/// (its usual caption), not shown (nothing to do on this computer), how
/// things are, or something that needs attention
#[derive(Clone, Debug, PartialEq)]
pub enum Summary {
    Unknown,
    Hidden,
    Says(String),
    Warns(String),
}

/// A tool's symbol in a round accent-colored button shape, as the dock's
/// icons of Celenn OS: on the toolbox's tiles and at the top of a tool
pub fn tool_icon<'a, M: 'static>(name: &'static str, size: u16) -> Element<'a, M> {
    widget::icon::from_name(name)
        .size(size / 2)
        .apply(widget::container)
        .center(Length::Fixed(f32::from(size)))
        .class(theme::Container::custom(move |theme| {
            let accent = theme.cosmic().accent_color();
            cosmic::iced::widget::container::Style {
                icon_color: Some(cosmic::iced::Color::WHITE),
                background: Some(cosmic::iced::Background::Color(accent.into())),
                border: cosmic::iced::Border {
                    radius: (f32::from(size) / 2.0).into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        }))
        .into()
}

/// Whether the account is an administrator (in the wheel group): its
/// password is the one pkexec asks for
pub fn is_admin() -> bool {
    static ADMIN: OnceLock<bool> = OnceLock::new();
    *ADMIN.get_or_init(|| {
        std::process::Command::new("id")
            .arg("-nG")
            .output()
            .map(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .split_whitespace()
                    .any(|group| group == "wheel")
            })
            // Unknown: no notice
            .unwrap_or(true)
    })
}

pub use crate::i18n::fill;

/// The symbol in front of a row: in the accent color when it stands out
pub fn state_icon<'a, M: 'static>(name: &'static str, accent: bool) -> Element<'a, M> {
    let icon = widget::icon::from_name(name)
        .size(24)
        .apply(widget::container);
    if accent {
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

pub use crate::style::{dim_caption, row_title};

/// A row's text: a first line (usually its title, `row_title`), and a
/// dimmed caption under it
pub fn two_lines<'a, M: 'static>(first: Element<'a, M>, caption: String) -> Element<'a, M> {
    widget::column::with_capacity(2)
        .spacing(2)
        .push(first)
        .push(dim_caption(caption).wrapping(Wrapping::Word))
        .width(Length::Fill)
        .into()
}

/// A row with its title and caption only (no first-line widget)
pub fn label<'a, M: 'static>(title: &'static str, caption: String) -> Element<'a, M> {
    two_lines(row_title(title).into(), caption)
}

/// What the image's screenshot test asks a page to show at once, from
/// CELENN_TOOLS_PREVIEW (tests/vm-lib.sh): "failure" (a failure card in
/// Free up space), "developer-tools" (the list open in Install apps) or
/// "offline" (Web apps without fetching its sites' icons)
pub fn preview(what: &str) -> bool {
    std::env::var("CELENN_TOOLS_PREVIEW").is_ok_and(|value| value == what)
}

/// A page of the Celenn OS site in the session language ("docs/#updates")
pub fn site_url(path: &str) -> String {
    let language = match crate::i18n::language() {
        Some("pt_BR") => "pt-br/",
        Some("fr") => "fr/",
        Some("de") => "de/",
        Some("es") => "es/",
        Some("it") => "it/",
        _ => "",
    };
    format!("https://celenn-os.github.io/celenn/{language}{path}")
}

pub use crate::process::spawn;
