// "Change computers" (a tool of Celenn OS Tools): the settings people choose and
// the list of their apps, saved to a JSON file, then taken back on another
// Celenn OS computer.
//
// What travels: the COSMIC settings of SETTINGS (theme, wallpaper of
// Celenn OS, panel, dock, keyboard, shortcuts), the Flathub apps installed
// on top of the first-boot ones, and the developer tools ticked in Tools.
// Never a personal file, a password, an account or browser history, nor
// what belongs to the machine (screens, scale, hardware).
//
// Taking a file back is careful, because a file can come from anyone: only
// the settings of SETTINGS are written (each key a plain name, its value
// text of a bounded size), only valid Flathub app ids are installed, only
// developer tools of our own list, and nothing in the file is ever run (a
// shortcut that starts a program is left out). The COSMIC settings folder is
// copied first, as `ujust reset-cosmic-config` does. Whatever is left out is
// said, with why.
//
// Blocking functions: called off the window's thread.

use crate::devtools;
use crate::i18n::{fill, tr};
use crate::system;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const TEXTS: &[&str] = &[
    "Change computers",
    "Save my settings",
    "Look, keyboard, shortcuts and the list of your apps, in a file in Documents. Copy it to a USB drive to take it along.",
    "Restore my settings",
    "On the new computer, choose the file saved on the old one, then tick what you want back.",
    "My files",
    "Documents and photos are not in the settings file: copy them to a USB drive, or send them over Wi-Fi with LocalSend, open on both computers.",
    "Open LocalSend",
    "Restore…",
    "Saved in Documents.",
    "Show",
    "Choose the file saved on your other computer",
    "celenn-settings",
    "Restore these settings",
    "Saved on {} on “{}”. Tick what you want back. Your files are not in this file: use Déjà Dup or LocalSend for them.",
    "Saved on {}. Tick what you want back. Your files are not in this file: use Déjà Dup or LocalSend for them.",
    "Look",
    "Dark or light theme, accent color, wallpaper, panel and dock.",
    "Keyboard and shortcuts",
    "Keyboard layout and the shortcuts you changed.",
    "Apps ({})",
    "{} and {} more, from Flathub.",
    "{}, from Flathub.",
    "Developer tools ({})",
    "Take them back",
    "Restoring…",
    "Restored",
    "Close",
    "Your own wallpaper is not in the file: copy the picture, then choose it in Settings.",
    "{} shortcuts start a program: for safety, add them again in Settings.",
    "One shortcut starts a program: for safety, add it again in Settings.",
    "{} settings unknown to this version of Celenn OS were left out.",
    "{} app names were not valid and were left out.",
    "The apps could not be installed.",
    "A copy of your previous settings is in {}.",
    "Log out and back in to see every change.",
    "This file is not a Celenn OS settings file.",
    "This file was saved by a newer Celenn OS. Update this computer first.",
    "This file could not be read.",
    "Nothing to take back: everything in this file is already here.",
];

/// Version of the file's layout. A file with a higher number comes from a
/// newer Celenn OS and is refused (its meaning may have changed).
pub const FORMAT: u64 = 1;
const MADE_BY: &str = "Celenn OS settings";
/// What files saved before the project was renamed say
const MADE_BY_BEFORE_RENAME: &str = "cosmic-blue settings";
/// Bigger files and values are not settings files (the largest COSMIC
/// setting, a computed theme, is about 10 KB)
const MAX_FILE: u64 = 2 * 1024 * 1024;
const MAX_VALUE: usize = 64 * 1024;
const MAX_APPS: usize = 500;

/// What can be taken back, ticked separately
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Group {
    Look,
    Keyboard,
    Apps,
    DevTools,
}

