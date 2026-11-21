// What the pages of cosmic-blue Tools share (tools.rs): the page frame (the
// tours' banner with the page's icon, its title), the rows' text and
// symbols, and running a command in the background.

use cosmic::iced::core::text::Wrapping;
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, text};
use cosmic::{Apply, Element, cosmic_theme, theme};
use std::path::Path;
use std::sync::OnceLock;

/// Banner of the tours (branding/tour/build.py), behind the page's icon
const BANNER: &str = "/usr/share/cosmic-blue/tour-banner.svg";

fn banner() -> Option<widget::svg::Handle> {
    static BANNER_HANDLE: OnceLock<Option<widget::svg::Handle>> = OnceLock::new();
    BANNER_HANDLE
        .get_or_init(|| {
            Path::new(BANNER)
                .exists()
                .then(|| widget::svg::Handle::from_path(BANNER))
        })
        .clone()
}

/// A page: the banner with its icon, its title, then its content, centered
/// and scrolling when the window is too small
pub fn header<'a, M: 'static>(
    icon: &'static str,
    title: &'static str,
    content: Element<'a, M>,
) -> Element<'a, M> {
    let cosmic_theme::Spacing {
        space_xxs, space_s, ..
    } = theme::spacing();
    let icon = widget::icon::from_name(icon).size(64);
    let top: Element<'a, M> = match banner() {
        Some(banner) => cosmic::iced::widget::stack![
            widget::svg(banner).width(600).height(100),
            widget::container(icon).center(Length::Fill),
        ]
        .width(600)
        .height(100)
        .apply(widget::container)
        .center_x(Length::Fill)
        .into(),
        None => widget::container(icon).center_x(Length::Fill).into(),
    };
    widget::column::with_capacity(6)
        .push(top)
        .push(widget::space::vertical().height(space_xxs))
        .push(text::title2(title).center().width(Length::Fill))
        .push(widget::space::vertical().height(space_s))
        .push(content)
        .push(widget::space::vertical().height(space_s))
        .max_width(640.0)
        .width(640.0)
        .align_x(Alignment::Center)
        .apply(widget::container)
        .center_x(Length::Fill)
        .apply(widget::scrollable)
        .into()
}

/// A translated text with its {} replaced
pub fn fill(text: &str, value: &str) -> String {
    text.replacen("{}", value, 1)
}

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

/// A row's text: a first line, and a caption under it
pub fn two_lines<'a, M: 'static>(first: Element<'a, M>, caption: String) -> Element<'a, M> {
    widget::column::with_capacity(2)
        .spacing(2)
        .push(first)
        .push(text::caption(caption).wrapping(Wrapping::Word))
        .width(Length::Fill)
        .into()
}

/// A row with its title and caption only (no first-line widget)
pub fn label<'a, M: 'static>(title: &'static str, caption: String) -> Element<'a, M> {
    two_lines(text::body(title).into(), caption)
}

/// Runs a command in the background: Some(true) when it worked, None when
/// pkexec says the password prompt was dismissed (126) or the password was
/// wrong three times (127). Commands that run pkexec themselves pass its
/// exit code on.
pub async fn run(program: &'static str, args: Vec<String>) -> Option<bool> {
    match tokio::process::Command::new(program)
        .args(&args)
        .status()
        .await
    {
        Ok(status) if status.success() => Some(true),
        Ok(status) if matches!(status.code(), Some(126 | 127)) => None,
        Ok(status) => {
            eprintln!("{program} {args:?}: {status}");
            Some(false)
        }
        Err(error) => {
            eprintln!("Could not start {program}: {error}");
            Some(false)
        }
    }
}

/// Starts a program without waiting for it (a browser, another app)
pub fn spawn(program: &str, args: &[&str]) {
    if let Err(error) = std::process::Command::new(program).args(args).spawn() {
        eprintln!("Could not start {program}: {error}");
    }
}
