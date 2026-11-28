// The "Change computers" tool (more.rs): its rows, in the order things are
// done, and the dialog that takes a file back (transfer.rs).
//
//   Save my settings     (on the old computer) Save: the settings and the
//                        app list to Documents, then a toast with Show (the
//                        folder, to copy the file to a USB drive)
//   Restore my settings  (on the new one) Restore…: the file chooser, then
//                        the dialog: what the file has, ticked by group
//                        (developer tools left unticked), Restore, then what
//                        was done and what was not, with why
//   My files             not in the file: a USB drive, or LocalSend

use super::{LOCALSEND, Message, Page, State, apply_list, refresh};
use crate::apps::{CATEGORIES, DEVELOPMENT};
use crate::common::{spawn, state_icon};
use crate::failure;
use crate::i18n::{fill, tr};
use crate::transfer::{self, Group, Report, Restore};
use cosmic::app::Task;
use cosmic::iced::core::text::Wrapping;
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, settings, text};
use cosmic::{Apply, Element, theme};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum TransferMessage {
    Save,
    Saved(Result<PathBuf, String>),
    ShowFolder,
    Open,
    Chosen(Option<PathBuf>),
    Read(Result<Restore, &'static str>),
    Tick(Group, bool),
    Apply,
    /// The settings report, and whether the apps went in (None: nothing
    /// to install, or the password prompt was dismissed)
    Done(Report, Option<bool>),
    Close,
    OpenLocalSend,
}

/// The dialog's steps
pub enum Step {
    Choose(Restore, BTreeSet<Group>),
    Working,
    /// What was taken back (true) or not, with why; then notes
    Done(Vec<(bool, String)>, Vec<String>),
}

fn group_text(restore: &Restore, group: Group) -> (String, String) {
    match group {
        Group::Look => (
            tr("Look").to_owned(),
            tr("Dark or light theme, accent color, wallpaper, panel and dock.").to_owned(),
        ),
        Group::Keyboard => (
            tr("Keyboard and shortcuts").to_owned(),
            tr("Keyboard layout and the shortcuts you changed.").to_owned(),
        ),
        Group::Apps => {
            let names: Vec<&str> = restore.apps.iter().map(|(_, name)| name.as_str()).collect();
            let caption = if names.len() > 5 {
                let first = fill(tr("{} and {} more, from Flathub."), &names[..4].join(", "));
                fill(&first, &(names.len() - 4).to_string())
            } else {
                fill(tr("{}, from Flathub."), &names.join(", "))
            };
            (fill(tr("Apps ({})"), &names.len().to_string()), caption)
        }
        Group::DevTools => (
            fill(
                tr("Developer tools ({})"),
                &restore.dev_tools.len().to_string(),
            ),
            restore.dev_tools.join(", "),
        ),
    }
}

/// Installs the apps and developer tools ticked: Some(true) when all went
/// in, Some(false) when something failed, None when there was nothing to
/// do or the password prompt was dismissed
async fn install(
    apps: Vec<String>,
    dev_tools: Option<(BTreeSet<usize>, BTreeSet<usize>)>,
) -> Option<bool> {
    let mut ok = None;
    if !apps.is_empty() {
        let mut args: Vec<String> = ["install", "--system", "--noninteractive", "-y", "flathub"]
            .map(str::to_owned)
            .to_vec();
        args.extend(apps);
        ok = match failure::run("flatpak", args, None).await {
            Some(Ok(())) => Some(true),
            Some(Err(_)) => Some(false),
            None => None,
        };
    }
    if let Some((installed, chosen)) = dev_tools {
        let tools = match apply_list(DEVELOPMENT, installed, chosen).await {
            Some(Ok(())) => Some(true),
            Some(Err(_)) => Some(false),
            None => None,
        };
        ok = match (ok, tools) {
            (Some(false), _) | (_, Some(false)) => Some(false),
            (Some(true), _) | (_, Some(true)) => Some(true),
            _ => None,
        };
    }
    ok
}

impl Page {
    pub(super) fn transfer_update(&mut self, message: TransferMessage) -> Task<Message> {
        let app = |message| cosmic::Action::App(Message::Transfer(message));
        match message {
            TransferMessage::Save => {
                let dev_tools: Vec<&'static str> = self
                    .state
                    .as_ref()
                    .map(|state| {
                        CATEGORIES[DEVELOPMENT]
                            .tools()
                            .enumerate()
                            .filter(|(i, _)| state.installed[DEVELOPMENT].contains(i))
                            .map(|(_, tool)| tool.name)
                            .collect()
                    })
                    .unwrap_or_default();
                self.saving = true;
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || transfer::save(dev_tools))
                            .await
                            .unwrap_or_else(|error| Err(error.to_string()))
                    },
                    move |result| app(TransferMessage::Saved(result)),
                );
            }
            TransferMessage::Saved(result) => {
                self.saving = false;
                return match result {
                    Ok(_) => crate::common::toast(
                        tr("Saved in Documents.").to_owned(),
                        Some((
                            tr("Show").to_owned(),
                            Message::Transfer(TransferMessage::ShowFolder),
                        )),
                        Message::Toast,
                    ),
                    Err(error) => {
                        eprintln!("Could not save the settings: {error}");
                        self.toast("This did not work.")
                    }
                };
            }
            TransferMessage::ShowFolder => {
                let folder = transfer::documents_folder();
                spawn("xdg-open", &[&folder.to_string_lossy()]);
            }
            TransferMessage::Open => {
                use cosmic::dialog::file_chooser;
                let dialog = file_chooser::open::Dialog::new()
                    .title(tr("Choose the file saved on your other computer"))
                    .filter(file_chooser::FileFilter::new("Celenn OS").glob("*.json"));
                return Task::perform(
                    async move {
                        dialog
                            .open_file()
                            .await
                            .ok()
                            .and_then(|response| response.url().to_file_path().ok())
                    },
                    move |path| app(TransferMessage::Chosen(path)),
                );
            }
            TransferMessage::Chosen(None) => {}
            TransferMessage::Chosen(Some(path)) => {
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            transfer::read(&path, &crate::system::installed_apps())
                        })
                        .await
                        .unwrap_or(Err("This file could not be read."))
                    },
                    move |result| app(TransferMessage::Read(result)),
                );
            }
            TransferMessage::Read(Err(why)) => return self.toast(why),
            TransferMessage::Read(Ok(mut restore)) => {
                // Developer tools already here are not offered again
                if let Some(state) = &self.state {
                    let installed = &state.installed[DEVELOPMENT];
                    restore.dev_tools.retain(|name| {
                        !CATEGORIES[DEVELOPMENT]
                            .tools()
                            .position(|tool| tool.name == *name)
                            .is_some_and(|i| installed.contains(&i))
                    });
                }
                let groups = restore.groups();
                if groups.is_empty() {
                    return self
                        .toast("Nothing to take back: everything in this file is already here.");
                }
                let ticked = groups
                    .into_iter()
                    .filter(|g| *g != Group::DevTools)
                    .collect();
                self.restore = Some(Step::Choose(restore, ticked));
            }
            TransferMessage::Tick(group, on) => {
                if let Some(Step::Choose(_, ticked)) = &mut self.restore {
                    if on {
                        ticked.insert(group);
                    } else {
                        ticked.remove(&group);
                    }
                }
            }
            TransferMessage::Apply => {
                let Some(Step::Choose(restore, ticked)) = self.restore.take() else {
                    return Task::none();
                };
                self.restore = Some(Step::Working);
                let apps: Vec<String> = if ticked.contains(&Group::Apps) {
                    restore.apps.iter().map(|(id, _)| id.clone()).collect()
                } else {
                    Vec::new()
                };
                let dev_tools = self
                    .state
                    .as_ref()
                    .filter(|_| ticked.contains(&Group::DevTools) && !restore.dev_tools.is_empty())
                    .map(|state| {
                        let installed = state.installed[DEVELOPMENT].clone();
                        let mut chosen = installed.clone();
                        chosen.extend(
                            CATEGORIES[DEVELOPMENT]
                                .tools()
                                .enumerate()
                                .filter(|(_, tool)| restore.dev_tools.contains(&tool.name))
                                .map(|(i, _)| i),
                        );
                        (installed, chosen)
                    });
                self.restored = Some((restore.clone(), ticked.clone()));
                return Task::perform(
                    async move {
                        let report = tokio::task::spawn_blocking(move || {
                            transfer::apply_settings(&restore, &ticked)
                        })
                        .await
                        .unwrap_or_default();
                        let apps = install(apps, dev_tools).await;
                        (report, apps)
                    },
                    move |(report, apps)| app(TransferMessage::Done(report, apps)),
                );
            }
            TransferMessage::Done(report, apps) => {
                let Some((restore, ticked)) = self.restored.take() else {
                    return Task::none();
                };
                let mut rows = Vec::new();
                for group in ticked {
                    let (title, _) = group_text(&restore, group);
                    match group {
                        Group::Look | Group::Keyboard => {
                            rows.push((report.groups.contains(&group), title));
                        }
                        Group::Apps | Group::DevTools if apps == Some(true) => {
                            rows.push((true, title));
                        }
                        Group::Apps | Group::DevTools => {
                            let failed = tr("The apps could not be installed.").to_owned();
                            if !rows.contains(&(false, failed.clone())) {
                                rows.push((false, failed));
                            }
                        }
                    }
                }
                rows.extend(
                    transfer::left_out(&restore, &report)
                        .into_iter()
                        .map(|line| (false, line)),
                );
                let mut notes = Vec::new();
                if report.written > 0 {
                    notes.push(tr("Log out and back in to see every change.").to_owned());
                }
                if let Some(backup) = &report.backup {
                    // ~/.config/… rather than the whole path
                    let path = backup.to_string_lossy();
                    let home = std::env::var("HOME").unwrap_or_default();
                    let path = match path.strip_prefix(&format!("{home}/")) {
                        Some(rest) if !home.is_empty() => format!("~/{rest}"),
                        _ => path.into_owned(),
                    };
                    notes.push(fill(
                        tr("A copy of your previous settings is in {}."),
                        &path,
                    ));
                }
                self.restore = Some(Step::Done(rows, notes));
                return refresh();
            }
            TransferMessage::Close => self.restore = None,
            TransferMessage::OpenLocalSend => spawn("flatpak", &["run", LOCALSEND]),
        }
        Task::none()
    }

    /// The tool's rows: save on the old computer, take back on the new one,
    /// and the files, which travel apart
    pub(super) fn transfer<'a>(&self, state: &State) -> Element<'a, Message> {
        let busy = self.saving || self.restore.is_some();
        let save: Element<'a, Message> = if self.saving {
            widget::progress_bar::indeterminate_circular()
                .size(20.0)
                .into()
        } else {
            widget::button::standard(tr("Save"))
                .on_press_maybe((!busy).then_some(Message::Transfer(TransferMessage::Save)))
                .into()
        };
        let restore = widget::button::standard(tr("Restore…"))
            .on_press_maybe((!busy).then_some(Message::Transfer(TransferMessage::Open)))
            .into();
        // LocalSend is a first-boot app; without it, the caption still says
        // what to do
        let localsend = if state.localsend {
            widget::button::standard(tr("Open LocalSend"))
                .on_press(Message::Transfer(TransferMessage::OpenLocalSend))
                .into()
        } else {
            widget::space::horizontal().width(0).into()
        };
        settings::section()
            .add(self.row(
                "document-save-symbolic",
                "Save my settings",
                "Look, keyboard, shortcuts and the list of your apps, in a file in Documents. Copy it to a USB drive to take it along.",
                save,
                None,
            ))
            .add(self.row(
                "document-open-symbolic",
                "Restore my settings",
                "On the new computer, choose the file saved on the old one, then tick what you want back.",
                restore,
                None,
            ))
            .add(self.row(
                "folder-symbolic",
                "My files",
                "Documents and photos are not in the settings file: copy them to a USB drive, or send them over Wi-Fi with LocalSend, open on both computers.",
                localsend,
                None,
            ))
            .into()
    }

    /// The dialog that takes a file back
    pub fn dialog(&self) -> Option<Element<'_, Message>> {
        let step = self.restore.as_ref()?;
        let space_xxs = theme::spacing().space_xxs;
        let dialog = match step {
            Step::Choose(restore, ticked) => {
                let body = if restore.computer.is_empty() {
                    fill(
                        tr(
                            "Saved on {}. Tick what you want back. Your files are not in this file: use Déjà Dup or LocalSend for them.",
                        ),
                        &restore.date,
                    )
                } else {
                    fill(
                        &fill(
                            tr(
                                "Saved on {} on “{}”. Tick what you want back. Your files are not in this file: use Déjà Dup or LocalSend for them.",
                            ),
                            &restore.date,
                        ),
                        &restore.computer,
                    )
                };
                let mut section = settings::section();
                for group in restore.groups() {
                    let (title, caption) = group_text(restore, group);
                    let on = ticked.contains(&group);
                    section = section.add(
                        settings::item_row(vec![
                            widget::checkbox(on)
                                .on_toggle(move |on| {
                                    Message::Transfer(TransferMessage::Tick(group, on))
                                })
                                .into(),
                            widget::column::with_capacity(2)
                                .spacing(2)
                                .push(text::heading(title))
                                .push(text::caption(caption).wrapping(Wrapping::Word))
                                .width(Length::Fill)
                                .apply(widget::mouse_area)
                                .on_press(Message::Transfer(TransferMessage::Tick(group, !on)))
                                .into(),
                        ])
                        .spacing(theme::spacing().space_s),
                    );
                }
                widget::dialog()
                    .title(tr("Restore these settings"))
                    .body(body)
                    .control(section)
                    .primary_action(
                        widget::button::suggested(tr("Take them back")).on_press_maybe(
                            (!ticked.is_empty())
                                .then_some(Message::Transfer(TransferMessage::Apply)),
                        ),
                    )
                    .secondary_action(
                        widget::button::standard(tr("Cancel"))
                            .on_press(Message::Transfer(TransferMessage::Close)),
                    )
            }
            Step::Working => widget::dialog()
                .title(tr("Restore these settings"))
                .control(
                    widget::row::with_capacity(2)
                        .spacing(space_xxs)
                        .align_y(Alignment::Center)
                        .push(widget::progress_bar::indeterminate_circular().size(20.0))
                        .push(text::body(tr("Restoring…"))),
                ),
            Step::Done(rows, notes) => {
                let mut section = settings::section();
                for (done, line) in rows {
                    let icon = if *done {
                        state_icon("object-select-symbolic", true)
                    } else {
                        state_icon("dialog-information-symbolic", false)
                    };
                    section = section.add(
                        settings::item_row(vec![
                            icon,
                            text::body(line.clone())
                                .wrapping(Wrapping::Word)
                                .width(Length::Fill)
                                .into(),
                        ])
                        .spacing(theme::spacing().space_s),
                    );
                }
                let mut column = widget::column::with_capacity(notes.len() + 1)
                    .spacing(space_xxs)
                    .push(section);
                for note in notes {
                    column = column.push(text::caption(note.clone()).wrapping(Wrapping::Word));
                }
                widget::dialog()
                    .title(tr("Restored"))
                    .control(column)
                    .primary_action(
                        widget::button::suggested(tr("Close"))
                            .on_press(Message::Transfer(TransferMessage::Close)),
                    )
            }
        };
        Some(dialog.into())
    }
}
