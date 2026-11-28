// The desktop's layout, shared by the tour (main.rs) and Celenn OS Tools
// (more.rs, with the dock's setting), like the layout page of COSMIC's own
// first-run assistant:
//
//   - Top bar and dock: Celenn OS's default (the system defaults in
//     /usr/share/cosmic), close to the Mac.
//   - One bar at the bottom: the bar holds the app library and launcher on
//     the left, the open windows, then the status icons and the clock on the
//     right; no dock. Close to Windows.
//
// Only the panel's place, its applets and whether the dock exists are
// written to the person's settings (~/.config/cosmic); going back to the
// default removes them, so the rest of their panel settings stay.

use std::path::PathBuf;

const PANELS: &str = "com.system76.CosmicPanel";
const PANEL: &str = "com.system76.CosmicPanel.Panel";
const DOCK: &str = "com.system76.CosmicPanel.Dock";

/// The bottom bar's applets: those of COSMIC's own "Bottom panel" layout
/// (cosmic-initial-setup res/layouts/bottom-panel)
const BOTTOM_WINGS: &str = r#"Some(([
    "com.system76.CosmicPanelLauncherButton",
    "com.system76.CosmicPanelWorkspacesButton",
    "com.system76.CosmicPanelAppButton",
    "com.system76.CosmicAppList",
    "com.system76.CosmicAppletMinimize",
], [
    "com.system76.CosmicAppletInputSources",
    "com.system76.CosmicAppletStatusArea",
    "com.system76.CosmicAppletA11y",
    "com.system76.CosmicAppletTiling",
    "com.system76.CosmicAppletAudio",
    "com.system76.CosmicAppletBluetooth",
    "com.system76.CosmicAppletNetwork",
    "com.system76.CosmicAppletBattery",
    "com.system76.CosmicAppletNotifications",
    "com.system76.CosmicAppletTime",
    "com.system76.CosmicAppletPower",
]))"#;

/// The settings the bottom bar changes: (config, key, value)
const BOTTOM: &[(&str, &str, &str)] = &[
    (PANELS, "entries", "[\n    \"Panel\",\n]"),
    (PANEL, "anchor", "Bottom"),
    (PANEL, "plugins_center", "Some([])"),
    (PANEL, "plugins_wings", BOTTOM_WINGS),
];

fn setting(id: &str, key: &str) -> PathBuf {
    crate::system::config_dir()
        .join("cosmic")
        .join(id)
        .join("v1")
        .join(key)
}

/// Dock always visible, or hidden when a window covers it (Celenn OS's
/// default, as COSMIC Settings' "Automatically hide dock"). Tools only.
#[allow(dead_code)]
pub fn dock_always_visible() -> bool {
    std::fs::read_to_string(setting(DOCK, "autohide")).is_ok_and(|value| value.trim() == "Never")
}

#[allow(dead_code)]
pub fn set_dock_always_visible(visible: bool) -> bool {
    let path = setting(DOCK, "autohide");
    let tmp = path.with_file_name(".autohide.tmp");
    let result = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&tmp, if visible { "Never" } else { "OnOverlap" }))
        .and_then(|()| std::fs::rename(&tmp, &path));
    if let Err(error) = &result {
        eprintln!("{}: {error}", path.display());
    }
    result.is_ok()
}

/// Whether the desktop has one bar at the bottom (no dock)
pub fn bottom_bar() -> bool {
    let read = |id, key| {
        std::fs::read_to_string(setting(id, key))
            .map(|value| value.trim().to_owned())
            .unwrap_or_default()
    };
    read(PANEL, "anchor") == "Bottom" && !read(PANELS, "entries").contains("Dock")
}

/// Switches the layout. The panel follows its settings as they change; it
/// is restarted too, as COSMIC's assistant does, for the applets that only
/// load when it starts.
pub fn set_bottom_bar(bottom: bool) -> bool {
    let mut ok = true;
    for (id, key, value) in BOTTOM {
        let path = setting(id, key);
        let result = if bottom {
            let tmp = path.with_file_name(format!(".{key}.tmp"));
            path.parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&tmp, value))
                .and_then(|()| std::fs::rename(&tmp, &path))
        } else {
            match std::fs::remove_file(&path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                other => other,
            }
        };
        if let Err(error) = result {
            eprintln!("{}: {error}", path.display());
            ok = false;
        }
    }
    let _ = std::process::Command::new("pkill")
        .args(["-x", "cosmic-panel"])
        .status();
    ok
}
