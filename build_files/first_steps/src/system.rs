// What the first steps tour reads from the system and changes in it, for the
// person who runs it: blocking functions, called off the window's thread
// (tokio::task::spawn_blocking). Nothing here needs root, except the switch
// to the NVIDIA image, which goes through the Updates app's helper (pkexec).
//
// COSMIC settings are files, one per key, in ~/.config/cosmic/<id>/v1/: the
// desktop watches them and applies a change at once (the same files that
// COSMIC Settings and `ujust toggle-dark-mode` write).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Apps installed at the first boot (flatpak-preinstall.service)
const PREINSTALL_DIR: &str = "/usr/share/flatpak/preinstall.d";
pub const WALLPAPERS: &str = "/usr/share/backgrounds/cosmic-blue";
/// Small copies of the wallpapers, for the tour (branding/tour/thumbnails.py)
pub const THUMBNAILS: &str = "/usr/share/cosmic-blue/wallpapers";
pub const AVATARS: &str = "/usr/share/pixmaps/faces";
const IMAGE_INFO: &str = "/usr/share/ublue-os/image-info.json";
/// Homebrew, set up at the first boot (ublue-os/brew)
pub const UPDATES_HELPER: &str = "/usr/libexec/cosmic-blue-updates-helper";

fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"))
}

/// A COSMIC setting: the person's value, else the system default
pub fn cosmic_read(id: &str, key: &str) -> Option<String> {
    let user = config_dir().join("cosmic").join(id).join("v1").join(key);
    let system = Path::new("/usr/share/cosmic").join(id).join("v1").join(key);
    std::fs::read_to_string(user)
        .or_else(|_| std::fs::read_to_string(system))
        .ok()
        .map(|value| value.trim().to_owned())
}

/// Writes a COSMIC setting in one go (a temporary file renamed), so the
/// desktop never reads half of it
pub fn cosmic_write(id: &str, key: &str, value: &str) -> bool {
    let dir = config_dir().join("cosmic").join(id).join("v1");
    let tmp = dir.join(format!(".{key}.tmp"));
    let result = std::fs::create_dir_all(&dir)
        .and_then(|()| std::fs::write(&tmp, value))
        .and_then(|()| std::fs::rename(&tmp, dir.join(key)));
    if let Err(error) = &result {
        eprintln!("Could not write the COSMIC setting {id}/{key}: {error}");
    }
    result.is_ok()
}

/// The tour's own memory (~/.config/cosmic-blue/<name>)
pub fn tour_read(name: &str) -> Option<String> {
    std::fs::read_to_string(config_dir().join("cosmic-blue").join(name))
        .ok()
        .map(|value| value.trim().to_owned())
}

pub fn tour_write(name: &str, value: &str) {
    let dir = config_dir().join("cosmic-blue");
    if let Err(error) =
        std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(dir.join(name), value))
    {
        eprintln!("Could not save {name}: {error}");
    }
}

// --- Look ----------------------------------------------------------------

pub fn is_dark() -> bool {
    cosmic_read("com.system76.CosmicTheme.Mode", "is_dark").as_deref() != Some("false")
}

/// Dark or light theme. Automatic switching at sunrise and sunset would undo
/// the choice: it is turned off, like `ujust toggle-dark-mode` does.
pub fn set_dark(dark: bool) -> bool {
    let id = "com.system76.CosmicTheme.Mode";
    if cosmic_read(id, "auto_switch").as_deref() == Some("true") {
        cosmic_write(id, "auto_switch", "false");
    }
    cosmic_write(id, "is_dark", if dark { "true" } else { "false" })
}

/// The wallpapers of cosmic-blue that have a small copy, sorted by name:
/// (wallpaper, its small copy)
pub fn wallpapers() -> Vec<(PathBuf, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(THUMBNAILS) else {
        return Vec::new();
    };
    let mut found: Vec<(PathBuf, PathBuf)> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter_map(|thumb| {
            let wallpaper = Path::new(WALLPAPERS).join(thumb.file_name()?);
            wallpaper.is_file().then_some((wallpaper, thumb))
        })
        .collect();
    found.sort();
    found
}

