// Web apps (webapps.rs): what makes one. A web app is Firefox (the Flatpak
// that comes with Celenn OS) with a profile of its own and without its
// toolbars:
//   ~/.local/share/applications/io.github.celenn_os.WebApp_<host>.desktop
//       the launcher: `flatpak run org.mozilla.firefox --name <id>
//       --profile <profile> <address>`; --name gives the window the
//       launcher's id (its Wayland app_id), so the dock shows it as its own
//       app with its own icon
//   ~/.local/share/icons/hicolor/{256x256,scalable}/apps/<id>.{png,svg}
//       its icon (webapp_icons.rs)
//   ~/.var/app/org.mozilla.firefox/data/celenn-web-apps/<host>/
//       the profile (Firefox's folder, so the Flatpak can use it): its
//       user.js hides the tabs and the address bar (chrome/userChrome.css),
//       and opens what would be a new tab in a new window instead
// "Also add to the dock" pins it to the dock's favorites.

use std::path::{Path, PathBuf};

/// What launchers of web apps start with
pub const ID_PREFIX: &str = "io.github.celenn_os.WebApp_";

/// The launchers' own category, which the app list's folder shows
pub const CATEGORY: &str = "X-Celenn-WebApp";

/// The launcher's key that keeps the web app's address
pub const URL_KEY: &str = "X-Celenn-WebApp-Url";

/// The same key, in launchers made before the project was renamed
pub const URL_KEY_BEFORE_RENAME: &str = "X-CosmicBlue-WebApp-Url";

pub const FIREFOX: &str = "org.mozilla.firefox";

/// A web app found on this account
#[derive(Clone, Debug, PartialEq)]
pub struct WebApp {
    pub id: String,
    pub name: String,
    pub url: String,
}

fn data_home() -> PathBuf {
    crate::system::xdg("XDG_DATA_HOME", ".local/share")
}

/// The folder of this account's launchers
pub fn applications() -> PathBuf {
    data_home().join("applications")
}

pub fn launcher(id: &str) -> PathBuf {
    applications().join(format!("{id}.desktop"))
}

pub fn icon_paths(id: &str) -> [PathBuf; 2] {
    let icons = data_home().join("icons/hicolor");
    [
        icons.join(format!("256x256/apps/{id}.png")),
        icons.join(format!("scalable/apps/{id}.svg")),
    ]
}

/// The web app's icon file, when it has one
pub fn icon_file(id: &str) -> Option<PathBuf> {
    icon_paths(id).into_iter().find(|path| path.is_file())
}

/// Its Firefox profile (in Firefox's own folder, which the Flatpak sees)
pub fn profile(id: &str) -> PathBuf {
    let name = id.strip_prefix(ID_PREFIX).unwrap_or(id);
    crate::system::home().join(format!(".var/app/{FIREFOX}/data/celenn-web-apps/{name}"))
}

pub fn firefox_installed() -> bool {
    ["/var/lib/flatpak/app", ".local/share/flatpak/app"]
        .iter()
        .any(|dir| {
            let dir = if dir.starts_with('/') {
                PathBuf::from(dir)
            } else {
                crate::system::home().join(dir)
            };
            dir.join(FIREFOX).is_dir()
        })
}

/// The site's host, without "www." ("https://www.netflix.com/" → netflix.com)
pub fn host(url: &str) -> Option<&str> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host = rest.split(['/', '?', '#']).next()?;
    let host = host.split(':').next()?;
    Some(host.strip_prefix("www.").unwrap_or(host))
}

/// The launcher id of a site: one per host ("web.whatsapp.com" →
/// io.github.celenn_os.WebApp_web_whatsapp_com). A dot becomes "_" and a
/// hyphen "__", so that two hosts never share an id: a host has no "_",
/// and no "-" next to a dot.
pub fn app_id(url: &str) -> Option<String> {
    let host = host(url)?.to_ascii_lowercase();
    if host.is_empty()
        || !host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return None;
    }
    Some(format!(
        "{ID_PREFIX}{}",
        host.replace('-', "__").replace('.', "_")
    ))
}

