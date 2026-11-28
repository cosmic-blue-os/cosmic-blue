// Housekeeping, like Windows' Storage Sense and the Mac's "Optimize
// storage": what Celenn OS Tools does on its own, a few times a day, in
// each person's session (celenn-housekeeping.timer, a systemd user
// timer, runs `celenn-tools --housekeeping`, with no window).
//
//   - Files that stayed in the trash for more than 30 days are deleted. The
//     trash of the home folder only, not those of USB drives.
//   - When the disk is almost full, one notification says so, with a button
//     that opens the Free up space tool. Again a week later if it still is,
//     or at once when it gets critical. Never in a child's session (Parental
//     controls): the parent sees it in theirs.
//
//   - Web apps made by an older version are brought up to date
//     (webapps::update_older), once.
//
// Both are on by default; the Free up space tool turns them off. Off is a
// file in ~/.config/celenn/ (keep-trash, no-disk-warning), so nothing
// is written while they stay on.

use crate::common::{fill, spawn};
use crate::i18n::tr;
use crate::system::xdg;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Days a file stays in the trash before it is deleted
pub const TRASH_DAYS: u64 = 30;
/// Under this, the disk is critical: warned at once, even after a warning
const CRITICAL: u64 = 1_000_000_000;
/// The disk is almost full under 10 % free, at most 20 GB (a 1 TB disk is
/// not almost full with 90 GB left)
const LOW_SHARE: u64 = 10;
const LOW_MAX: u64 = 20_000_000_000;
/// A warning comes back after a week when nothing changed
const AGAIN: Duration = Duration::from_secs(7 * 24 * 3600);
const PARENTAL_RULES: &str = "/etc/celenn/parental-controls";
const ICON: &str = "celenn-tools";

pub const TEXTS: &[&str] = &[
    "The disk is almost full",
    "{} left. Updates and new apps need room.",
    "Free up space",
];

fn config(name: &str) -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("celenn").join(name)
}

fn warned_file() -> PathBuf {
    xdg("XDG_STATE_HOME", ".local/state")
        .join("celenn")
        .join("disk-warned")
}

fn trash() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share").join("Trash")
}

/// A setting that is on unless its "off" file exists
fn setting(off: &str) -> bool {
    !config(off).exists()
}

fn set_setting(off: &str, on: bool) {
    let path = config(off);
    let result = if on {
        match fs::remove_file(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    } else {
        path.parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|()| fs::write(&path, ""))
    };
    if let Err(error) = result {
        eprintln!("{}: {error}", path.display());
    }
}

pub fn trash_auto() -> bool {
    setting("keep-trash")
}

pub fn set_trash_auto(on: bool) {
    set_setting("keep-trash", on);
}

pub fn disk_warning() -> bool {
    setting("no-disk-warning")
}

pub fn set_disk_warning(on: bool) {
    set_setting("no-disk-warning", on);
}

/// Size of a file, or of a folder and everything in it (links not followed)
fn size_of(path: &Path) -> u64 {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return 0;
    };
    if !metadata.is_dir() {
        return metadata.len();
    }
    fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| size_of(&entry.path()))
        .sum()
}

/// What the trash holds, in bytes
pub fn trash_size() -> u64 {
    size_of(&trash().join("files"))
}

/// Deletes what went to the trash more than `days` days ago: each file has
/// its Trash/info/<name>.trashinfo, written when it was trashed (freedesktop
/// trash). Returns how many were deleted.
pub fn purge_trash(days: u64) -> usize {
    let trash = trash();
    let limit = Duration::from_secs(days * 24 * 3600);
    let now = SystemTime::now();
    let mut deleted = 0;
    for entry in fs::read_dir(trash.join("info"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let info = entry.path();
        if info.extension().and_then(|e| e.to_str()) != Some("trashinfo") {
            continue;
        }
        let old = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|trashed| now.duration_since(trashed).ok())
            .is_some_and(|age| age > limit);
        if !old {
            continue;
        }
        let Some(name) = info.file_stem() else {
            continue;
        };
        let file = trash.join("files").join(name);
        let removed = match fs::symlink_metadata(&file) {
            Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(&file),
            Ok(_) => fs::remove_file(&file),
            // Already gone: only its info is left
            Err(_) => Ok(()),
        };
        match removed.and_then(|()| fs::remove_file(&info)) {
            Ok(()) => deleted += 1,
            Err(error) => eprintln!("{}: {error}", file.display()),
        }
    }
    deleted
}

