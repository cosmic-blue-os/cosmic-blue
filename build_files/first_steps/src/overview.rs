// Celenn OS Tools, the toolbox (the window's first page): where things
// stand, in one line; a search through every tool and setting; then the
// tools as tiles, in groups, each saying how things are (tools.rs asks the
// pages that know), and under them the tools for experienced users, the
// first steps tour again and the documentation. Also the "Protect your
// files" tool, whose facts are read here.
// It only reads: nothing changes from here, and nothing is ever said
// outside the window (no notifications about updates).

use crate::common::{Summary, dim_caption, fill, label, row_title, spawn, state_icon, tool_icon};
use crate::i18n::tr;
use crate::maintenance::{self, Disk};
use crate::updates_page::{self, Summary as System};
use crate::{GROUPS, Tool};
use cosmic::app::Task;
use cosmic::iced::core::text::{Ellipsize, EllipsizeHeightLimit};
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, settings, text};
use cosmic::{Apply, Element, cosmic_theme, theme};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

pub const TEXTS: &[&str] = &[
    "All is well: Celenn OS looks after itself.",
    "One thing needs your attention: {}",
    "{} things need your attention: {}",
    "Up to date, version {}",
    "Ready at the next restart.",
    "Updates are paused",
    "Could not read the version.",
    "One device has an update.",
    "{} devices have an update.",
    "Not set up yet.",
    "Last backup today.",
    "Last backup {} days ago.",
    "{} free",
    "Almost full: {} free.",
    "Backups",
    "Not set up yet. A USB drive is enough: Déjà Dup keeps a copy of your home folder.",
    "Set up",
    "Open",
    "Search a tool",
    "Nothing found.",
    "Version {}",
];

/// A backup older than this many days asks for attention
const BACKUP_DAYS: u64 = 14;

/// Every setting inside the tools that the search finds, besides the
/// tools themselves: its title, words that also find it (its caption), its
/// tool. Texts of the pages themselves, so they are already translated.
/// The kinds of apps come from apps.rs, found by their apps' names too.
const SETTINGS: &[(&str, &str, Tool)] = &[
    ("Check now", "", Tool::Update),
    ("What's new", "", Tool::Update),
    (
        "Firmware updates",
        "New versions of the BIOS, SSD, dock… from their makers.",
        Tool::Update,
    ),
    ("Get new features early", "", Tool::Advanced),
    ("An older version", "", Tool::Advanced),
    ("Another site", "WhatsApp, Netflix, Gmail…", Tool::WebApps),
    ("Disk space", "", Tool::Space),
    ("Trash", "", Tool::Space),
    (
        "Delete files that stayed in the trash for more than 30 days",
        "",
        Tool::Space,
    ),
    ("Warn when the disk is almost full", "", Tool::Space),
    ("Backups", "Déjà Dup", Tool::Backup),
    ("Printers and scanners", "", Tool::Print),
    ("Battery charge limit", "", Tool::Battery),
    ("NVIDIA graphics card", "", Tool::Nvidia),
    (
        "Firmware settings (BIOS/UEFI)",
        "Restart into the computer's own settings: boot order, virtualization, Secure Boot…",
        Tool::Advanced,
    ),
    (
        "Virtual machines",
        "virt-manager, to run Windows or another Linux in a window.",
        Tool::Advanced,
    ),
    (
        "FlatPark",
        "More apps in COSMIC Store, from outside Flathub. Less checked.",
        Tool::Advanced,
    ),
    (
        "Dark theme",
        "Light text on dark backgrounds. Off: the light theme.",
        Tool::Look,
    ),
    (
        "Layout",
        "Top bar and dock: like the Mac. One bar at the bottom: like Windows.",
        Tool::Look,
    ),
    ("Dock always visible", "", Tool::Look),
    (
        "More in Settings",
        "Accent color, wallpaper, panel, dock, fonts…",
        Tool::Look,
    ),
    ("The Celenn OS look", "", Tool::Repair),
    ("All COSMIC settings", "", Tool::Repair),
    ("Apps that come with Celenn OS", "", Tool::Repair),
    ("Documentation", "", Tool::Help),
    ("Ask a question", "", Tool::Help),
    ("Report a problem", "", Tool::Help),
];

/// At most this many results, so that the page does not scroll
const RESULTS: usize = 8;

#[derive(Clone, Debug)]
pub enum Backup {
    Never,
    /// Days since the last one
    Last(u64),
}