/// The COSMIC settings that travel: (id, key, group); key "*": every key of
/// that id. Settings of the machine (screens, scale, the panels' screen)
/// are not in it.
const SETTINGS: &[(&str, &str, Group)] = &[
    ("com.system76.CosmicTheme.Mode", "*", Group::Look),
    ("com.system76.CosmicTheme.Dark", "*", Group::Look),
    ("com.system76.CosmicTheme.Dark.Builder", "*", Group::Look),
    ("com.system76.CosmicTheme.Light", "*", Group::Look),
    ("com.system76.CosmicTheme.Light.Builder", "*", Group::Look),
    ("com.system76.CosmicTk", "*", Group::Look),
    ("com.system76.CosmicBackground", "all", Group::Look),
    ("com.system76.CosmicBackground", "same-on-all", Group::Look),
    ("com.system76.CosmicPanel", "entries", Group::Look),
    ("com.system76.CosmicPanel.Panel", "*", Group::Look),
    ("com.system76.CosmicPanel.Dock", "*", Group::Look),
    ("com.system76.CosmicAppList", "favorites", Group::Look),
    ("com.system76.CosmicComp", "autotile", Group::Look),
    ("com.system76.CosmicComp", "autotile_behavior", Group::Look),
    ("com.system76.CosmicComp", "active_hint", Group::Look),
    ("com.system76.CosmicComp", "xkb_config", Group::Keyboard),
    (
        "com.system76.CosmicComp",
        "keyboard_config",
        Group::Keyboard,
    ),
    (
        "com.system76.CosmicSettings.Shortcuts",
        "custom",
        Group::Keyboard,
    ),
];
/// Keys of the ids above that belong to the machine: the screen a panel
/// is on
const MACHINE_KEYS: &[&str] = &["output"];
const CUSTOM_SHORTCUTS: (&str, &str) = ("com.system76.CosmicSettings.Shortcuts", "custom");
const BACKGROUND: &str = "com.system76.CosmicBackground";

fn group_of(id: &str, key: &str) -> Option<Group> {
    if !plain_name(key) || MACHINE_KEYS.contains(&key) {
        return None;
    }
    SETTINGS
        .iter()
        .find(|(i, k, _)| *i == id && (*k == "*" || *k == key))
        .map(|(_, _, group)| *group)
}

/// A setting's key or id: letters, digits, dots, dashes and underscores,
/// never a path
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// A Flatpak app id: at least three parts, as Flathub requires
pub fn valid_app_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split('.').collect();
    id.len() <= 255
        && parts.len() >= 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && !part.starts_with(|c: char| c.is_ascii_digit())
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        })
}

/// A name shown in the window: one short line of text
fn clean_name(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_control())
        .take(60)
        .collect::<String>()
        .trim()
        .to_owned()
}

fn output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default()
}

fn cosmic_dir() -> PathBuf {
    system::config_dir().join("cosmic")
}

// --- Saving ----------------------------------------------------------------

/// The person's own COSMIC settings that travel (values still at the
/// system default are not in ~/.config and need no copy)
fn own_settings(cosmic: &Path) -> Vec<(String, String, String)> {
    let mut found = Vec::new();
    let mut ids: Vec<&str> = SETTINGS.iter().map(|(id, _, _)| *id).collect();
    ids.dedup();
    for id in ids {
        let Ok(entries) = std::fs::read_dir(cosmic.join(id).join("v1")) else {
            continue;
        };
        for entry in entries.flatten() {
            let key = entry.file_name().to_string_lossy().into_owned();
            if group_of(id, &key).is_none() || !entry.file_type().is_ok_and(|t| t.is_file()) {
                continue;
            }
            if let Ok(value) = std::fs::read_to_string(entry.path())
                && value.len() <= MAX_VALUE
            {
                found.push((id.to_owned(), key, value));
            }
        }
    }
    found.sort();
    found
}

/// The Flathub apps installed on top of the first-boot ones: (id, name)
fn own_apps() -> Vec<(String, String)> {
    let preinstalled: BTreeSet<String> = system::preinstall_apps().into_iter().collect();
    let mut apps = Vec::new();
    for scope in ["--system", "--user"] {
        let list = output(
            "flatpak",
            &["list", scope, "--app", "--columns=application,origin,name"],
        );
        for line in list.lines() {
            let mut columns = line.split('\t');
            let (Some(id), Some(origin)) = (columns.next(), columns.next()) else {
                continue;
            };
            let name = columns.next().unwrap_or(id);
            if origin.trim() == "flathub"
                && valid_app_id(id.trim())
                && !preinstalled.contains(id.trim())
            {
                apps.push((id.trim().to_owned(), clean_name(name)));
            }
        }
    }
    apps.sort();
    apps.dedup_by(|a, b| a.0 == b.0);
    apps
}