/// How bad the disk is: 0 fine, 1 almost full, 2 critical
pub fn level(total: u64, free: u64) -> u8 {
    if free < CRITICAL {
        2
    } else if free < (total / LOW_SHARE).min(LOW_MAX) {
        1
    } else {
        0
    }
}

/// Whether to warn now, given the last warning (its level and when)
fn should_warn(level: u8, last: Option<(u8, SystemTime)>, now: SystemTime) -> bool {
    match last {
        _ if level == 0 => false,
        None => true,
        Some((last_level, when)) => {
            level > last_level
                || now
                    .duration_since(when)
                    .is_ok_and(|elapsed| elapsed >= AGAIN)
        }
    }
}

fn last_warning() -> Option<(u8, SystemTime)> {
    let path = warned_file();
    let level = fs::read_to_string(&path).ok()?.trim().parse().ok()?;
    let when = fs::metadata(&path).ok()?.modified().ok()?;
    Some((level, when))
}

/// A child of Parental controls: the parent gets the warnings
fn is_child() -> bool {
    std::env::var("USER").is_ok_and(|user| {
        Path::new(PARENTAL_RULES)
            .join(format!("{user}.json"))
            .exists()
    })
}

/// One notification, with a button that opens the Free up space tool. Waits
/// for it to be clicked or closed.
fn notify(free: u64) {
    let action = format!("open={}", tr("Free up space"));
    let output = std::process::Command::new("notify-send")
        .args([
            "--app-name",
            tr("Celenn OS Tools"),
            "--icon",
            ICON,
            "--action",
            &action,
            "--wait",
            tr("The disk is almost full"),
            &fill(
                tr("{} left. Updates and new apps need room."),
                &crate::units::size(free),
            ),
        ])
        .stdin(std::process::Stdio::null())
        .output();
    match output {
        Ok(output) if String::from_utf8_lossy(&output.stdout).trim() == "open" => {
            spawn("/usr/libexec/celenn-tools", &["--tool", "space"]);
        }
        Ok(_) => {}
        Err(error) => eprintln!("notify-send: {error}"),
    }
}

/// `celenn-tools --housekeeping`: the timer's run
pub fn run() -> i32 {
    crate::webapps::update_older();
    if trash_auto() {
        let deleted = purge_trash(TRASH_DAYS);
        if deleted > 0 {
            println!("Deleted {deleted} files trashed more than {TRASH_DAYS} days ago");
        }
    }
    let Some(disk) = crate::maintenance::disk() else {
        return 0;
    };
    let level = level(disk.total, disk.free);
    let path = warned_file();
    if level == 0 {
        // Room again: the next time it fills up is a new warning
        let _ = fs::remove_file(&path);
        return 0;
    }
    if !disk_warning() || is_child() || !should_warn(level, last_warning(), SystemTime::now()) {
        return 0;
    }
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Err(error) = fs::write(&path, level.to_string()) {
        eprintln!("{}: {error}", path.display());
    }
    notify(disk.free);
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1_000_000_000;

    #[test]
    fn levels() {
        // 128 GB disk: almost full under 12.8 GB
        assert_eq!(level(128 * GB, 20 * GB), 0);
        assert_eq!(level(128 * GB, 12 * GB), 1);
        assert_eq!(level(128 * GB, GB / 2), 2);
        // 2 TB disk: almost full under 20 GB, not 200 GB
        assert_eq!(level(2000 * GB, 100 * GB), 0);
        assert_eq!(level(2000 * GB, 19 * GB), 1);
    }

    #[test]
    fn warnings() {
        let now = SystemTime::now();
        let day = Duration::from_secs(24 * 3600);
        assert!(!should_warn(0, None, now));
        assert!(should_warn(1, None, now));
        assert!(!should_warn(1, Some((1, now - day)), now));
        assert!(should_warn(1, Some((1, now - 8 * day)), now));
        // Critical after almost full: at once
        assert!(should_warn(2, Some((1, now - day)), now));
        assert!(!should_warn(1, Some((2, now - day)), now));
    }
}