#[derive(Clone, Debug, Default)]
pub struct Facts {
    system: Option<Result<System, String>>,
    /// None: Déjà Dup is not installed
    backup: Option<Backup>,
    disk: Option<Disk>,
    /// Devices with a firmware update; None: fwupd did not answer
    firmware: Option<usize>,
}

#[derive(Clone, Debug)]
pub enum Message {
    Facts(Box<Facts>),
    Search(String),
    /// Opens a tool (handled by the window, tools.rs)
    Go(Tool),
    Backups,
    /// The first steps tour again
    Tour,
    /// The documentation
    Docs,
}

pub struct Page {
    facts: Option<Facts>,
    query: String,
}

fn backup() -> Option<Backup> {
    if !Command::new("sh")
        .args(["-c", "command -v deja-dup"])
        .stdout(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
    {
        return None;
    }
    let output = Command::new("gsettings")
        .args(["get", "org.gnome.DejaDup", "last-backup"])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let last = String::from_utf8_lossy(&output.stdout)
        .trim()
        .trim_matches('\'')
        .to_owned();
    if last.is_empty() {
        return Some(Backup::Never);
    }
    let output = Command::new("date")
        .args(["-d", &last, "+%s"])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let then: u64 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .ok()?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(then, |now| now.as_secs());
    Some(Backup::Last(now.saturating_sub(then) / 86_400))
}

/// The devices with a firmware update, from fwupd's own list (no
/// download: fwupd refreshes it on its own timer)
fn firmware() -> Option<usize> {
    let output = Command::new("fwupdmgr")
        .args(["get-updates", "--json", "--no-unreported-check"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    // 2: nothing to update
    if output.status.code() == Some(2) {
        return Some(0);
    }
    if !output.status.success() {
        return None;
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    Some(
        value["Devices"]
            .as_array()?
            .iter()
            .filter(|device| {
                device["Releases"]
                    .as_array()
                    .is_some_and(|releases| !releases.is_empty())
            })
            .count(),
    )
}

fn read_facts() -> Facts {
    Facts {
        system: Some(updates_page::summary()),
        backup: backup(),
        disk: maintenance::disk(),
        firmware: firmware(),
    }
}

/// Reads the facts again (each time the page is shown)
pub fn refresh() -> Task<Message> {
    Task::perform(
        async {
            tokio::task::spawn_blocking(read_facts)
                .await
                .unwrap_or_default()
        },
        |facts| cosmic::Action::App(Message::Facts(Box::new(facts))),
    )
}

/// Lower case, without accents, so that "theme" finds "Thème"
pub(crate) fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' => 'a',
            'ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ñ' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ß' => 's',
            c => c,
        })
        .collect()
}

/// The tools and settings whose title or caption hold every word of the
/// query, in the window's language or in English
fn search(query: &str) -> Vec<(&'static str, Tool)> {
    let words: Vec<String> = fold(query).split_whitespace().map(str::to_owned).collect();
    let tools = Tool::ALL.into_iter().map(|tool| {
        let (title, lead, _) = tool.texts();
        (title, lead, String::new(), tool)
    });
    let categories = crate::apps::CATEGORIES.iter().map(|category| {
        let names: Vec<&str> = category.tools().map(|tool| tool.name).collect();
        (
            category.title,
            category.caption,
            names.join(" "),
            Tool::Apps,
        )
    });
    tools
        .chain(
            SETTINGS
                .iter()
                .map(|(title, caption, tool)| (*title, *caption, String::new(), *tool)),
        )
        .chain(categories)
        .filter(|(title, caption, names, _)| {
            let mut haystack = fold(&format!("{} {} {title} {names}", tr(title), tr(caption)));
            if !caption.is_empty() {
                haystack.push(' ');
                haystack.push_str(&fold(caption));
            }
            words.iter().all(|word| haystack.contains(word.as_str()))
        })
        .map(|(title, _, _, tool)| (title, tool))
        .take(RESULTS)
        .collect()
}

impl Page {
    pub fn new() -> (Self, Task<Message>) {
        (
            Page {
                facts: None,
                query: String::new(),
            },
            refresh(),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Facts(facts) => self.facts = Some(*facts),
            Message::Search(query) => self.query = query,
            Message::Backups => spawn("deja-dup", &[]),
            Message::Tour => spawn(crate::TOUR, &[]),
            Message::Docs => spawn("xdg-open", &[&crate::common::site_url("docs/")]),
            // tools.rs opens the tool
            Message::Go(_) => {}
        }
        Task::none()
    }

    /// What the tiles of Update, Protect your files and Free up space say
    pub fn summary(&self, tool: Tool) -> Summary {
        let Some(facts) = &self.facts else {
            return Summary::Unknown;
        };
        match tool {
            Tool::Update => match &facts.system {
                Some(Ok(System { paused: true, .. })) => {
                    Summary::Warns(tr("Updates are paused").to_owned())
                }
                Some(Ok(System {
                    staged: Some(_), ..
                })) => Summary::Says(tr("Ready at the next restart.").to_owned()),
                Some(Ok(system)) => match facts.firmware {
                    Some(1) => Summary::Warns(tr("One device has an update.").to_owned()),
                    Some(devices) if devices > 1 => {
                        Summary::Warns(fill(tr("{} devices have an update."), &devices.to_string()))
                    }
                    _ => Summary::Says(fill(tr("Up to date, version {}"), &system.version)),
                },
                Some(Err(_)) => Summary::Warns(tr("Could not read the version.").to_owned()),
                None => Summary::Unknown,
            },
            Tool::Backup => match facts.backup {
                None => Summary::Hidden,
                Some(Backup::Never) => Summary::Warns(tr("Not set up yet.").to_owned()),
                Some(Backup::Last(0)) => Summary::Says(tr("Last backup today.").to_owned()),
                Some(Backup::Last(days)) => {
                    let text = fill(tr("Last backup {} days ago."), &days.to_string());
                    if days > BACKUP_DAYS {
                        Summary::Warns(text)
                    } else {
                        Summary::Says(text)
                    }
                }
            },
            Tool::Space => match &facts.disk {
                Some(disk) if disk.total > 0 && disk.used as f32 / disk.total as f32 >= 0.9 => {
                    Summary::Warns(fill(
                        tr("Almost full: {} free."),
                        &crate::units::size(disk.free),
                    ))
                }
                Some(disk) => Summary::Says(fill(tr("{} free"), &crate::units::size(disk.free))),
                None => Summary::Unknown,
            },
            _ => Summary::Unknown,
        }
    }

    /// Protect your files: Déjà Dup, how recent the last backup is
    pub fn backup_view(&self) -> Element<'_, Message> {
        let space_s = theme::spacing().space_s;
        let backup = self.facts.as_ref().and_then(|facts| facts.backup.clone());
        let (caption, button) = match backup {
            None | Some(Backup::Never) => (
                tr("Not set up yet. A USB drive is enough: Déjà Dup keeps a copy of your home folder.")
                    .to_owned(),
                widget::button::suggested(tr("Set up")),
            ),
            Some(Backup::Last(0)) => (
                tr("Last backup today.").to_owned(),
                widget::button::standard(tr("Open")),
            ),
            Some(Backup::Last(days)) => (
                fill(tr("Last backup {} days ago."), &days.to_string()),
                if days > BACKUP_DAYS {
                    widget::button::suggested(tr("Open"))
                } else {
                    widget::button::standard(tr("Open"))
                },
            ),
        };
        settings::section()
            .add(
                settings::item_row(vec![
                    state_icon("document-save-symbolic", false),
                    label(tr("Backups"), caption),
                    button.on_press(Message::Backups).into(),
                ])
                .spacing(space_s),
            )
            .into()
    }

    /// The toolbox: where things stand, the search, the tiles of the
    /// tools shown (in GROUPS' order, with what each says); and its footer
    pub fn view<'a>(
        &'a self,
        tiles: &[(Tool, Summary)],
    ) -> (Element<'a, Message>, Element<'a, Message>) {
        let cosmic_theme::Spacing {
            space_xxs,
            space_xs,
            space_s,
            ..
        } = theme::spacing();
        let attention: Vec<&str> = tiles
            .iter()
            .filter(|(_, summary)| matches!(summary, Summary::Warns(_)))
            .map(|(tool, _)| tr(tool.title()))
            .collect();
        let status: Element<'a, Message> = match attention.len() {
            0 => dim_caption(tr("All is well: Celenn OS looks after itself.")).into(),
            count => {
                let names = attention.join(", ");
                let line = if count == 1 {
                    fill(tr("One thing needs your attention: {}"), &names)
                } else {
                    fill(
                        &fill(tr("{} things need your attention: {}"), &count.to_string()),
                        &names,
                    )
                };
                widget::row::with_capacity(2)
                    .spacing(space_xxs)
                    .align_y(Alignment::Center)
                    .push(warning(
                        widget::icon::from_name("dialog-warning-symbolic")
                            .size(16)
                            .into(),
                    ))
                    .push(text::body(line))
                    .into()
            }
        };
        let search = widget::search_input(tr("Search a tool"), self.query.as_str())
            .on_input(Message::Search)
            .on_clear(Message::Search(String::new()))
            .width(Length::Fixed(360.0));
        // The group headings sit close to their tiles, so that every tile
        // fits without scrolling in the window COSMIC opens on a 1080p
        // screen (693 px high)
        let mut body = widget::column::with_capacity(8)
            .align_x(Alignment::Center)
            .push(text::title3(tr("Toolbox")))
            .push(widget::space::vertical().height(space_xxs))
            .push(status)
            .push(widget::space::vertical().height(space_s))
            .push(search)
            .push(widget::space::vertical().height(space_xs));
        if self.query.trim().is_empty() {
            let shown_in = |tools: &[Tool]| {
                tiles
                    .iter()
                    .filter(move |(tool, summary)| {
                        tools.contains(tool) && *summary != Summary::Hidden
                    })
                    .collect::<Vec<_>>()
            };
            // Tiles keep their roomy height, and get lower only when this
            // computer shows so many tools that they would not fit otherwise
            let lines: usize = GROUPS
                .iter()
                .map(|(_, tools)| shown_in(tools).len().div_ceil(3))
                .sum();
            let height = if lines > ROOMY_LINES {
                LOW_TILE
            } else {
                ROOMY_TILE
            };
            for (group, tools) in GROUPS {
                let shown: Vec<Element<'a, Message>> = shown_in(tools)
                    .into_iter()
                    .map(|(tool, summary)| tile(*tool, summary, height))
                    .collect();
                body = body
                    .push(
                        text::heading(tr(group))
                            .width(Length::Fill)
                            .apply(widget::container)
                            .padding([space_xs, 0, space_xxs, 0]),
                    )
                    .push(grid(shown));
            }
        } else {
            body = body
                .push(widget::space::vertical().height(space_s))
                .push(self.results());
        }
        let footer = widget::row::with_capacity(4)
            .spacing(space_xxs)
            .align_y(Alignment::Center)
            .push(
                widget::button::standard(tr("For experienced users…"))
                    .on_press(Message::Go(Tool::Advanced)),
            )
            .push(widget::space::horizontal().width(Length::Fill))
            .push(widget::button::standard(tr("First steps again")).on_press(Message::Tour))
            .push(widget::tooltip(
                widget::button::icon(widget::icon::from_name("help-browser-symbolic"))
                    .name(tr("Help on this tool"))
                    .on_press(Message::Docs),
                text::body(tr("Help on this tool")),
                widget::tooltip::Position::Top,
            ));
        (body.into(), footer.into())
    }

    fn results(&self) -> Element<'_, Message> {
        let found = search(&self.query);
        if found.is_empty() {
            return text::body(tr("Nothing found.")).into();
        }
        let mut section = settings::section();
        for (title, tool) in found {
            section = section.add(
                settings::item_row(vec![
                    label(title, tr(tool.title()).to_owned()),
                    widget::button::standard(tr("Open"))
                        .on_press(Message::Go(tool))
                        .into(),
                ])
                .align_y(Alignment::Center),
            );
        }
        section.into()
    }
}