fn computer_name() -> String {
    let pretty = output("hostnamectl", &["--pretty"]);
    let name = if pretty.trim().is_empty() {
        std::fs::read_to_string("/etc/hostname").unwrap_or_default()
    } else {
        pretty
    };
    clean_name(&name)
}

/// The file's content
fn file_content(
    settings: &[(String, String, String)],
    apps: &[(String, String)],
    dev_tools: &[&str],
) -> Value {
    let date = output("date", &["+%Y-%m-%d"]).trim().to_owned();
    let date_text = output("date", &["+%-d %B %Y"]).trim().to_owned();
    json!({
        "format": FORMAT,
        "made_by": MADE_BY,
        "date": date,
        "date_text": date_text,
        "computer": computer_name(),
        "settings": settings
            .iter()
            .map(|(id, key, value)| json!({"id": id, "key": key, "value": value}))
            .collect::<Vec<_>>(),
        "apps": apps
            .iter()
            .map(|(id, name)| json!({"id": id, "name": name}))
            .collect::<Vec<_>>(),
        "dev_tools": dev_tools,
    })
}

/// The Documents folder (XDG), else the home folder
fn documents() -> PathBuf {
    let dir = output("xdg-user-dir", &["DOCUMENTS"]);
    let dir = PathBuf::from(dir.trim());
    if dir.is_absolute() && dir.is_dir() {
        return dir;
    }
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from)
}

pub fn documents_folder() -> PathBuf {
    documents()
}

/// Saves the file in Documents; its path, or why it failed
pub fn save(dev_tools: Vec<&'static str>) -> Result<PathBuf, String> {
    let content = file_content(&own_settings(&cosmic_dir()), &own_apps(), &dev_tools);
    let text = serde_json::to_string_pretty(&content).map_err(|error| error.to_string())?;
    let folder = documents();
    let stem = format!(
        "{}-{}",
        tr("celenn-settings"),
        content["date"].as_str().unwrap_or_default()
    );
    let mut path = folder.join(format!("{stem}.json"));
    let mut n = 2;
    while path.exists() {
        path = folder.join(format!("{stem}-{n}.json"));
        n += 1;
    }
    std::fs::write(&path, text + "\n").map_err(|error| error.to_string())?;
    Ok(path)
}

// --- Taking back ------------------------------------------------------------

/// A file read and checked, before anything is written
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Restore {
    pub date: String,
    pub computer: String,
    /// (id, key, value, group)
    pub settings: Vec<(String, String, String, Group)>,
    /// Apps not installed here yet: (id, name)
    pub apps: Vec<(String, String)>,
    /// Developer tools of our list, by name
    pub dev_tools: Vec<&'static str>,
    /// Said whatever is ticked: settings unknown to this version, app ids
    /// not valid
    pub unknown_settings: usize,
    pub invalid_apps: usize,
}

impl Restore {
    pub fn groups(&self) -> Vec<Group> {
        let mut groups: Vec<Group> = self.settings.iter().map(|s| s.3).collect();
        if !self.apps.is_empty() {
            groups.push(Group::Apps);
        }
        if !self.dev_tools.is_empty() {
            groups.push(Group::DevTools);
        }
        groups.sort();
        groups.dedup();
        groups
    }
}

/// Why a file cannot be taken back (an English text, translated when shown)
pub fn read(path: &Path, installed_apps: &BTreeSet<String>) -> Result<Restore, &'static str> {
    let size = std::fs::metadata(path)
        .map_err(|_| "This file could not be read.")?
        .len();
    if size > MAX_FILE {
        return Err("This file is not a Celenn OS settings file.");
    }
    let text = std::fs::read_to_string(path).map_err(|_| "This file could not be read.")?;
    parse(&text, installed_apps)
}

