// Developer tools that cosmic-blue offers to install without a terminal:
// the same list in the first steps tour (main.rs) and in cosmic-blue Tools'
// Extra software page (more.rs), installed the same way.

use std::collections::BTreeSet;
use std::path::Path;

pub const BREW: &str = "/home/linuxbrew/.linuxbrew/bin/brew";
const BREW_PREFIX: &str = "/home/linuxbrew/.linuxbrew";

fn brew_has_formula(name: &str) -> bool {
    Path::new(BREW_PREFIX).join("Cellar").join(name).is_dir()
}

fn brew_has_cask(name: &str) -> bool {
    Path::new(BREW_PREFIX).join("Caskroom").join(name).is_dir()
}

/// Developer tools, as in `ujust install-dev-tools` (Bluefin's menu), but
/// picked in the tour: Homebrew formulas and casks for the user, Podman
/// Desktop from Flathub. Lima stays with ujust (its setup needs a restart).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    /// Homebrew formulas (the first one tells if it is installed)
    Formula(&'static [&'static str]),
    /// A Homebrew cask, its name in the ublue-os tap or Homebrew's
    Cask(&'static str, bool),
    Flatpak(&'static str),
}

pub struct DevTool {
    pub name: &'static str,
    pub kind: ToolKind,
}

/// Groups of developer tools: a heading, then its tools
pub const DEV_TOOLS: &[(&str, &[DevTool])] = &[
    (
        "Containers",
        &[
            DevTool {
                name: "Docker",
                kind: ToolKind::Formula(&["docker", "docker-compose", "lazydocker", "dive"]),
            },
            DevTool {
                name: "Podman Desktop",
                kind: ToolKind::Flatpak("io.podman_desktop.PodmanDesktop"),
            },
        ],
    ),
    (
        "Code editors",
        &[
            DevTool {
                name: "VS Code",
                kind: ToolKind::Cask("visual-studio-code-linux", true),
            },
            DevTool {
                name: "VSCodium",
                kind: ToolKind::Cask("vscodium-linux", true),
            },
            DevTool {
                name: "Zed",
                kind: ToolKind::Cask("zed", false),
            },
            DevTool {
                name: "JetBrains Toolbox",
                kind: ToolKind::Cask("jetbrains-toolbox-linux", true),
            },
            DevTool {
                name: "Antigravity",
                kind: ToolKind::Cask("antigravity-ide-linux", true),
            },
        ],
    ),
    (
        "In the terminal",
        &[
            DevTool {
                name: "Neovim",
                kind: ToolKind::Formula(&["neovim"]),
            },
            DevTool {
                name: "Helix",
                kind: ToolKind::Formula(&["helix"]),
            },
            DevTool {
                name: "vim",
                kind: ToolKind::Formula(&["vim"]),
            },
            DevTool {
                name: "micro",
                kind: ToolKind::Formula(&["micro"]),
            },
        ],
    ),
];

/// Every developer tool, numbered in the order of DEV_TOOLS
pub fn dev_tools() -> impl Iterator<Item = &'static DevTool> {
    DEV_TOOLS.iter().flat_map(|(_, tools)| tools.iter())
}

/// The Homebrew tools that are installed (numbers of dev_tools())
pub fn brew_installed() -> BTreeSet<usize> {
    dev_tools()
        .enumerate()
        .filter(|(_, tool)| match tool.kind {
            ToolKind::Formula(names) => brew_has_formula(names[0]),
            ToolKind::Cask(name, _) => brew_has_cask(name),
            ToolKind::Flatpak(_) => false,
        })
        .map(|(i, _)| i)
        .collect()
}

/// Installs developer tools with Homebrew, as the user (like `ujust
/// install-dev-tools`, with the devcontainer CLI it always adds)
pub async fn brew_install(formulas: Vec<&'static str>, casks: Vec<String>) -> bool {
    let brew = |args: Vec<String>| async move {
        tokio::process::Command::new(BREW)
            .args(&args)
            .env("HOMEBREW_NO_ENV_HINTS", "1")
            .stdin(std::process::Stdio::null())
            .status()
            .await
            .is_ok_and(|status| status.success())
    };
    let mut ok = true;
    if casks.iter().any(|cask| cask.starts_with("ublue-os/tap/")) {
        brew(vec!["tap".into(), "ublue-os/tap".into()]).await;
    }
    if !formulas.is_empty() {
        let mut args = vec!["install".to_owned(), "devcontainer".to_owned()];
        args.extend(formulas.iter().map(|name| (*name).to_owned()));
        ok &= brew(args).await;
    }
    for cask in casks {
        ok &= brew(vec!["install".into(), "--cask".into(), cask]).await;
    }
    ok
}

/// Removes developer tools installed with Homebrew (their formulas or cask;
/// the devcontainer CLI stays, other tools may need it)
#[allow(dead_code)] // The tour only installs
pub async fn brew_remove(formulas: Vec<&'static str>, casks: Vec<String>) -> bool {
    let brew = |args: Vec<String>| async move {
        tokio::process::Command::new(BREW)
            .args(&args)
            .env("HOMEBREW_NO_ENV_HINTS", "1")
            .stdin(std::process::Stdio::null())
            .status()
            .await
            .is_ok_and(|status| status.success())
    };
    let mut ok = true;
    if !formulas.is_empty() {
        let mut args = vec!["uninstall".to_owned(), "--formula".to_owned()];
        args.extend(formulas.iter().map(|name| (*name).to_owned()));
        ok &= brew(args).await;
    }
    for cask in casks {
        ok &= brew(vec!["uninstall".into(), "--cask".into(), cask]).await;
    }
    ok
}

/// The Homebrew name of a cask (in the ublue-os tap or Homebrew's own)
pub fn cask_name(name: &str, tap: bool) -> String {
    if tap {
        format!("ublue-os/tap/{name}")
    } else {
        name.to_owned()
    }
}
