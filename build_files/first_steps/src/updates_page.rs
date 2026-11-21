// cosmic-blue Updates: the system version in one window, without jargon.
//
//   - the current version, a "Check now" button (uupd-manual.service, the
//     same update as the daily timer: system, flatpaks, Homebrew), and the
//     version waiting for the next restart, if any;
//   - the previous version, with a button that goes back to it (bootc
//     rollback), then offers to restart;
//   - one switch, "Get new features early": off follows the stable channel
//     (one tested image a week, the default), on follows testing (every
//     day). dev stays a ujust command for testers;
//   - last, in its own quieter row, "An older version": the stable image of
//     a past week (stable-YYYYMMDD tags), which pauses updates until
//     "Resume updates". The previous version above stays the one to go
//     back to.
//
// Changing the version needs root: the app runs
// /usr/libexec/cosmic-blue-updates-helper through pkexec, which asks for an
// administrator password (polkit action io.github.cosmic_blue_os.updates.manage)
// and only accepts those changes. After going back, that helper records
// the version left behind (/var/lib/cosmic-blue/update-hold), so that the
// automatic updates do not install it again (/usr/libexec/cosmic-blue-bootc);
// they start again with the next version.
// No notifications: the system updates by itself, this is for when people
// want to look or act.
//
// This is the first page of cosmic-blue Tools (tools.rs), which the Updates
// launcher opens directly (`cosmic-blue-tools --updates`, and
// /usr/libexec/cosmic-blue-updates, a link to it). From the command line:
//   cosmic-blue-tools --status   what the page would show, as text (tests)
//   cosmic-blue-tools --older    the older stable versions it would list
//
// The look is the tours' (main.rs): same banner, title, rows and buttons.

use crate::common::{fill, header, run, state_icon, two_lines};
use crate::i18n::tr;
use cosmic::app::Task;
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, settings, text, toaster::Toasts};
use cosmic::{Apply, Element, cosmic_theme, theme};

const HELPER: &str = "/usr/libexec/cosmic-blue-updates-helper";
/// The version left behind by the last rollback: "<digest> <version>"
const HOLD: &str = "/var/lib/cosmic-blue/update-hold";
const RELEASES: &str = "https://github.com/cosmic-blue-os/cosmic-blue/releases";
const VENDOR: &str = "ghcr.io/cosmic-blue-os/";
const ICON: &str = "cosmic-blue-update";

/// Every text, for the translation check
pub const TEXTS: &[&str] = &[
    "Updates",
    "cosmic-blue updates itself in the background. A new version starts at the next restart, without interrupting you.",
    "Current version: {}",
    "Weekly updates, tested before they come out",
    "Daily updates, sooner but less tested",
    "dev channel: every change, for testers",
    "This system stays on this version: it does not update by itself.",
    "Check now",
    "Checking…",
    "You are up to date.",
    "The check did not work. Are you online?",
    "Ready for the next restart: {}",
    "Downloaded and checked. It takes effect at the next restart.",
    "Restart",
    "Previous version: {}",
    "Something wrong since the last update? Go back to this version. Your files and settings stay as they are.",
    "Go back to it",
    "Back to {} at the next restart",
    "Your files and settings stay as they are. The version you leave will not be installed again: updates start again with the next one.",
    "Version {} is set aside",
    "You went back from it, so it will not be installed again. Updates start again with the next version.",
    "Get new features early",
    "Off (recommended): one tested update a week.\nOn: an update every day, sooner but less tested.",
    "You follow the dev channel, chosen in a terminal. Turn this off to go back to one tested update a week.",
    "Downloading the new version… This can take a few minutes.",
    "Something went wrong, nothing was changed.",
    "The system version could not be read: is this computer installed with cosmic-blue?",
    "Apps from COSMIC Store update by themselves too.",
    "What's new",
    "An older version",
    "Choose the stable version of a past week. The system then stays on it, without updates, until you resume them.",
    "Choose…",
    "Hide",
    "Stable version of {}",
    "Use this one",
    "No older version found. Are you online?",
    "{y}-{m}-{d}",
    "Updates are paused",
    "This system stays on this version. Resume them to get one tested update a week again.",
    "Resume updates",
];