pub fn parse(text: &str, installed_apps: &BTreeSet<String>) -> Result<Restore, &'static str> {
    let not_ours = "This file is not a Celenn OS settings file.";
    let file: Value = serde_json::from_str(text).map_err(|_| not_ours)?;
    if !matches!(
        file["made_by"].as_str(),
        Some(MADE_BY | MADE_BY_BEFORE_RENAME)
    ) {
        return Err(not_ours);
    }
    match file["format"].as_u64() {
        Some(format) if format > FORMAT => {
            return Err("This file was saved by a newer Celenn OS. Update this computer first.");
        }
        Some(_) => {}
        None => return Err(not_ours),
    }
    let text_of = |value: &Value| value.as_str().map(str::to_owned);
    let mut restore = Restore {
        date: file["date_text"]
            .as_str()
            .or(file["date"].as_str())
            .map(clean_name)
            .unwrap_or_default(),
        computer: file["computer"]
            .as_str()
            .map(clean_name)
            .unwrap_or_default(),
        ..Restore::default()
    };
    for setting in file["settings"].as_array().into_iter().flatten() {
        let (Some(id), Some(key), Some(value)) = (
            text_of(&setting["id"]),
            text_of(&setting["key"]),
            text_of(&setting["value"]),
        ) else {
            restore.unknown_settings += 1;
            continue;
        };
        match group_of(&id, &key) {
            Some(group) if plain_name(&id) && value.len() <= MAX_VALUE => {
                restore.settings.push((id, key, value, group));
            }
            _ => restore.unknown_settings += 1,
        }
    }
    for app in file["apps"].as_array().into_iter().flatten().take(MAX_APPS) {
        let Some(id) = app["id"].as_str() else {
            restore.invalid_apps += 1;
            continue;
        };
        if !valid_app_id(id) {
            restore.invalid_apps += 1;
        } else if !installed_apps.contains(id) && !restore.apps.iter().any(|(known, _)| known == id)
        {
            let name = app["name"].as_str().map(clean_name).unwrap_or_default();
            let name = if name.is_empty() { id.to_owned() } else { name };
            restore.apps.push((id.to_owned(), name));
        }
    }
    for name in file["dev_tools"].as_array().into_iter().flatten() {
        if let Some(tool) = devtools::dev_tools().find(|tool| Some(tool.name) == name.as_str())
            && !restore.dev_tools.contains(&tool.name)
        {
            restore.dev_tools.push(tool.name);
        }
    }
    Ok(restore)
}

/// What taking back did, to show
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// Where the previous settings were copied
    pub backup: Option<PathBuf>,
    pub written: usize,
    /// The groups with at least one setting written
    pub groups: BTreeSet<Group>,
    pub own_wallpaper: bool,
    pub shortcuts_with_programs: usize,
    pub failed: usize,
}

/// A custom shortcut list (RON map) without the shortcuts that start a
/// program (Spawn): the list kept, and how many were left out. None when
/// the text is not a map.
pub fn without_programs(ron: &str) -> Option<(String, usize)> {
    let text = ron.trim();
    let inner = text.strip_prefix('{')?.strip_suffix('}')?;
    // Top-level entries, split at commas outside brackets and strings
    let (mut entries, mut current) = (Vec::new(), String::new());
    let (mut depth, mut in_string, mut escaped) = (0i32, false, false);
    for c in inner.chars() {
        if in_string {
            current.push(c);
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                entries.push(std::mem::take(&mut current));
                continue;
            }
            _ => {}
        }
        if depth < 0 {
            return None;
        }
        current.push(c);
    }
    if in_string || depth != 0 {
        return None;
    }
    entries.push(current);
    let mut kept = Vec::new();
    let mut left_out = 0;
    for entry in entries.iter().map(|e| e.trim()).filter(|e| !e.is_empty()) {
        let value = value_of(entry)?;
        if value.trim_start().starts_with("Spawn") {
            left_out += 1;
        } else {
            kept.push(entry);
        }
    }
    let mut out = String::from("{\n");
    for entry in kept {
        out.push_str("    ");
        out.push_str(entry);
        out.push_str(",\n");
    }
    out.push('}');
    Some((out, left_out))
}

