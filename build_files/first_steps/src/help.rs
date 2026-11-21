// The Help page of cosmic-blue Tools (tools.rs): the documentation, a place
// to ask questions, and the bug report of `ujust report-bug`, as buttons.
// "Report a problem" runs /usr/libexec/cosmic-blue-report-bug, the same
// script as `ujust report-bug`: it opens the bug report form with the system
// details filled in (cosmic-blue-system-info: no personal data). The page
// shows those details first, and can copy them.

use crate::common::{label, spawn, state_icon};
use crate::i18n::tr;
use cosmic::app::Task;
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, settings, text};
use cosmic::{Element, cosmic_theme, theme};

const DOCS: &str = "/usr/bin/cosmic-blue-docs";
const QUESTIONS: &str = "https://github.com/cosmic-blue-os/cosmic-blue/discussions/categories/q-a";
const REPORT: &str = "/usr/libexec/cosmic-blue-report-bug";
const SYSTEM_INFO: &str = "/usr/libexec/cosmic-blue-system-info";
const ICON: &str = "cosmic-blue-docs";

pub const TEXTS: &[&str] = &[
    "Help",
    "Find answers, ask a question, or tell us what goes wrong.",
    "Documentation",
    "Updates, software, commands and tips",
    "Open",
    "Ask a question",
    "Other cosmic-blue users and its makers answer, on GitHub.",
    "Report a problem",
    "Opens the bug report form with the details of this computer filled in. They hold no personal data. A GitHub account is needed.",
    "Show the details",
    "Hide the details",
    "Report",
    "Copy",
    "Copied.",
];

#[derive(Clone, Debug)]
pub enum Message {
    Docs,
    Questions,
    /// Show or hide the system details
    Details,
    Info(String),
    Copy,
    Report,
}

pub struct Page {
    /// The system details, once shown
    details: Option<String>,
    copied: bool,
}

fn system_info() -> String {
    std::process::Command::new(SYSTEM_INFO)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .trim_end()
                .to_owned()
        })
        .unwrap_or_else(|error| format!("{SYSTEM_INFO}: {error}"))
}

impl Page {
    pub fn new() -> (Self, Task<Message>) {
        (
            Page {
                details: None,
                copied: false,
            },
            Task::none(),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Docs => spawn(DOCS, &[]),
            Message::Questions => spawn("xdg-open", &[QUESTIONS]),
            Message::Details => {
                if self.details.take().is_none() {
                    // Reading the details takes a few seconds (rpm-ostree)
                    return Task::perform(
                        async {
                            tokio::task::spawn_blocking(system_info)
                                .await
                                .unwrap_or_default()
                        },
                        |info| cosmic::Action::App(Message::Info(info)),
                    );
                }
                self.copied = false;
            }
            Message::Info(info) => self.details = Some(info),
            Message::Copy => {
                if let Some(details) = &self.details {
                    self.copied = true;
                    return cosmic::iced::clipboard::write(details.clone());
                }
            }
            Message::Report => spawn(REPORT, &["open"]),
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let cosmic_theme::Spacing {
            space_xxs, space_s, ..
        } = theme::spacing();
        let row = |icon: &'static str,
                   title: &'static str,
                   caption: &'static str,
                   button: Element<'static, Message>| {
            settings::item_row(vec![
                state_icon(icon, false),
                label(title, tr(caption).to_owned()),
                button,
            ])
            .spacing(space_s)
        };
        let open = |message: Message| -> Element<'static, Message> {
            widget::button::standard(tr("Open"))
                .on_press(message)
                .into()
        };

        let mut report = settings::section().add(
            settings::item_row(vec![
                state_icon("dialog-warning-symbolic", false),
                label(
                    tr("Report a problem"),
                    tr("Opens the bug report form with the details of this computer filled in. They hold no personal data. A GitHub account is needed.").to_owned(),
                ),
                widget::column::with_capacity(2)
                    .spacing(space_xxs)
                    .align_x(Alignment::End)
                    .push(widget::button::suggested(tr("Report")).on_press(Message::Report))
                    .push(
                        widget::button::link(tr(if self.details.is_some() {
                            "Hide the details"
                        } else {
                            "Show the details"
                        }))
                        .on_press(Message::Details),
                    )
                    .into(),
            ])
            .spacing(space_s),
        );
        if let Some(details) = &self.details {
            report = report.add(
                widget::column::with_capacity(2)
                    .spacing(space_xxs)
                    .push(
                        text::monotext(details.as_str())
                            .size(12)
                            .width(Length::Fill),
                    )
                    .push(
                        widget::row::with_capacity(2)
                            .spacing(space_xxs)
                            .align_y(Alignment::Center)
                            .push(widget::button::standard(tr("Copy")).on_press(Message::Copy))
                            .push_maybe(self.copied.then(|| text::caption(tr("Copied.")))),
                    )
                    .padding([space_xxs, space_s]),
            );
        }

        let content = widget::column::with_capacity(3)
            .spacing(space_s)
            .align_x(Alignment::Center)
            .push(
                text::body(tr(
                    "Find answers, ask a question, or tell us what goes wrong.",
                ))
                .align_x(Alignment::Center)
                .width(Length::Fill),
            )
            .push(
                settings::section()
                    .add(row(
                        "help-contents-symbolic",
                        tr("Documentation"),
                        "Updates, software, commands and tips",
                        open(Message::Docs),
                    ))
                    .add(row(
                        "dialog-question-symbolic",
                        tr("Ask a question"),
                        "Other cosmic-blue users and its makers answer, on GitHub.",
                        open(Message::Questions),
                    )),
            )
            .push(report);
        crate::common::header(ICON, tr("Help"), content.into())
    }
}