/// --older and --status: Some(exit code) when one of them is given
pub fn command_line(args: &[String]) -> Option<i32> {
    let older = args.iter().any(|arg| arg == "--older");
    if !older && !args.iter().any(|arg| arg == "--status") {
        return None;
    }
    let status = match read_status() {
        Ok(status) => status,
        Err(error) => {
            eprintln!("{error}");
            return Some(1);
        }
    };
    if older {
        for tag in stable_tags(status.booted.image()) {
            println!("{tag}");
        }
    } else {
        println!("{}", status.summary());
    }
    Some(0)
}

/// One deployment (a version of the system on the disk), as rpm-ostree
/// lists them
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Deployment {
    version: String,
    /// "ostree-image-signed:docker://ghcr.io/cosmic-blue-os/cosmic-blue:stable"
    reference: String,
    booted: bool,
    staged: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Channel {
    Stable,
    Testing,
    Dev,
    /// A date or version tag: no automatic updates
    Pinned,
}

impl Deployment {
    /// The image name with its tag
    fn image(&self) -> &str {
        let image = self
            .reference
            .rsplit_once("docker://")
            .map_or(self.reference.as_str(), |(_, image)| image);
        image
            .strip_prefix("ostree-unverified-registry:")
            .unwrap_or(image)
    }

    /// The image tag ("stable", "stable-20261119"...)
    fn tag(&self) -> Option<&str> {
        self.image().rsplit_once(':').map(|(_, tag)| tag)
    }

    fn channel(&self) -> Option<Channel> {
        let image = self.image();
        if !image.starts_with(VENDOR) || image.contains('@') {
            return None;
        }
        let (_, tag) = image.rsplit_once(':')?;
        Some(match tag {
            "stable" => Channel::Stable,
            "testing" | "latest" => Channel::Testing,
            "dev" => Channel::Dev,
            _ => Channel::Pinned,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct Status {
    booted: Deployment,
    /// Downloaded update, which starts at the next restart
    staged: Option<Deployment>,
    /// The other version on the disk, one rollback away
    previous: Option<Deployment>,
    /// True when the previous version starts at the next restart (after a
    /// rollback)
    rollback_queued: bool,
    /// Version left behind by the last rollback
    held: Option<String>,
}

impl Status {
    /// The channel the next restart follows (switching channel stages the
    /// image of the new one)
    fn channel(&self) -> Option<Channel> {
        self.staged.as_ref().unwrap_or(&self.booted).channel()
    }

    fn summary(&self) -> String {
        let version = |d: Option<&Deployment>| d.map_or("none".to_owned(), |d| d.version.clone());
        format!(
            "current: {}\nchannel: {}\nstaged: {}\nprevious: {}\nrollback-queued: {}\nheld: {}",
            self.booted.version,
            match self.channel() {
                Some(Channel::Stable) => "stable",
                Some(Channel::Testing) => "testing",
                Some(Channel::Dev) => "dev",
                Some(Channel::Pinned) => "pinned",
                None => "unknown",
            },
            version(self.staged.as_ref()),
            version(self.previous.as_ref()),
            if self.rollback_queued { "yes" } else { "no" },
            self.held.as_deref().unwrap_or("none"),
        )
    }
}

/// The deployments from `rpm-ostree status --json` (readable without root),
/// or from the file named by COSMIC_BLUE_UPDATES_STATUS_JSON (previews)
fn read_status() -> Result<Status, String> {
    let json = match std::env::var_os("COSMIC_BLUE_UPDATES_STATUS_JSON") {
        Some(path) => std::fs::read_to_string(path).map_err(|error| error.to_string())?,
        None => {
            let output = std::process::Command::new("rpm-ostree")
                .args(["status", "--json"])
                .output()
                .map_err(|error| format!("rpm-ostree: {error}"))?;
            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
            }
            String::from_utf8_lossy(&output.stdout).into_owned()
        }
    };
    let value: serde_json::Value =
        serde_json::from_str(&json).map_err(|error| error.to_string())?;
    let deployments: Vec<Deployment> = value["deployments"]
        .as_array()
        .ok_or("rpm-ostree lists no deployments")?
        .iter()
        .map(|d| Deployment {
            version: d["version"].as_str().unwrap_or("?").to_owned(),
            reference: d["container-image-reference"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            booted: d["booted"].as_bool().unwrap_or(false),
            staged: d["staged"].as_bool().unwrap_or(false),
        })
        .collect();
    let booted = deployments
        .iter()
        .find(|d| d.booted)
        .cloned()
        .ok_or("no booted deployment")?;
    // The first deployment is the one the next restart starts
    let rollback_queued = deployments.first().is_some_and(|d| !d.booted && !d.staged);
    let held = std::fs::read_to_string(HOLD)
        .ok()
        .and_then(|hold| hold.split_whitespace().nth(1).map(str::to_owned));
    Ok(Status {
        staged: deployments.iter().find(|d| d.staged).cloned(),
        previous: deployments.iter().find(|d| !d.booted && !d.staged).cloned(),
        booted,
        rollback_queued,
        held,
    })
}

/// What runs in the background, shown instead of the buttons it concerns
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Busy {
    Checking,
    Switching,
    RollingBack,
    /// Switching to an older stable version
    Pinning,
}

#[derive(Clone, Debug)]
pub enum Message {
    Status(Result<Status, String>),
    CheckNow,
    Rollback,
    Early(bool),
    /// Show or hide the list of older stable versions
    ShowOlder,
    Older(Vec<String>),
    /// Stay on an older stable version (its tag)
    Pin(String),
    /// End of a background command: what it was, and whether it worked
    /// (None: the password prompt was cancelled)
    Done(Busy, Option<bool>),
    Restart,
    OpenReleases,
    CloseToast(widget::ToastId),
}

pub struct Page {
    status: Option<Result<Status, String>>,
    busy: Option<Busy>,
    /// Tags of the older stable versions, once listed (None: list closed)
    older: Option<Vec<String>>,
    toasts: Toasts<Message>,
}

fn refresh() -> Task<Message> {
    Task::perform(
        async {
            tokio::task::spawn_blocking(read_status)
                .await
                .unwrap_or_else(|e| Err(e.to_string()))
        },
        |status| cosmic::Action::App(Message::Status(status)),
    )
}

fn background(busy: Busy, program: &'static str, args: &[&str]) -> Task<Message> {
    let args = args.iter().map(|arg| (*arg).to_owned()).collect();
    Task::perform(run(program, args), move |result| {
        cosmic::Action::App(Message::Done(busy, result))
    })
}

impl Page {
    fn toast(&mut self, message: &'static str) -> Task<Message> {
        self.toasts
            .push(widget::toaster::Toast::new(tr(message)))
            .map(cosmic::Action::App)
    }
}

impl Page {
    pub fn new() -> (Self, Task<Message>) {
        let page = Page {
            status: None,
            busy: None,
            older: None,
            toasts: Toasts::new(Message::CloseToast),
        };
        (page, refresh())
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Status(status) => self.status = Some(status),
            Message::CheckNow => {
                self.busy = Some(Busy::Checking);
                // Allowed without a password by uupd's own polkit rule
                return background(
                    Busy::Checking,
                    "systemctl",
                    &["start", "uupd-manual.service"],
                );
            }
            Message::Rollback => {
                self.busy = Some(Busy::RollingBack);
                return background(Busy::RollingBack, "pkexec", &[HELPER, "rollback"]);
            }
            Message::Early(early) => {
                self.busy = Some(Busy::Switching);
                let channel = if early { "testing" } else { "stable" };
                return background(Busy::Switching, "pkexec", &[HELPER, "channel", channel]);
            }
            Message::ShowOlder => {
                if self.older.take().is_none() {
                    let image = self
                        .status
                        .as_ref()
                        .and_then(|status| status.as_ref().ok())
                        .map(|status| status.booted.image().to_owned())
                        .unwrap_or_default();
                    return Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || stable_tags(&image))
                                .await
                                .unwrap_or_default()
                        },
                        |tags| cosmic::Action::App(Message::Older(tags)),
                    );
                }
            }
            Message::Older(tags) => {
                if tags.is_empty() {
                    return self.toast("No older version found. Are you online?");
                }
                self.older = Some(tags);
            }
            Message::Pin(tag) => {
                self.busy = Some(Busy::Pinning);
                self.older = None;
                return background(Busy::Pinning, "pkexec", &[HELPER, "pin", &tag]);
            }
            Message::Done(busy, result) => {
                self.busy = None;
                let staged_before = self
                    .status
                    .as_ref()
                    .and_then(|status| status.as_ref().ok())
                    .and_then(|status| status.staged.clone());
                let toast = match (busy, result) {
                    (_, None) => Task::none(),
                    (Busy::Checking, Some(false)) => {
                        self.toast("The check did not work. Are you online?")
                    }
                    (_, Some(false)) => self.toast("Something went wrong, nothing was changed."),
                    (Busy::Checking, Some(true)) => {
                        // Nothing new staged by the check: say so (a new
                        // version shows as its own row)
                        match read_status() {
                            Ok(status) if status.staged == staged_before => {
                                self.toast("You are up to date.")
                            }
                            _ => Task::none(),
                        }
                    }
                    (_, Some(true)) => Task::none(),
                };
                return Task::batch([toast, refresh()]);
            }
            Message::Restart => {
                if let Err(error) = std::process::Command::new("systemctl")
                    .arg("reboot")
                    .spawn()
                {
                    eprintln!("Could not restart: {error}");
                }
            }
            Message::OpenReleases => {
                if let Err(error) = std::process::Command::new("xdg-open").arg(RELEASES).spawn() {
                    eprintln!("Could not open {RELEASES}: {error}");
                }
            }
            Message::CloseToast(id) => self.toasts.remove(id),
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let cosmic_theme::Spacing {
            space_xxs,
            space_s,
            space_m: _,
            space_l: _,
            ..
        } = theme::spacing();

        let mut content = widget::column::with_capacity(8)
            .spacing(space_s)
            .align_x(Alignment::Center)
            .push(
                text::body(tr(
                    "cosmic-blue updates itself in the background. A new version starts at the next restart, without interrupting you.",
                ))
                .align_x(Alignment::Center)
                .width(Length::Fill),
            );
        match &self.status {
            None => {}
            Some(Err(error)) => {
                eprintln!("{error}");
                content = content.push(
                    settings::section().add(settings::item_row(vec![two_lines(
                        text::body(tr(
                            "The system version could not be read: is this computer installed with cosmic-blue?",
                        ))
                        .into(),
                        error.clone(),
                    )])),
                );
            }
            Some(Ok(status)) => {
                content = content.push(self.versions(status, space_s));
                if let Some(section) = self.channel_section(status, space_s) {
                    content = content.push(section);
                }
                if let Some(section) = self.older_section(status, space_s) {
                    content = content.push(section);
                }
            }
        }
        // One line at the bottom, so that the window needs no scroll bar
        // while nothing is unfolded, even on small screens
        content = content.push(
            widget::row::with_capacity(2)
                .spacing(space_xxs)
                .align_y(Alignment::Center)
                .push(text::caption(tr(
                    "Apps from COSMIC Store update by themselves too.",
                )))
                .push(
                    widget::button::link(tr("What's new"))
                        .trailing_icon(true)
                        .on_press(Message::OpenReleases),
                )
                .apply(widget::container)
                .center_x(Length::Fill),
        );

        let page = header(ICON, tr("Updates"), content.into());
        widget::toaster(&self.toasts, page)
    }
}

impl Page {
    /// Current version, update waiting for a restart, previous version
    fn versions<'a>(&'a self, status: &'a Status, space_s: u16) -> Element<'a, Message> {
        let idle = self.busy.is_none();
        let mut section = settings::section();

        let channel_text = match status.booted.channel() {
            Some(Channel::Stable) => tr("Weekly updates, tested before they come out"),
            Some(Channel::Testing) => tr("Daily updates, sooner but less tested"),
            Some(Channel::Dev) => tr("dev channel: every change, for testers"),
            Some(Channel::Pinned) | None => {
                tr("This system stays on this version: it does not update by itself.")
            }
        };
        let check: Element<'a, Message> = if self.busy == Some(Busy::Checking) {
            text::body(tr("Checking…")).into()
        } else {
            widget::button::standard(tr("Check now"))
                .on_press_maybe(idle.then_some(Message::CheckNow))
                .into()
        };
        section = section.add(
            settings::item_row(vec![
                state_icon("emblem-ok-symbolic", true),
                two_lines(
                    text::body(fill(tr("Current version: {}"), &status.booted.version)).into(),
                    channel_text.to_owned(),
                ),
                check,
            ])
            .spacing(space_s),
        );

        if status.rollback_queued {
            if let Some(previous) = &status.previous {
                section = section.add(
                    settings::item_row(vec![
                        state_icon("system-reboot-symbolic", false),
                        two_lines(
                            text::body(fill(tr("Back to {} at the next restart"), &previous.version))
                                .into(),
                            tr("Your files and settings stay as they are. The version you leave will not be installed again: updates start again with the next one.").to_owned(),
                        ),
                        widget::button::suggested(tr("Restart"))
                            .on_press(Message::Restart)
                            .into(),
                    ])
                    .spacing(space_s),
                );
            }
            return section.into();
        }

        if let Some(staged) = &status.staged {
            section = section.add(
                settings::item_row(vec![
                    state_icon("software-update-available-symbolic", false),
                    two_lines(
                        text::body(fill(tr("Ready for the next restart: {}"), &staged.version))
                            .into(),
                        tr("Downloaded and checked. It takes effect at the next restart.")
                            .to_owned(),
                    ),
                    widget::button::suggested(tr("Restart"))
                        .on_press(Message::Restart)
                        .into(),
                ])
                .spacing(space_s),
            );
        }

        if let Some(held) = &status.held {
            section = section.add(
                settings::item_row(vec![
                    state_icon("media-playback-pause-symbolic", false),
                    two_lines(
                        text::body(fill(tr("Version {} is set aside"), held)).into(),
                        tr("You went back from it, so it will not be installed again. Updates start again with the next version.").to_owned(),
                    ),
                ])
                .spacing(space_s),
            );
        }

        if let Some(previous) = &status.previous {
            let button: Element<'a, Message> = if self.busy == Some(Busy::RollingBack) {
                widget::progress_bar::indeterminate_circular().into()
            } else {
                widget::button::standard(tr("Go back to it"))
                    .on_press_maybe(idle.then_some(Message::Rollback))
                    .into()
            };
            section = section.add(
                settings::item_row(vec![
                    state_icon("edit-undo-symbolic", false),
                    two_lines(
                        text::body(fill(tr("Previous version: {}"), &previous.version)).into(),
                        tr("Something wrong since the last update? Go back to this version. Your files and settings stay as they are.").to_owned(),
                    ),
                    button,
                ])
                .spacing(space_s),
            );
        }
        section.into()
    }

    /// The "Get new features early" switch (stable or testing), for systems
    /// that follow a channel
    fn channel_section<'a>(
        &'a self,
        status: &'a Status,
        space_s: u16,
    ) -> Option<Element<'a, Message>> {
        let channel = status.channel()?;
        if channel == Channel::Pinned {
            let button: Element<'a, Message> =
                if matches!(self.busy, Some(Busy::Switching | Busy::Pinning)) {
                    widget::progress_bar::indeterminate_circular().into()
                } else {
                    widget::button::standard(tr("Resume updates"))
                        .on_press_maybe(
                            (self.busy.is_none() && !status.rollback_queued)
                                .then_some(Message::Early(false)),
                        )
                        .into()
                };
            return Some(
                settings::section()
                    .add(
                        settings::item_row(vec![
                            state_icon("media-playback-pause-symbolic", false),
                            two_lines(
                                text::body(tr("Updates are paused")).into(),
                                tr("This system stays on this version. Resume them to get one tested update a week again.").to_owned(),
                            ),
                            button,
                        ])
                        .spacing(space_s),
                    )
                    .into(),
            );
        }
        let caption = if matches!(self.busy, Some(Busy::Switching | Busy::Pinning)) {
            tr("Downloading the new version… This can take a few minutes.")
        } else if channel == Channel::Dev {
            tr(
                "You follow the dev channel, chosen in a terminal. Turn this off to go back to one tested update a week.",
            )
        } else {
            tr(
                "Off (recommended): one tested update a week.\nOn: an update every day, sooner but less tested.",
            )
        };
        let toggler = widget::toggler(channel != Channel::Stable);
        let toggler = if self.busy.is_none() && !status.rollback_queued {
            toggler.on_toggle(Message::Early)
        } else {
            toggler
        };
        Some(
            settings::section()
                .add(
                    settings::item_row(vec![
                        two_lines(
                            text::body(tr("Get new features early")).into(),
                            caption.to_owned(),
                        ),
                        toggler.into(),
                    ])
                    .spacing(space_s),
                )
                .into(),
        )
    }
}