/// A web app's id as this tool makes them: what follows the prefix names
/// a folder (its profile, its icons), so only letters, digits and "_"
pub fn valid_id(id: &str) -> bool {
    id.strip_prefix(ID_PREFIX).is_some_and(|name| {
        !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// An address as typed ("laposte.fr", "https://www.laposte.fr/colis") made
/// into a web address; None when it does not look like one. Only characters
/// that are safe in a launcher's command line are kept.
pub fn normalize(typed: &str) -> Option<String> {
    let typed = typed.trim();
    if typed.is_empty() || typed.chars().any(char::is_whitespace) {
        return None;
    }
    let url = if typed.starts_with("https://") || typed.starts_with("http://") {
        typed.to_owned()
    } else if typed.contains("://") {
        return None;
    } else {
        format!("https://{typed}")
    };
    let allowed = |c: char| c.is_ascii_alphanumeric() || "-._~:/?#=&%+@!,;".contains(c);
    if !url.chars().all(allowed) {
        return None;
    }
    let host = host(&url)?;
    let labels: Vec<&str> = host.split('.').collect();
    let valid = labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
        && labels
            .last()
            .is_some_and(|tld| tld.chars().all(|c| c.is_ascii_alphabetic()) && tld.len() >= 2);
    valid.then(|| {
        // Only the site itself: the address of its home, as typed
        if url[url.find("://").unwrap_or(0) + 3..].contains('/') {
            url
        } else {
            format!("{url}/")
        }
    })
}

/// The web apps of this account, by name
pub fn installed() -> Vec<WebApp> {
    let Ok(entries) = std::fs::read_dir(applications()) else {
        return Vec::new();
    };
    let mut apps: Vec<WebApp> = entries
        .flatten()
        .filter_map(|entry| {
            let file = entry.file_name().into_string().ok()?;
            let id = file.strip_suffix(".desktop")?;
            if !valid_id(id) {
                return None;
            }
            let content = std::fs::read_to_string(entry.path()).ok()?;
            let value = |key: &str| {
                content
                    .lines()
                    .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
                    .map(str::to_owned)
            };
            Some(WebApp {
                id: id.to_owned(),
                name: value("Name")?,
                url: value(URL_KEY).or_else(|| value(URL_KEY_BEFORE_RENAME))?,
            })
        })
        .collect();
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps
}

/// The launcher: Firefox with the web app's profile, under its own id
pub fn desktop_entry(id: &str, name: &str, url: &str, profile: &Path) -> String {
    // (in Exec, % is written %%; the address has no quote nor space)
    let name = name.replace(['\n', '\r'], " ");
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name={name}\n\
         Comment={url}\n\
         Icon={id}\n\
         Exec=flatpak run {FIREFOX} --name {id} --profile \"{profile}\" \"{exec_url}\"\n\
         StartupWMClass={id}\n\
         StartupNotify=true\n\
         Terminal=false\n\
         Categories=Network;{CATEGORY};\n\
         {URL_KEY}={url}\n",
        profile = profile.display(),
        exec_url = url.replace('%', "%%"),
    )
}

/// Firefox without its tabs and address bar, opening on the site. A link
/// that would open a new tab (target="_blank", a middle click, a site that
/// opens a page of its own) opens a new window: a tab would stay hidden.
pub fn write_profile(profile: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(profile.join("chrome"))?;
    std::fs::write(
        profile.join("user.js"),
        "// Celenn OS web app (Celenn OS Tools)\n\
         user_pref(\"toolkit.legacyUserProfileCustomizations.stylesheets\", true);\n\
         user_pref(\"browser.tabs.inTitlebar\", 0);\n\
         user_pref(\"browser.toolbars.bookmarks.visibility\", \"never\");\n\
         user_pref(\"browser.shell.checkDefaultBrowser\", false);\n\
         user_pref(\"browser.aboutwelcome.enabled\", false);\n\
         user_pref(\"browser.startup.homepage_override.mstone\", \"ignore\");\n\
         user_pref(\"browser.sessionstore.resume_from_crash\", false);\n\
         user_pref(\"datareporting.policy.dataSubmissionPolicyBypassNotification\", true);\n\
         user_pref(\"media.eme.enabled\", true);\n\
         user_pref(\"browser.link.open_newwindow\", 2);\n\
         user_pref(\"browser.link.open_newwindow.override.external\", 2);\n\
         user_pref(\"browser.tabs.opentabfor.middleclick\", false);\n",
    )?;
    std::fs::write(
        profile.join("chrome/userChrome.css"),
        "/* Celenn OS web app: only the page */\n\
         #TabsToolbar, #nav-bar, #PersonalToolbar, #sidebar-main, #sidebar-box {\n  \
         visibility: collapse !important;\n}\n",
    )
}

/// The dock's favorites (cosmic-app-list's favorites: launcher ids)
const FAVORITES: &str = "cosmic/com.system76.CosmicAppList/v1/favorites";

/// The favorites of a favorites file; None when it is not one
pub fn favorites(text: &str) -> Option<Vec<String>> {
    ron::from_str(text).ok()
}

/// Pins a launcher to the dock, or unpins it (the dock's favorites); a
/// favorites file this version cannot read is left as it is
pub fn pin(id: &str, on: bool) {
    let file = crate::system::xdg("XDG_CONFIG_HOME", ".config").join(FAVORITES);
    let current = std::fs::read_to_string(&file)
        .or_else(|_| std::fs::read_to_string(Path::new("/usr/share").join(FAVORITES)))
        .unwrap_or_else(|_| "[]".to_owned());
    let Some(mut list) = favorites(&current) else {
        return;
    };
    if on == list.iter().any(|favorite| favorite == id) {
        return;
    }
    if on {
        list.push(id.to_owned());
    } else {
        list.retain(|favorite| favorite != id);
    }
    let Ok(text) = ron::ser::to_string_pretty(&list, ron::ser::PrettyConfig::new()) else {
        return;
    };
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&file, text);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_addresses() {
        assert_eq!(
            normalize("laposte.fr").as_deref(),
            Some("https://laposte.fr/")
        );
        assert_eq!(
            normalize(" https://www.laposte.fr/colis ").as_deref(),
            Some("https://www.laposte.fr/colis")
        );
        assert_eq!(normalize("laposte"), None);
        assert_eq!(normalize("ftp://laposte.fr"), None);
        assert_eq!(normalize("la poste.fr"), None);
        assert_eq!(normalize("laposte.fr/\"$(id)\""), None);
        assert_eq!(normalize("a.123"), None);
        assert_eq!(normalize("https://evil.com@laposte.fr/"), None);
    }

    #[test]
    fn ids_per_host() {
        assert_eq!(
            app_id("https://web.whatsapp.com/").as_deref(),
            Some("io.github.celenn_os.WebApp_web_whatsapp_com")
        );
        assert_eq!(
            app_id("https://www.netflix.com/"),
            app_id("https://netflix.com/fr/")
        );
        assert_eq!(app_id("https://LaPoste.fr/"), app_id("https://laposte.fr/"));
        // Hosts that used to share an id
        assert_ne!(app_id("https://a-b.com/"), app_id("https://a.b.com/"));
        assert_eq!(
            app_id("https://a-b.com/").as_deref(),
            Some("io.github.celenn_os.WebApp_a__b_com")
        );
        for (_, url, _) in crate::webapps::SITES {
            assert!(normalize(url).is_some(), "{url}");
            assert!(app_id(url).is_some_and(|id| valid_id(&id)), "{url}");
        }
    }

    #[test]
    fn validates_ids() {
        assert!(valid_id("io.github.celenn_os.WebApp_laposte_fr"));
        assert!(!valid_id("io.github.celenn_os.WebApp_"));
        assert!(!valid_id("io.github.celenn_os.WebApp_.."));
        assert!(!valid_id("io.github.celenn_os.WebApp_a/b"));
        assert!(!valid_id("io.github.celenn_os.WebApp_a b"));
        assert!(!valid_id("org.mozilla.firefox"));
    }

    #[test]
    fn launcher_runs_firefox() {
        let entry = desktop_entry(
            "io.github.celenn_os.WebApp_laposte_fr",
            "La Poste",
            "https://laposte.fr/?a=1%20",
            Path::new("/home/a/.var/app/org.mozilla.firefox/data/celenn-web-apps/laposte_fr"),
        );
        assert!(entry.contains(
            "Exec=flatpak run org.mozilla.firefox --name io.github.celenn_os.WebApp_laposte_fr --profile \"/home/a/.var/app/org.mozilla.firefox/data/celenn-web-apps/laposte_fr\" \"https://laposte.fr/?a=1%%20\"\n"
        ));
        assert!(entry.contains("StartupWMClass=io.github.celenn_os.WebApp_laposte_fr\n"));
    }

    /// The system's dock favorites (system_files) read as cosmic-app-list
    /// writes them: a COSMIC that changes their format fails here
    #[test]
    fn reads_the_system_favorites() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../system_files/usr/share")
            .join(FAVORITES);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let list = favorites(&text).expect("the system's favorites file");
        assert!(
            list.iter().any(|id| id == "org.mozilla.firefox"),
            "{list:?}"
        );
    }
}