/// The value of a map entry: what follows the first ':' outside brackets
/// and strings
fn value_of(entry: &str) -> Option<&str> {
    let (mut depth, mut in_string, mut escaped) = (0i32, false, false);
    for (i, c) in entry.char_indices() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ':' if depth == 0 => return Some(&entry[i + 1..]),
            _ => {}
        }
    }
    None
}

/// A wallpaper setting that points outside the system's pictures (the
/// person's own picture, not in the file)
fn own_wallpaper(value: &str) -> bool {
    value
        .split("Path(\"")
        .skip(1)
        .any(|rest| !rest.starts_with("/usr/share/"))
}

/// Copies the COSMIC settings folder next to it, as `ujust
/// reset-cosmic-config` does
fn backup(cosmic: &Path) -> std::io::Result<Option<PathBuf>> {
    if !cosmic.is_dir() {
        return Ok(None);
    }
    let stamp = output("date", &["+%Y%m%d-%H%M%S"]);
    let target = cosmic.with_file_name(format!("cosmic.backup-{}", stamp.trim()));
    copy_dir(cosmic, &target)?;
    Ok(Some(target))
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Writes the settings of the groups ticked (the apps are installed
/// apart, with a progress bar)
pub fn apply_settings(restore: &Restore, groups: &BTreeSet<Group>) -> Report {
    apply_settings_in(&cosmic_dir(), restore, groups)
}

fn apply_settings_in(cosmic: &Path, restore: &Restore, groups: &BTreeSet<Group>) -> Report {
    let mut report = Report::default();
    let chosen: Vec<&(String, String, String, Group)> = restore
        .settings
        .iter()
        .filter(|setting| groups.contains(&setting.3))
        .collect();
    if chosen.is_empty() {
        return report;
    }
    match backup(cosmic) {
        Ok(path) => report.backup = path,
        Err(error) => {
            // Without a copy, nothing is changed
            eprintln!("Could not copy the COSMIC settings: {error}");
            report.failed = chosen.len();
            return report;
        }
    }
    for (id, key, value, group) in chosen {
        let mut value = value.clone();
        if id == BACKGROUND && own_wallpaper(&value) {
            report.own_wallpaper = true;
            continue;
        }
        if (id.as_str(), key.as_str()) == CUSTOM_SHORTCUTS {
            match without_programs(&value) {
                Some((kept, left_out)) => {
                    report.shortcuts_with_programs += left_out;
                    value = kept;
                }
                None => {
                    report.failed += 1;
                    continue;
                }
            }
        }
        if write(cosmic, id, key, &value) {
            report.written += 1;
            report.groups.insert(*group);
        } else {
            report.failed += 1;
        }
    }
    report
}

/// Writes a setting in one go (a temporary file renamed), as
/// system::cosmic_write, in the given folder
fn write(cosmic: &Path, id: &str, key: &str, value: &str) -> bool {
    let dir = cosmic.join(id).join("v1");
    let tmp = dir.join(format!(".{key}.tmp"));
    std::fs::create_dir_all(&dir)
        .and_then(|()| std::fs::write(&tmp, value))
        .and_then(|()| std::fs::rename(&tmp, dir.join(key)))
        .is_ok()
}

/// The lines of the "Not taken back" part of the report
pub fn left_out(restore: &Restore, report: &Report) -> Vec<String> {
    let mut lines = Vec::new();
    if report.own_wallpaper {
        lines.push(
            tr("Your own wallpaper is not in the file: copy the picture, then choose it in Settings.")
                .to_owned(),
        );
    }
    match report.shortcuts_with_programs {
        0 => {}
        1 => lines.push(
            tr("One shortcut starts a program: for safety, add it again in Settings.").to_owned(),
        ),
        n => lines.push(fill(
            tr("{} shortcuts start a program: for safety, add them again in Settings."),
            &n.to_string(),
        )),
    }
    let unknown = restore.unknown_settings + report.failed;
    if unknown > 0 {
        lines.push(fill(
            tr("{} settings unknown to this version of Celenn OS were left out."),
            &unknown.to_string(),
        ));
    }
    if restore.invalid_apps > 0 {
        lines.push(fill(
            tr("{} app names were not valid and were left out."),
            &restore.invalid_apps.to_string(),
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(settings: Value, apps: Value) -> String {
        json!({
            "format": 1,
            "made_by": MADE_BY,
            "date": "2026-11-25",
            "date_text": "25 November 2026",
            "computer": "Lea's laptop",
            "settings": settings,
            "apps": apps,
            "dev_tools": ["Docker", "rm -rf"],
        })
        .to_string()
    }

    #[test]
    fn names() {
        assert!(plain_name("is_dark"));
        assert!(plain_name("same-on-all"));
        assert!(!plain_name("../../.bashrc"));
        assert!(!plain_name(".hidden"));
        assert!(!plain_name("a/b"));
        assert!(valid_app_id("com.spotify.Client"));
        assert!(valid_app_id("org.gimp.GIMP"));
        assert!(!valid_app_id("spotify"));
        assert!(!valid_app_id("com.spotify.Client --user"));
        assert!(!valid_app_id("com..Client"));
        assert!(!valid_app_id("org.7zip.App"));
        assert_eq!(clean_name("Spotify\n\u{1b}[31m"), "Spotify[31m");
    }

    #[test]
    fn allowlist() {
        assert_eq!(
            group_of("com.system76.CosmicTheme.Mode", "is_dark"),
            Some(Group::Look)
        );
        assert_eq!(
            group_of("com.system76.CosmicComp", "xkb_config"),
            Some(Group::Keyboard)
        );
        // A screen of the other computer, a setting not in the list
        assert_eq!(group_of("com.system76.CosmicPanel.Panel", "output"), None);
        assert_eq!(group_of("com.system76.CosmicComp", "input_default"), None);
        assert_eq!(
            group_of("com.system76.CosmicSettings.Shortcuts", "system_actions"),
            None
        );
        assert_eq!(group_of("com.system76.CosmicTk", "../x"), None);
    }

    #[test]
    fn reading() {
        let installed: BTreeSet<String> = ["org.mozilla.firefox".to_owned()].into();
        let text = file(
            json!([
                {"id": "com.system76.CosmicTheme.Mode", "key": "is_dark", "value": "false"},
                {"id": "com.system76.CosmicComp", "key": "xkb_config", "value": "(layout: \"ch\")"},
                {"id": "com.system76.CosmicComp", "key": "input_default", "value": "()"},
                {"id": "../../..", "key": "bashrc", "value": "x"},
                {"id": "com.system76.CosmicTk", "key": "icon_theme"},
            ]),
            json!([
                {"id": "com.spotify.Client", "name": "Spotify"},
                {"id": "org.mozilla.firefox", "name": "Firefox"},
                {"id": "com.spotify.Client", "name": "Spotify"},
                {"id": "x; rm -rf ~", "name": "Bad"},
                {"id": "org.gimp.GIMP"},
            ]),
        );
        let restore = parse(&text, &installed).unwrap();
        assert_eq!(restore.computer, "Lea's laptop");
        assert_eq!(restore.date, "25 November 2026");
        assert_eq!(restore.settings.len(), 2);
        assert_eq!(restore.unknown_settings, 3);
        assert_eq!(
            restore.apps,
            vec![
                ("com.spotify.Client".to_owned(), "Spotify".to_owned()),
                ("org.gimp.GIMP".to_owned(), "org.gimp.GIMP".to_owned()),
            ]
        );
        assert_eq!(restore.invalid_apps, 1);
        assert_eq!(restore.dev_tools, vec!["Docker"]);
        assert_eq!(
            restore.groups(),
            vec![Group::Look, Group::Keyboard, Group::Apps, Group::DevTools]
        );
    }

    #[test]
    fn refused_files() {
        let none = BTreeSet::new();
        assert!(parse("not json", &none).is_err());
        assert!(parse("{\"format\": 1}", &none).is_err());
        let newer = json!({"format": FORMAT + 1, "made_by": MADE_BY}).to_string();
        assert_eq!(
            parse(&newer, &none),
            Err("This file was saved by a newer Celenn OS. Update this computer first.")
        );
    }

    #[test]
    fn shortcuts() {
        let custom = r#"{
    (modifiers: [Super], key: "t", description: Some("Terminal, ok: \"yes\"")): Spawn("cosmic-term"),
    (modifiers: [Super, Shift], key: "q"): Disable,
    (modifiers: [Ctrl, Alt], key: "Delete"): Spawn("sh -c 'rm -rf ~'"),
    (modifiers: [Super], key: "Left"): Focus(Left),
}"#;
        let (kept, left_out) = without_programs(custom).unwrap();
        assert_eq!(left_out, 2);
        assert!(kept.contains("Disable"));
        assert!(kept.contains("Focus(Left)"));
        assert!(!kept.contains("Spawn"));
        assert_eq!(without_programs("{}"), Some(("{\n}".to_owned(), 0)));
        assert_eq!(without_programs("Spawn(\"x\")"), None);
        assert_eq!(without_programs("{ (key: \"a\": Spawn(\"x\") }"), None);
    }

    #[test]
    fn wallpapers() {
        assert!(!own_wallpaper(
            "(output: \"all\", source: Path(\"/usr/share/backgrounds/celenn/galaxy.jxl\"))"
        ));
        assert!(own_wallpaper(
            "(output: \"all\", source: Path(\"/home/lea/Pictures/cat.jpg\"))"
        ));
        assert!(!own_wallpaper(
            "(output: \"all\", source: Color(Single((0.0, 0.0, 0.0))))"
        ));
    }

    #[test]
    fn round_trip() {
        let dir = std::env::temp_dir().join(format!("celenn-transfer-{}", std::process::id()));
        let from = dir.join("from/cosmic");
        let to = dir.join("to/cosmic");
        for (id, key, value) in [
            ("com.system76.CosmicTheme.Mode", "is_dark", "false"),
            (
                "com.system76.CosmicPanel.Panel",
                "output",
                "Name(\"eDP-1\")",
            ),
            ("com.system76.CosmicPanel.Panel", "size", "M"),
            (
                "com.system76.CosmicBackground",
                "all",
                "(source: Path(\"/home/lea/cat.jpg\"))",
            ),
            (
                "com.system76.CosmicSettings.Shortcuts",
                "custom",
                "{ (modifiers: [Super], key: \"e\"): Spawn(\"x\"), (modifiers: [Super], key: \"q\"): Close, }",
            ),
            ("com.system76.CosmicFiles", "show_hidden", "true"),
        ] {
            assert!(write(&from, id, key, value));
        }
        let settings = own_settings(&from);
        // Not the panel's screen, not the files app
        assert_eq!(settings.len(), 4);
        let text = file_content(&settings, &[], &[]).to_string();
        let restore = parse(&text, &BTreeSet::new()).unwrap();
        assert!(write(
            &to,
            "com.system76.CosmicTheme.Mode",
            "is_dark",
            "true"
        ));
        let groups: BTreeSet<Group> = [Group::Look, Group::Keyboard].into();
        let report = apply_settings_in(&to, &restore, &groups);
        assert_eq!(report.written, 3);
        assert_eq!(report.groups, groups);
        assert!(report.own_wallpaper);
        assert_eq!(report.shortcuts_with_programs, 1);
        let read = |dir: &Path, id: &str, key: &str| {
            std::fs::read_to_string(dir.join(id).join("v1").join(key)).ok()
        };
        assert_eq!(
            read(&to, "com.system76.CosmicTheme.Mode", "is_dark").as_deref(),
            Some("false")
        );
        assert_eq!(
            read(&to, "com.system76.CosmicPanel.Panel", "size").as_deref(),
            Some("M")
        );
        assert_eq!(read(&to, "com.system76.CosmicPanel.Panel", "output"), None);
        assert_eq!(read(&to, "com.system76.CosmicBackground", "all"), None);
        let custom = read(&to, "com.system76.CosmicSettings.Shortcuts", "custom").unwrap();
        assert!(custom.contains("Close") && !custom.contains("Spawn"));
        // The previous value is in the copy
        let backup = report.backup.clone().unwrap();
        assert_eq!(
            read(&backup, "com.system76.CosmicTheme.Mode", "is_dark").as_deref(),
            Some("true")
        );
        let lines = left_out(&restore, &report);
        assert_eq!(lines.len(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }
}