/// In the warning color
fn warning<'a>(element: Element<'a, Message>) -> Element<'a, Message> {
    element
        .apply(widget::container)
        .class(theme::Container::custom(|theme| {
            let color = theme.cosmic().warning_color();
            cosmic::iced::widget::container::Style {
                icon_color: Some(color.into()),
                text_color: Some(color.into()),
                ..Default::default()
            }
        }))
        .into()
}

/// A tool's tile: its round symbol, its title, what it says (in the warning
/// color when it needs attention), as one button that opens it
/// Tile texts stay on one line, cut with an ellipsis when too long
const ONE_LINE: Ellipsize = Ellipsize::End(EllipsizeHeightLimit::Lines(1));

/// A tile's usual height, and the lower one used when more than
/// ROOMY_LINES lines of tiles are shown (a laptop with an NVIDIA card and
/// other disks shows six), so that every tile fits without scrolling in the
/// window COSMIC opens on a 1080p screen (693 px high)
const ROOMY_TILE: f32 = 54.0;
const LOW_TILE: f32 = 44.0;
const ROOMY_LINES: usize = 5;

fn tile<'a>(tool: Tool, summary: &Summary, height: f32) -> Element<'a, Message> {
    let cosmic_theme::Spacing { space_s, .. } = theme::spacing();
    let (title, _, usual) = tool.texts();
    let caption: Element<'a, Message> = match summary {
        Summary::Warns(caption) => {
            warning(text::caption(caption.clone()).ellipsize(ONE_LINE).into())
        }
        Summary::Says(caption) => dim_caption(caption.clone()).ellipsize(ONE_LINE).into(),
        Summary::Unknown | Summary::Hidden => dim_caption(tr(usual)).ellipsize(ONE_LINE).into(),
    };
    widget::row::with_capacity(2)
        .spacing(space_s)
        .align_y(Alignment::Center)
        .push(tool_icon(tool.icon(), 36))
        .push(
            widget::column::with_capacity(2)
                .spacing(2)
                .push(row_title(tr(title)).ellipsize(ONE_LINE))
                .push(caption)
                .width(Length::Fill)
                .clip(true),
        )
        .padding([0, space_s])
        .height(Length::Fill)
        .apply(widget::button::custom)
        .class(crate::style::tile_style(false))
        .padding(0)
        .width(Length::Fill)
        .height(Length::Fixed(height))
        .on_press(Message::Go(tool))
        .into()
}