/// The wallpaper shown now: its file, from the background setting
pub fn current_wallpaper() -> Option<PathBuf> {
    let all = cosmic_read("com.system76.CosmicBackground", "all")?;
    let start = all.find("Path(\"")? + "Path(\"".len();
    let end = start + all[start..].find('"')?;
    Some(PathBuf::from(&all[start..end]))
}

/// The same wallpaper on every screen, zoomed to fill it (cosmic-blue's
/// default setting, /usr/share/cosmic/com.system76.CosmicBackground)
pub fn set_wallpaper(path: &Path) -> bool {
    let value = format!(
        "(\n    output: \"all\",\n    source: Path({:?}),\n    filter_by_theme: true,\n    rotation_frequency: 300,\n    filter_method: Lanczos,\n    scaling_mode: Zoom,\n    sampling_method: Alphanumeric,\n)",
        path.to_string_lossy()
    );
    let id = "com.system76.CosmicBackground";
    cosmic_write(id, "same-on-all", "true") && cosmic_write(id, "all", &value)
}

/// cosmic-blue's avatars (512 x 512 PNG), sorted by name. Links are left out:
/// pop-robot.png, COSMIC Settings' default picture, is a link to planet.png
/// and would show the planet twice.
pub fn avatars() -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(AVATARS) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "png"))
        .collect();
    found.sort();
    found
}

fn uid() -> String {
    // The user's own files are theirs: the owner of the home folder
    std::env::var_os("HOME")
        .and_then(|home| std::fs::metadata(home).ok())
        .map(|meta| std::os::unix::fs::MetadataExt::uid(&meta).to_string())
        .unwrap_or_default()
}