impl Page {
    /// "An older version": the stable versions of past weeks, listed once
    /// "Choose…" is clicked (the previous version above is the one put
    /// forward)
    fn older_section<'a>(
        &'a self,
        status: &'a Status,
        space_s: u16,
    ) -> Option<Element<'a, Message>> {
        status.channel()?;
        let available = self.busy.is_none() && !status.rollback_queued;
        let current = status.staged.as_ref().unwrap_or(&status.booted).tag();
        let mut section = settings::section().add(
            settings::item_row(vec![
                two_lines(
                    text::body(tr("An older version")).into(),
                    tr("Choose the stable version of a past week. The system then stays on it, without updates, until you resume them.").to_owned(),
                ),
                widget::button::standard(tr(if self.older.is_some() { "Hide" } else { "Choose…" }))
                    .on_press_maybe(available.then_some(Message::ShowOlder))
                    .into(),
            ])
            .spacing(space_s),
        );
        let tags = self.older.as_deref().unwrap_or_default();
        for tag in tags.iter().filter(|tag| Some(tag.as_str()) != current) {
            section = section.add(
                settings::item_row(vec![
                    text::body(fill(tr("Stable version of {}"), &date(tag)))
                        .width(Length::Fill)
                        .into(),
                    widget::button::standard(tr("Use this one"))
                        .on_press_maybe(available.then(|| Message::Pin(tag.clone())))
                        .into(),
                ])
                .spacing(space_s),
            );
        }
        Some(section.into())
    }
}

