// Web apps (webapps.rs): their folder in the app list. COSMIC's App Library
// reads its folders from ~/.config/cosmic/com.system76.CosmicAppLibrary/
// v1/groups, else from the system's copy in /usr/share/cosmic (Celenn OS
// ships one); a "Web apps" folder shows the launchers of the category
// X-Celenn-WebApp while there are web apps.
//
// The file is read and written with the App Library's own types
// (cosmic-applibrary's app_group.rs, AppGroup and FilterType), so that a
// change of its format in a new COSMIC shows: the file is then left as it
// is, and the unit tests fail on the system's copy.

use crate::i18n::tr;
use crate::webapp_launcher::{CATEGORY, installed};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// The app list's folders: this account's, or the system's
pub const GROUPS: &str = "cosmic/com.system76.CosmicAppLibrary/v1/groups";

/// A folder of the app list (cosmic-applibrary's AppGroup)
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AppGroup {
    pub name: String,
    pub icon: String,
    pub filter: FilterType,
}

/// What a folder shows (cosmic-applibrary's FilterType)
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum FilterType {
    AppIds(Vec<String>),
    Categories {
        categories: Vec<String>,
        exclude: Vec<String>,
        include: Vec<String>,
    },
    None,
}

/// The folders of a groups file; None when it is not one
pub fn groups(text: &str) -> Option<Vec<AppGroup>> {
    ron::from_str(text).ok()
}

/// Adds the app list's "Web apps" folder while there are web apps, and
/// takes it away with the last one (this account's folders then are the
/// system's again, unless the person changed them)
pub fn update_folder() {
    let mine = crate::system::xdg("XDG_CONFIG_HOME", ".config").join(GROUPS);
    let system = std::fs::read_to_string(Path::new("/usr/share").join(GROUPS)).ok();
    let Some(current) = std::fs::read_to_string(&mine)
        .ok()
        .or_else(|| system.clone())
    else {
        return;
    };
    let Some(updated) = with_folder(&current, !installed().is_empty(), tr("Web apps")) else {
        return;
    };
    if updated == current {
        return;
    }
    let same_as_system = system
        .as_deref()
        .and_then(groups)
        .zip(groups(&updated))
        .is_some_and(|(system, updated)| system == updated);
    if same_as_system {
        let _ = std::fs::remove_file(&mine);
        return;
    }
    if let Some(dir) = mine.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&mine, updated);
}

/// Whether a folder is the web apps' one
fn is_web_apps(group: &AppGroup) -> bool {
    matches!(&group.filter, FilterType::Categories { categories, .. }
        if categories.iter().any(|category| category == CATEGORY))
}

/// The folders (a groups file) with the web apps' folder or without it,
/// unchanged when it already is as wanted; None when they cannot be read
pub fn with_folder(text: &str, wanted: bool, name: &str) -> Option<String> {
    let mut folders = groups(text)?;
    if wanted == folders.iter().any(is_web_apps) {
        return Some(text.to_owned());
    }
    if wanted {
        folders.push(AppGroup {
            name: name.to_owned(),
            icon: "folder-symbolic".to_owned(),
            filter: FilterType::Categories {
                categories: vec![CATEGORY.to_owned()],
                exclude: Vec::new(),
                include: Vec::new(),
            },
        });
    } else {
        folders.retain(|group| !is_web_apps(group));
    }
    ron::ser::to_string_pretty(&folders, ron::ser::PrettyConfig::new())
        .ok()
        .map(|text| text + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The system's folders (system_files) read with the App Library's
    /// types: a COSMIC that changes their format fails here
    fn system_groups() -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../system_files/usr/share")
            .join(GROUPS);
        std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }

    #[test]
    fn reads_the_system_folders() {
        let folders = groups(&system_groups()).expect("the system's groups file");
        assert!(folders.len() > 3);
        assert!(!folders.iter().any(is_web_apps));
    }

    #[test]
    fn adds_and_removes_the_folder() {
        let system = system_groups();
        let with = with_folder(&system, true, "Applis web").unwrap();
        assert!(with.contains("name: \"Applis web\""), "{with}");
        assert!(with.contains("\"X-Celenn-WebApp\""), "{with}");
        let (before, after) = (groups(&system).unwrap(), groups(&with).unwrap());
        assert_eq!(after.len(), before.len() + 1);
        assert_eq!(after[..before.len()], before[..]);
        assert_eq!(with_folder(&with, true, "Web apps").unwrap(), with);
        let without = with_folder(&with, false, "Web apps").unwrap();
        assert_eq!(groups(&without).unwrap(), before);
        assert_eq!(with_folder(&system, false, "Web apps").unwrap(), system);
        assert_eq!(with_folder("not a list", true, "Web apps"), None);
        // A folder with a field this version does not know is left alone
        let newer = "[(name: \"A\", icon: \"a\", filter: None, popup: true)]";
        assert_eq!(with_folder(newer, true, "Web apps"), None);
        // Names with quotes stay one name
        let quoted = with_folder("[]", true, "Web \"apps\"").unwrap();
        assert_eq!(groups(&quoted).unwrap()[0].name, "Web \"apps\"");
    }
}