fn busctl(args: &[&str]) -> Option<String> {
    let output = Command::new("busctl")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// The account's object in AccountsService
fn account() -> Option<String> {
    let reply = busctl(&[
        "call",
        "org.freedesktop.Accounts",
        "/org/freedesktop/Accounts",
        "org.freedesktop.Accounts",
        "FindUserById",
        "x",
        &uid(),
    ])?;
    // o "/org/freedesktop/Accounts/User1000"
    Some(reply.split('"').nth(1)?.to_owned())
}

/// The profile picture (login screen, panel): AccountsService keeps a copy
/// of the file. Anyone can change their own (polkit allows it).
pub fn set_avatar(path: &Path) -> bool {
    let Some(account) = account() else {
        return false;
    };
    busctl(&[
        "call",
        "org.freedesktop.Accounts",
        &account,
        "org.freedesktop.Accounts.User",
        "SetIconFile",
        "s",
        &path.to_string_lossy(),
    ])
    .is_some()
}

/// Dock always visible, or hidden when a window covers it (cosmic-blue's
/// default, as COSMIC Settings' "Automatically hide dock")
pub fn dock_always_visible() -> bool {
    cosmic_read("com.system76.CosmicPanel.Dock", "autohide").as_deref() == Some("Never")
}

pub fn set_dock_always_visible(visible: bool) -> bool {
    cosmic_write(
        "com.system76.CosmicPanel.Dock",
        "autohide",
        if visible { "Never" } else { "OnOverlap" },
    )
}

/// Windows side by side (tiling) on every workspace, as the panel's tiling
/// button and Super + Y do
pub fn tiling() -> bool {
    cosmic_read("com.system76.CosmicComp", "autotile").as_deref() == Some("true")
}

pub fn set_tiling(on: bool) -> bool {
    cosmic_write(
        "com.system76.CosmicComp",
        "autotile",
        if on { "true" } else { "false" },
    )
}

// --- Apps ----------------------------------------------------------------

/// The flatpak apps installed for everyone (where the first-boot apps and
/// COSMIC Store put them)
pub fn installed_apps() -> BTreeSet<String> {
    Command::new("flatpak")
        .args(["list", "--system", "--app", "--columns=application"])
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

/// The apps installed at the first boot ([Flatpak Preinstall <id>] sections)
pub fn preinstall_apps() -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(PREINSTALL_DIR) else {
        return Vec::new();
    };
    let mut apps = Vec::new();
    for entry in entries.flatten() {
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        for line in text.lines() {
            if let Some(id) = line
                .trim()
                .strip_prefix("[Flatpak Preinstall ")
                .and_then(|rest| rest.strip_suffix(']'))
            {
                // Runtimes (themes) are not apps the person waits for
                if !id.starts_with("org.gtk.Gtk3theme.") {
                    apps.push(id.to_owned());
                }
            }
        }
    }
    apps.sort();
    apps.dedup();
    apps
}

/// The first-boot installation is still running
pub fn preinstall_running() -> bool {
    Command::new("systemctl")
        .args(["is-active", "flatpak-preinstall.service"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok_and(|output| {
            matches!(
                String::from_utf8_lossy(&output.stdout).trim(),
                "activating" | "reloading"
            )
        })
}

// --- The computer ----------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct Computer {
    /// Online (NetworkManager's connectivity check): None when unknown
    pub online: Option<bool>,
    /// An NVIDIA graphics card, and whether this image has its driver
    pub nvidia: Option<bool>,
    /// Secure Boot is on (the NVIDIA image's key must then be enrolled)
    pub secure_boot: bool,
    /// Firmware updates waiting (fwupd): device names. None when unknown
    pub firmware: Option<Vec<String>>,
    /// A laptop battery whose charge can be capped: whether the cap is on
    pub battery_limit: Option<bool>,
    /// The laptop battery's object in UPower
    pub battery: Option<String>,
}

pub fn online() -> Option<bool> {
    let output = Command::new("nmcli")
        .args(["networking", "connectivity"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    match String::from_utf8_lossy(&output.stdout).trim() {
        "full" => Some(true),
        "none" | "limited" | "portal" => Some(false),
        _ => None,
    }
}

/// Some(has the driver) when the computer has an NVIDIA graphics card
fn nvidia() -> Option<bool> {
    let entries = std::fs::read_dir("/sys/bus/pci/devices").ok()?;
    let read = |dir: &Path, name: &str| {
        std::fs::read_to_string(dir.join(name))
            .map(|value| value.trim().to_owned())
            .unwrap_or_default()
    };
    let found = entries.flatten().any(|entry| {
        let dir = entry.path();
        // Class 0x03xxxx: display controller (VGA, 3D)
        read(&dir, "vendor") == "0x10de" && read(&dir, "class").starts_with("0x03")
    });
    found.then(|| image_name().ends_with("-nvidia"))
}

/// "cosmic-blue" or "cosmic-blue-nvidia"
pub fn image_name() -> String {
    std::fs::read_to_string(IMAGE_INFO)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|info| info.get("image-name")?.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn secure_boot() -> bool {
    Command::new("mokutil")
        .arg("--sb-state")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok_and(|output| String::from_utf8_lossy(&output.stdout).contains("SecureBoot enabled"))
}

/// Firmware updates from LVFS (fwupd), from the metadata the system already
/// has (fwupd-refresh.timer keeps it fresh): no download, no question
fn firmware() -> Option<Vec<String>> {
    let mut child = Command::new("fwupdmgr")
        .args([
            "get-updates",
            "--json",
            "--no-unreported-check",
            "--no-metadata-check",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    // fwupd can be slow to answer: give up after 20 seconds
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let mut out = String::new();
    std::io::Read::read_to_string(&mut child.stdout.take()?, &mut out).ok()?;
    let json: serde_json::Value = serde_json::from_str(&out).ok()?;
    let devices = json.get("Devices")?.as_array()?;
    Some(
        devices
            .iter()
            .filter(|device| {
                device
                    .get("Releases")
                    .and_then(|releases| releases.as_array())
                    .is_some_and(|releases| !releases.is_empty())
            })
            .filter_map(|device| device.get("Name")?.as_str().map(str::to_owned))
            .collect(),
    )
}

fn upower_property(device: &str, name: &str) -> Option<String> {
    let reply = busctl(&[
        "get-property",
        "org.freedesktop.UPower",
        device,
        "org.freedesktop.UPower.Device",
        name,
    ])?;
    // "b true"
    reply.split_whitespace().nth(1).map(str::to_owned)
}

/// The laptop's own battery (not a mouse's), when its charge can be capped:
/// (its object, whether the cap is on). Same rules as
/// `ujust toggle-battery-limit`.
fn battery() -> Option<(String, bool)> {
    let output = Command::new("upower")
        .arg("--enumerate")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let list = String::from_utf8_lossy(&output.stdout);
    let device = list
        .lines()
        .map(str::trim)
        .filter(|path| path.contains("/battery_"))
        .find(|path| upower_property(path, "PowerSupply").as_deref() == Some("true"))?
        .to_owned();
    if upower_property(&device, "ChargeThresholdSupported").as_deref() != Some("true") {
        return None;
    }
    let enabled = upower_property(&device, "ChargeThresholdEnabled").as_deref() == Some("true");
    Some((device, enabled))
}

/// Charging stops at 80 % (or the laptop maker's value) when on
pub fn set_battery_limit(device: &str, on: bool) -> bool {
    busctl(&[
        "call",
        "org.freedesktop.UPower",
        device,
        "org.freedesktop.UPower.Device",
        "EnableChargeThreshold",
        "b",
        if on { "true" } else { "false" },
    ])
    .is_some()
}

pub fn computer() -> Computer {
    let (battery, battery_limit) = match battery() {
        Some((device, on)) => (Some(device), Some(on)),
        None => (None, None),
    };
    let nvidia = nvidia();
    Computer {
        online: online(),
        secure_boot: nvidia.is_some() && secure_boot(),
        nvidia,
        firmware: firmware(),
        battery,
        battery_limit,
    }
}

// --- Files that do not install here ------------------------------------------

/// The kind of a file that cannot be installed on cosmic-blue
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Package {
    Windows,
    Debian,
    Rpm,
}

pub fn package_kind(path: &Path) -> Package {
    match path
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .as_deref()
    {
        Some("deb") => Package::Debian,
        Some("rpm") => Package::Rpm,
        _ => Package::Windows,
    }
}

/// What to search for in COSMIC Store from a file name:
/// "SpotifySetup.exe" -> "spotify",
/// "google-chrome-stable_current_amd64.deb" -> "google chrome"
pub fn search_term(path: &Path) -> String {
    const NOISE: &[&str] = &[
        "setup",
        "installer",
        "install",
        "full",
        "x64",
        "x86",
        "win",
        "win32",
        "win64",
        "windows",
        "amd64",
        "arm64",
        "aarch64",
        "i386",
        "i686",
        "x86_64",
        "noarch",
        "linux",
        "latest",
        "stable",
        "current",
        "release",
        "portable",
        "offline",
        "online",
        "web",
        "user",
        "system",
        "deb",
        "rpm",
        "exe",
        "msi",
        "ubuntu",
        "debian",
        "fedora",
        "el8",
        "el9",
        "el10",
        "bin",
        "pkg",
        "package",
    ];
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    // "SpotifySetup" -> "Spotify Setup"
    let mut spaced = String::new();
    let mut previous_lower = false;
    for c in stem.chars() {
        if c.is_uppercase() && previous_lower {
            spaced.push(' ');
        }
        previous_lower = c.is_lowercase();
        spaced.push(c);
    }
    let words: Vec<String> = spaced
        .split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|word| !word.is_empty())
        // Versions, build numbers and the like
        .filter(|word| !word.chars().any(|c| c.is_ascii_digit()))
        .filter(|word| !NOISE.contains(&word.as_str()))
        .take(2)
        .collect();
    words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_terms() {
        for (file, term) in [
            ("SpotifySetup.exe", "spotify"),
            ("setup-spotify.exe", "spotify"),
            ("google-chrome-stable_current_amd64.deb", "google chrome"),
            ("code_1.95.3-1731513102_amd64.deb", "code"),
            ("ZoomInstallerFull.exe", "zoom"),
            ("discord-0.0.75.deb", "discord"),
            ("vlc-3.0.21-win64.exe", "vlc"),
            ("steam_latest.deb", "steam"),
            ("teamviewer.x86_64.rpm", "teamviewer"),
        ] {
            assert_eq!(search_term(Path::new(file)), term, "{file}");
        }
    }
}