/// Older stable versions of the image (newest first, at most 8): its
/// stable-YYYYMMDD tags, listed anonymously from the registry. The newest
/// is the current stable, which the "Get new features early" switch covers.
fn stable_tags(image: &str) -> Vec<String> {
    let Some(path) = image
        .strip_prefix("ghcr.io/")
        .and_then(|rest| rest.rsplit_once(':'))
        .map(|(path, _)| path.to_owned())
    else {
        return Vec::new();
    };
    let curl = |args: &[&str]| -> Option<serde_json::Value> {
        let output = std::process::Command::new("curl")
            .args(["-fsSL", "--max-time", "20"])
            .args(args)
            .output()
            .ok()?;
        if !output.status.success() {
            eprintln!("curl: {}", String::from_utf8_lossy(&output.stderr).trim());
            return None;
        }
        serde_json::from_slice(&output.stdout).ok()
    };
    let Some(token) = curl(&[&format!(
        "https://ghcr.io/token?scope=repository:{path}:pull"
    )])
    .and_then(|value| value["token"].as_str().map(str::to_owned)) else {
        return Vec::new();
    };
    let Some(list) = curl(&[
        "-H",
        &format!("Authorization: Bearer {token}"),
        &format!("https://ghcr.io/v2/{path}/tags/list?n=10000"),
    ]) else {
        return Vec::new();
    };
    let mut tags: Vec<String> = list["tags"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|tag| tag.as_str())
        .filter(|tag| is_stable_tag(tag))
        .map(str::to_owned)
        .collect();
    tags.sort_unstable_by(|a, b| b.cmp(a));
    tags.into_iter().skip(1).take(8).collect()
}

fn is_stable_tag(tag: &str) -> bool {
    tag.strip_prefix("stable-")
        .is_some_and(|date| date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()))
}

/// "stable-20261119" as a date in the session language
fn date(tag: &str) -> String {
    let digits = tag.trim_start_matches("stable-");
    if digits.len() != 8 {
        return tag.to_owned();
    }
    tr("{y}-{m}-{d}")
        .replace("{y}", &digits[..4])
        .replace("{m}", &digits[4..6])
        .replace("{d}", &digits[6..])
}
