// The Maintenance page of cosmic-blue Tools (tools.rs): what `ujust` offers
// to keep the computer in shape, as buttons.
//
//   - Disk space: how full the disk is (the Disk Usage Analyzer shows the
//     details) and "Clean up", which runs /usr/libexec/cosmic-blue-clean, the
//     same script as `ujust clean-system`: app parts no app uses any more,
//     unused container images, old Homebrew downloads, then (administrator
//     password) the same for everyone's apps and the system's caches. The
//     space it freed shows when it ends.
//   - The apps that come with cosmic-blue (/usr/share/flatpak/preinstall.d):
//     when some were removed, "Reinstall" puts them back, as the first boot
//     installed them.
//   - Firmware updates: opens GNOME Firmware.

use crate::common::{fill, label, run, spawn, state_icon, two_lines};
use crate::i18n::tr;
use cosmic::app::Task;
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, settings, text, toaster::Toasts};
use cosmic::{Element, cosmic_theme, theme};
use std::collections::BTreeSet;
use std::process::{Command, Stdio};

const CLEAN: &str = "/usr/libexec/cosmic-blue-clean";
const PREINSTALL_DIR: &str = "/usr/share/flatpak/preinstall.d";
const DISK_USAGE: &str = "org.gnome.baobab";
const FIRMWARE: &str = "org.gnome.Firmware";
const ICON: &str = "cosmic-blue-tools";

pub const TEXTS: &[&str] = &[
    "Maintenance",
    "Keep the computer tidy and in shape, without typing commands.",
    "Disk space",
    "{used} used of {total}",
    "{} free",
    "See what takes space",
    "Free up space",
    "Removes what nothing uses any more: leftover parts of removed apps, unused container images, old downloads of tools. Your files stay as they are.",
    "Clean up",
    "Cleaning up…",
    "{} freed",
    "B",
    "kB",
    "MB",
    "GB",
    "TB",
    "Everything was already tidy.",
    "Something went wrong, nothing was changed.",
    "Apps that come with cosmic-blue",
    "All of them are installed.",
    "One of them was removed. Put it back?",
    "{} of them were removed. Put them back?",
    "Reinstall",
    "Installing…",
    "The apps are back.",
    "The apps could not be installed. Are you online?",
    "Firmware updates",
    "New versions of the BIOS, SSD, dock… from their makers.",
    "Open",
];

#[derive(Clone, Debug, Default)]
pub struct Disk {
    pub total: u64,
    pub used: u64,
    pub free: u64,
}