/// Tiles in three columns
fn grid<'a>(tiles: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let space_xs = theme::spacing().space_xs;
    let mut lines = widget::column::with_capacity(tiles.len().div_ceil(3)).spacing(space_xs);
    let mut tiles = tiles.into_iter().peekable();
    while tiles.peek().is_some() {
        let mut line = widget::row::with_capacity(3).spacing(space_xs);
        let mut count = 0;
        for tile in tiles.by_ref().take(3) {
            line = line.push(tile);
            count += 1;
        }
        for _ in count..3 {
            line = line.push(widget::space::horizontal().width(Length::Fill));
        }
        lines = lines.push(line);
    }
    lines.width(Length::Fill).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Open Sans (COSMIC's interface font) advance widths, in thousandths
    /// of the font size, of the characters the Tools texts use
    const OPEN_SANS: &[(char, u16)] = &[
        (' ', 260),
        ('%', 823),
        ('\'', 221),
        ('(', 296),
        (')', 296),
        (',', 245),
        ('-', 322),
        ('.', 266),
        ('/', 367),
        ('0', 572),
        ('1', 572),
        ('2', 572),
        ('3', 572),
        ('4', 572),
        ('5', 572),
        ('6', 572),
        ('7', 572),
        ('8', 572),
        ('9', 572),
        (':', 266),
        (';', 266),
        ('?', 429),
        ('A', 633),
        ('B', 648),
        ('C', 631),
        ('D', 729),
        ('E', 556),
        ('F', 516),
        ('G', 728),
        ('H', 738),
        ('I', 279),
        ('J', 267),
        ('K', 614),
        ('L', 519),
        ('M', 903),
        ('N', 754),
        ('O', 779),
        ('P', 602),
        ('Q', 779),
        ('R', 618),
        ('S', 549),
        ('T', 553),
        ('U', 728),
        ('V', 595),
        ('W', 926),
        ('X', 577),
        ('Y', 560),
        ('Z', 571),
        ('a', 556),
        ('b', 613),
        ('c', 476),
        ('d', 613),
        ('e', 561),
        ('f', 339),
        ('g', 548),
        ('h', 614),
        ('i', 253),
        ('j', 253),
        ('k', 525),
        ('l', 253),
        ('m', 930),
        ('n', 614),
        ('o', 604),
        ('p', 613),
        ('q', 613),
        ('r', 408),
        ('s', 477),
        ('t', 353),
        ('u', 614),
        ('v', 501),
        ('w', 778),
        ('x', 524),
        ('y', 504),
        ('z', 468),
        ('{', 379),
        ('}', 379),
        ('«', 497),
        ('»', 497),
        ('¿', 429),
        ('À', 633),
        ('Ä', 633),
        ('É', 556),
        ('Ê', 556),
        ('Ó', 779),
        ('Ö', 779),
        ('Ú', 728),
        ('Ü', 728),
        ('ß', 622),
        ('à', 556),
        ('á', 556),
        ('ã', 556),
        ('ä', 556),
        ('ç', 476),
        ('è', 561),
        ('é', 561),
        ('ê', 561),
        ('í', 253),
        ('î', 253),
        ('ñ', 614),
        ('ò', 604),
        ('ó', 604),
        ('ô', 604),
        ('õ', 604),
        ('ö', 604),
        ('ù', 614),
        ('ú', 614),
        ('û', 614),
        ('ü', 614),
        ('œ', 942),
        ('’', 170),
        ('“', 350),
        ('”', 350),
        ('„', 405),
        ('…', 784),
    ];

    /// A text's width in pixels at a size, a character missing from the
    /// table counted as wide as the font size (wider than any letter)
    fn width(text: &str, size: f32) -> f32 {
        text.chars()
            .map(|c| {
                OPEN_SANS
                    .iter()
                    .find(|(known, _)| *known == c)
                    .map_or(1000, |(_, advance)| *advance)
            })
            .map(|advance| f32::from(advance) * size / 1000.0)
            .sum()
    }

    /// Every tile caption fits on its one line in every language: in the
    /// window's default size a tile leaves about 205 px to its caption
    /// (12 px text; "Guide, questions, report a problem", 196 px, shows
    /// whole). Captions with a value get a long example of it. A new tile
    /// caption goes in this list.
    #[test]
    fn tile_captions_fit() {
        let mut captions: Vec<(&str, &str)> = Tool::ALL
            .iter()
            .filter(|tool| **tool != Tool::Advanced)
            .map(|tool| (tool.texts().2, ""))
            .collect();
        captions.extend([
            ("Up to date, version {}", "45.20261128.1"),
            ("Ready at the next restart.", ""),
            ("Updates are paused", ""),
            ("One device has an update.", ""),
            ("{} devices have an update.", "12"),
            ("Could not read the version.", ""),
            ("Not set up yet.", ""),
            ("Last backup today.", ""),
            ("Last backup {} days ago.", "99"),
            ("Almost full: {} free.", "999.9 MB"),
            ("{} free", "999.9 GB"),
            ("No printer added yet.", ""),
            ("Charging stops at 80 %.", ""),
            ("Off: charges to 100 %.", ""),
            ("None opens at startup yet.", ""),
            ("Dark theme", ""),
            ("Light theme", ""),
            ("Its driver waits for your OK.", ""),
            ("NVIDIA's own driver.", ""),
            ("The open Linux driver.", ""),
            ("Installed.", ""),
        ]);
        let po = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("po/tools");
        let mut catalogs = vec![("en".to_owned(), Vec::new())];
        for entry in std::fs::read_dir(&po).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "po") {
                let lang = path.file_stem().unwrap().to_string_lossy().into_owned();
                let text = std::fs::read_to_string(&path).unwrap();
                catalogs.push((lang, crate::i18n::parse_po(&text)));
            }
        }
        assert!(catalogs.len() > 1, "no catalog in {}", po.display());
        let mut too_long = Vec::new();
        for (lang, catalog) in &catalogs {
            for (text, value) in &captions {
                let translated = catalog
                    .iter()
                    .find(|(id, _)| id == text)
                    .map_or(*text, |(_, translation)| translation.as_str());
                let shown = fill(translated, value);
                let px = width(&shown, 12.0);
                if px > 200.0 {
                    too_long.push(format!("{lang}: {shown} ({px:.0} px)"));
                }
            }
        }
        assert!(
            too_long.is_empty(),
            "tile captions cut:\n{}",
            too_long.join("\n")
        );
    }

    #[test]
    fn folds_accents() {
        assert_eq!(fold("Thème Sombre"), "theme sombre");
    }

    #[test]
    fn finds_settings() {
        let pages = |query| {
            search(query)
                .into_iter()
                .map(|(_, page)| page)
                .collect::<Vec<_>>()
        };
        assert_eq!(pages("steam"), [Tool::Apps]);
        assert!(pages("dark").contains(&Tool::Look));
        assert!(pages("BIOS").contains(&Tool::Advanced));
        assert!(pages("backup").contains(&Tool::Backup));
        assert!(pages("nothing like this").is_empty());
        // Every word counts
        assert_eq!(search("battery limit").len(), 1);
    }
}