#[derive(Clone, Debug)]
pub struct State {
    pub disk: Option<Disk>,
    /// Apps of the first boot that are not installed (ids)
    pub missing: Vec<String>,
    pub disk_usage_app: bool,
    pub firmware_app: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Busy {
    Cleaning,
    Reinstalling,
}

#[derive(Clone, Debug)]
pub enum Message {
    State(State),
    CleanUp,
    Reinstall,
    /// End of a background command, and whether it worked (None: the
    /// password prompt was dismissed)
    Done(Busy, Option<bool>),
    Open(&'static str),
    CloseToast(widget::ToastId),
}

pub struct Page {
    state: Option<State>,
    busy: Option<Busy>,
    /// Free space before cleaning up, to tell how much it freed
    free_before: Option<u64>,
    toasts: Toasts<Message>,
}

/// Size and use of the disk that holds the home folders and apps (/var:
/// the system's own files are on the same disk)
pub fn disk() -> Option<Disk> {
    let output = Command::new("df")
        .args(["-B1", "--output=size,used,avail", "/var"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut numbers = text
        .lines()
        .nth(1)?
        .split_whitespace()
        .map(|n| n.parse::<u64>().ok());
    Some(Disk {
        total: numbers.next()??,
        used: numbers.next()??,
        free: numbers.next()??,
    })
}

/// The flatpak apps installed for everyone
fn installed() -> BTreeSet<String> {
    Command::new("flatpak")
        .args(["list", "--system", "--columns=application"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|line| line.trim().to_owned())
                .filter(|line| !line.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// The apps of the first boot ([Flatpak Preinstall <id>])
fn preinstall() -> Vec<String> {
    let mut ids = Vec::new();
    for entry in std::fs::read_dir(PREINSTALL_DIR)
        .into_iter()
        .flatten()
        .flatten()
    {
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        ids.extend(text.lines().filter_map(|line| {
            line.trim()
                .strip_prefix("[Flatpak Preinstall ")
                .and_then(|rest| rest.strip_suffix(']'))
                .map(str::to_owned)
        }));
        // Themes for older apps are not apps people remove
        ids.retain(|id| !id.starts_with("org.gtk.Gtk3theme."));
    }
    ids.sort();
    ids.dedup();
    ids
}

pub fn read_state() -> State {
    let installed = installed();
    State {
        disk: disk(),
        missing: preinstall()
            .into_iter()
            .filter(|id| !installed.contains(id))
            .collect(),
        disk_usage_app: installed.contains(DISK_USAGE),
        firmware_app: installed.contains(FIRMWARE),
    }
}

/// Bytes as the file manager shows them: "12.3 GB" (powers of 1000), with
/// the units and the decimal comma of the session language
pub fn size(bytes: u64) -> String {
    let units = ["B", "kB", "MB", "GB", "TB"].map(tr);
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit + 1 < units.len() {
        value /= 1000.0;
        unit += 1;
    }
    let number = if unit == 0 || value >= 100.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    };
    // Every translated language writes 1,5 (English 1.5)
    let number = if crate::i18n::language().is_some() {
        number.replace('.', ",")
    } else {
        number
    };
    format!("{number} {}", units[unit])
}

fn refresh() -> Task<Message> {
    Task::perform(
        async {
            tokio::task::spawn_blocking(read_state)
                .await
                .unwrap_or_else(|_| State {
                    disk: None,
                    missing: Vec::new(),
                    disk_usage_app: false,
                    firmware_app: false,
                })
        },
        |state| cosmic::Action::App(Message::State(state)),
    )
}

impl Page {
    pub fn new() -> (Self, Task<Message>) {
        let page = Page {
            state: None,
            busy: None,
            free_before: None,
            toasts: Toasts::new(Message::CloseToast),
        };
        (page, refresh())
    }

    fn toast(&mut self, message: String) -> Task<Message> {
        self.toasts
            .push(widget::toaster::Toast::new(message))
            .map(cosmic::Action::App)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::State(state) => self.state = Some(state),
            Message::CleanUp => {
                self.busy = Some(Busy::Cleaning);
                self.free_before = disk().map(|disk| disk.free);
                return Task::perform(run(CLEAN, Vec::new()), |result| {
                    cosmic::Action::App(Message::Done(Busy::Cleaning, result))
                });
            }
            Message::Reinstall => {
                let Some(state) = &self.state else {
                    return Task::none();
                };
                self.busy = Some(Busy::Reinstalling);
                let mut args: Vec<String> =
                    ["install", "--system", "--noninteractive", "-y", "flathub"]
                        .map(str::to_owned)
                        .to_vec();
                args.extend(state.missing.iter().cloned());
                return Task::perform(run("flatpak", args), |result| {
                    cosmic::Action::App(Message::Done(Busy::Reinstalling, result))
                });
            }
            Message::Done(busy, result) => {
                self.busy = None;
                let toast = match (busy, result) {
                    (_, None) => Task::none(),
                    (Busy::Cleaning, Some(true)) => {
                        let freed = self
                            .free_before
                            .zip(disk().map(|disk| disk.free))
                            .map_or(0, |(before, after)| after.saturating_sub(before));
                        // Less than 10 MB: nothing worth saying
                        if freed < 10_000_000 {
                            self.toast(tr("Everything was already tidy.").to_owned())
                        } else {
                            self.toast(fill(tr("{} freed"), &size(freed)))
                        }
                    }
                    (Busy::Cleaning, Some(false)) => {
                        self.toast(tr("Something went wrong, nothing was changed.").to_owned())
                    }
                    (Busy::Reinstalling, Some(true)) => {
                        self.toast(tr("The apps are back.").to_owned())
                    }
                    (Busy::Reinstalling, Some(false)) => self
                        .toast(tr("The apps could not be installed. Are you online?").to_owned()),
                };
                return Task::batch([toast, refresh()]);
            }
            Message::Open(app) => spawn("flatpak", &["run", app]),
            Message::CloseToast(id) => self.toasts.remove(id),
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let cosmic_theme::Spacing {
            space_xxs, space_s, ..
        } = theme::spacing();
        let idle = self.busy.is_none();
        let mut content = widget::column::with_capacity(6)
            .spacing(space_s)
            .align_x(Alignment::Center)
            .push(
                text::body(tr(
                    "Keep the computer tidy and in shape, without typing commands.",
                ))
                .align_x(Alignment::Center)
                .width(Length::Fill),
            );
        let Some(state) = &self.state else {
            return crate::common::header(ICON, tr("Maintenance"), content.into());
        };

        // Disk space
        let mut disk = settings::section().title(tr("Disk space"));
        if let Some(info) = &state.disk {
            let share = if info.total > 0 {
                info.used as f32 / info.total as f32
            } else {
                0.0
            };
            let used = tr("{used} used of {total}")
                .replace("{used}", &size(info.used))
                .replace("{total}", &size(info.total));
            let mut row = vec![
                state_icon("drive-harddisk-symbolic", share >= 0.9),
                widget::column::with_capacity(3)
                    .spacing(space_xxs)
                    .push(text::body(used))
                    .push(
                        widget::progress_bar::determinate_linear(share)
                            .width(Length::Fill)
                            .girth(6),
                    )
                    .push(text::caption(fill(tr("{} free"), &size(info.free))))
                    .width(Length::Fill)
                    .into(),
            ];
            if state.disk_usage_app {
                row.push(
                    widget::button::standard(tr("See what takes space"))
                        .on_press(Message::Open(DISK_USAGE))
                        .into(),
                );
            }
            disk = disk.add(settings::item_row(row).spacing(space_s));
        }
        let clean: Element<'_, Message> = if self.busy == Some(Busy::Cleaning) {
            widget::row::with_capacity(2)
                .spacing(space_xxs)
                .align_y(Alignment::Center)
                .push(widget::progress_bar::indeterminate_circular().size(20.0))
                .push(text::body(tr("Cleaning up…")))
                .into()
        } else {
            widget::button::suggested(tr("Clean up"))
                .on_press_maybe(idle.then_some(Message::CleanUp))
                .into()
        };
        disk = disk.add(
            settings::item_row(vec![
                state_icon("edit-clear-all-symbolic", false),
                label(
                    tr("Free up space"),
                    tr("Removes what nothing uses any more: leftover parts of removed apps, unused container images, old downloads of tools. Your files stay as they are.").to_owned(),
                ),
                clean,
            ])
            .spacing(space_s),
        );
        content = content.push(disk);

        // Apps that come with cosmic-blue
        let caption = if state.missing.is_empty() {
            tr("All of them are installed.").to_owned()
        } else {
            match state.missing.len() {
                1 => tr("One of them was removed. Put it back?").to_owned(),
                count => fill(
                    tr("{} of them were removed. Put them back?"),
                    &count.to_string(),
                ),
            }
        };
        let mut apps_row = vec![
            state_icon("view-app-grid-symbolic", false),
            label(tr("Apps that come with cosmic-blue"), caption),
        ];
        if self.busy == Some(Busy::Reinstalling) {
            apps_row.push(
                widget::row::with_capacity(2)
                    .spacing(space_xxs)
                    .align_y(Alignment::Center)
                    .push(widget::progress_bar::indeterminate_circular().size(20.0))
                    .push(text::body(tr("Installing…")))
                    .into(),
            );
        } else if !state.missing.is_empty() {
            apps_row.push(
                widget::button::standard(tr("Reinstall"))
                    .on_press_maybe(idle.then_some(Message::Reinstall))
                    .into(),
            );
        }
        let mut system = settings::section().add(settings::item_row(apps_row).spacing(space_s));

        // Firmware
        if state.firmware_app {
            system = system.add(
                settings::item_row(vec![
                    state_icon("application-x-firmware-symbolic", false),
                    two_lines(
                        text::body(tr("Firmware updates")).into(),
                        tr("New versions of the BIOS, SSD, dock… from their makers.").to_owned(),
                    ),
                    widget::button::standard(tr("Open"))
                        .on_press(Message::Open(FIRMWARE))
                        .into(),
                ])
                .spacing(space_s),
            );
        }
        content = content.push(system);

        let page = crate::common::header(ICON, tr("Maintenance"), content.into());
        widget::toaster(&self.toasts, page)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(size(0), "0 B");
        assert_eq!(size(9_400_000_000), "9.4 GB");
        assert_eq!(size(238_000_000_000), "238 GB");
        assert_eq!(size(1_500_000_000_000), "1.5 TB");
    }
}
