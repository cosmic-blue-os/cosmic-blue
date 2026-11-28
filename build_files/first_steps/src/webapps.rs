// Web apps, a tool of Celenn OS Tools (tools.rs): a site in its own
// window, with its icon in the app list and in the dock, as on a phone.
//
// A web app is Firefox (the Flatpak that comes with Celenn OS) with a
// profile of its own and without its toolbars:
//   ~/.local/share/applications/io.github.celenn_os.WebApp_<host>.desktop
//       the launcher: `flatpak run org.mozilla.firefox --name <id>
//       --profile <profile> <address>`; --name gives the window the
//       launcher's id (its Wayland app_id), so the dock shows it as its own
//       app with its own icon
//   ~/.local/share/icons/hicolor/{256x256,scalable}/apps/<id>.{png,svg}
//       the site's icon (its "apple-touch-icon" or favicon), or a round
//       letter when the site has none; framed with the margin that the
//       system's icons have (a site's logo fills its whole square)
//   ~/.config/cosmic/com.system76.CosmicAppLibrary/v1/groups
//       a "Web apps" folder in the app list, while there are web apps
//       (launchers have the category X-Celenn-WebApp)
//   ~/.var/app/org.mozilla.firefox/data/celenn-web-apps/<host>/
//       the profile (Firefox's folder, so the Flatpak can use it): its
//       user.js hides the tabs and the address bar (chrome/userChrome.css)
// Firefox's system policies (/usr/share/celenn/firefox: uBlock Origin,
// no telemetry, HTTPS-Only...) apply to every profile, and so do the
// parental controls (the DNS filter, Firefox allowed or not for a child).
// People sign in once in each web app: each has its own cookies.
// "Also add to the dock" (off by default) pins it to the dock's favorites.

use crate::common::{ToastAsk, dim_caption, label, row_title, toast, tool_icon};
use crate::i18n::{fill, tr};
use cosmic::app::Task;
use cosmic::iced::core::text::{Ellipsize, EllipsizeHeightLimit, Wrapping};
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, settings};
use cosmic::{Apply, Element, cosmic_theme, theme};
use std::path::{Path, PathBuf};

/// What launchers of web apps start with
const ID_PREFIX: &str = "io.github.celenn_os.WebApp_";
/// The launchers' own category, which the app list's folder shows
const CATEGORY: &str = "X-Celenn-WebApp";
/// The app list's folders: this account's, or the system's
const GROUPS: &str = "cosmic/com.system76.CosmicAppLibrary/v1/groups";
/// How much of an icon's square its picture fills (as the system's icons)
const ICON_FILL: f32 = 0.82;
/// The launcher's key that keeps the web app's address
const URL_KEY: &str = "X-Celenn-WebApp-Url";
/// The same key, in launchers made before the project was renamed
const URL_KEY_BEFORE_RENAME: &str = "X-CosmicBlue-WebApp-Url";
const FIREFOX: &str = "org.mozilla.firefox";
/// Tiles on a line (the page is as wide as the toolbox, WIDE in tools.rs):
/// three, so that names and captions are not cut, in tiles as low as the
/// toolbox's smaller ones, so that the page still fits without scrolling
/// in the window COSMIC opens on a 1080p screen (693 px high)
const PER_LINE: usize = 3;

/// Sites people use most, in the order of the grid: name, address, caption
pub const SITES: &[(&str, &str, &str)] = &[
    ("WhatsApp", "https://web.whatsapp.com/", "Messages"),
    ("Messenger", "https://www.messenger.com/", "Messages"),
    (
        "Instagram",
        "https://www.instagram.com/",
        "Photos and stories",
    ),
    ("Gmail", "https://mail.google.com/", "Email"),
    ("Outlook", "https://outlook.live.com/mail/", "Email"),
    ("Netflix", "https://www.netflix.com/", "Films and series"),
    ("Disney+", "https://www.disneyplus.com/", "Films and series"),
    ("YouTube", "https://www.youtube.com/", "Videos"),
    ("Spotify", "https://open.spotify.com/", "Music"),
    ("Deezer", "https://www.deezer.com/", "Music"),
    (
        "Microsoft 365",
        "https://www.office.com/",
        "Word and Excel online",
    ),
    (
        "Google Docs",
        "https://docs.google.com/",
        "Documents and sheets",
    ),
    ("Canva", "https://www.canva.com/", "Posters and cards"),
    ("Google Photos", "https://photos.google.com/", "Your photos"),
];

pub const TEXTS: &[&str] = &[
    "Messages",
    "Email",
    "Films and series",
    "Videos",
    "Music",
    "Word and Excel online",
    "Documents and sheets",
    "Posters and cards",
    "Your photos",
    "Photos and stories",
    "Added",
    "Adding…",
    "Another site",
    "Its name and icon are found by themselves.",
    "For example laposte.fr",
    "Add",
    "Also add to the dock",
    "Otherwise its icon shows in the dock while it is open.",
    "Remove",
    "They open with Firefox: the protection against ads and tracking, and the parental controls, apply to them too. Sign in once in each one.",
    "{} is in your apps.",
    "Open",
    "Remove {}? You will have to sign in again if you add it back.",
    "{} was removed.",
    "This address does not look right. Type it as in the address bar, for example laposte.fr.",
    "{} could not be added. Is this computer connected to the internet?",
    "Firefox is missing. Put it back in Repair.",
];

/// A web app found on this account
#[derive(Clone, Debug, PartialEq)]
pub struct WebApp {
    pub id: String,
    pub name: String,
    pub url: String,
}

#[derive(Clone, Debug)]
pub enum Message {
    /// A site of the grid: add it, or ask before removing it
    Site(usize),
    Address(String),
    AddAddress,
    Dock(bool),
    Added(Result<WebApp, String>),
    /// Ask before removing (its id)
    AskRemove(String),
    Remove(String),
    Launch(String),
    Toast(ToastAsk<Message>),
    /// The popular sites' icons are in the cache
    IconsCached,
}

pub struct Page {
    apps: Vec<WebApp>,
    address: String,
    dock: bool,
    /// Addresses being added
    adding: Vec<String>,
    /// The popular sites' icons (from the cache), in the order of SITES
    site_icons: Vec<Option<PathBuf>>,
    /// Their icons are being fetched
    caching: bool,
}

impl Page {
    pub fn new() -> Page {
        Page {
            apps: installed(),
            address: String::new(),
            dock: false,
            adding: Vec::new(),
            site_icons: site_icons(),
            caching: false,
        }
    }

    /// The tool was opened: its web apps read again, and the popular sites'
    /// icons fetched from the sites themselves when not in the cache yet
    pub fn opened(&mut self) -> Task<Message> {
        update_older();
        self.apps = installed();
        if self.caching || !SITES.iter().any(|(_, url, _)| icon_missing(url)) {
            return Task::none();
        }
        self.caching = true;
        Task::perform(
            async {
                let _ = tokio::task::spawn_blocking(cache_icons).await;
            },
            |()| cosmic::Action::App(Message::IconsCached),
        )
    }

    /// What its tile says
    pub fn summary(&self) -> Option<String> {
        (!self.apps.is_empty()).then(|| {
            self.apps
                .iter()
                .map(|app| app.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
    }

    fn app_of(&self, url: &str) -> Option<&WebApp> {
        let id = app_id(url)?;
        self.apps.iter().find(|app| app.id == id)
    }

    fn add(&mut self, name: Option<&'static str>, url: String) -> Task<Message> {
        if !firefox_installed() {
            return toast(
                tr("Firefox is missing. Put it back in Repair.").to_owned(),
                None,
                Message::Toast,
            );
        }
        if self.adding.contains(&url) {
            return Task::none();
        }
        self.adding.push(url.clone());
        let dock = self.dock;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || create(name, &url, dock))
                    .await
                    .unwrap_or_else(|error| Err(error.to_string()))
            },
            |result| cosmic::Action::App(Message::Added(result)),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Site(i) => {
                let Some(&(name, url, _)) = SITES.get(i) else {
                    return Task::none();
                };
                match self.app_of(url) {
                    Some(app) => self.update(Message::AskRemove(app.id.clone())),
                    None => self.add(Some(name), url.to_owned()),
                }
            }
            Message::Address(address) => {
                self.address = address;
                Task::none()
            }
            Message::AddAddress => match normalize(&self.address) {
                Some(url) => {
                    self.address.clear();
                    self.add(None, url)
                }
                None => toast(
                    tr("This address does not look right. Type it as in the address bar, for example laposte.fr.")
                        .to_owned(),
                    None,
                    Message::Toast,
                ),
            },
            Message::Dock(on) => {
                self.dock = on;
                Task::none()
            }
            Message::Added(result) => {
                self.apps = installed();
                match result {
                    Ok(app) => {
                        self.adding.retain(|url| *url != app.url);
                        toast(
                            fill(tr("{} is in your apps."), &app.name),
                            Some((tr("Open").to_owned(), Message::Launch(app.id.clone()))),
                            Message::Toast,
                        )
                    }
                    Err(url) => {
                        self.adding.retain(|adding| *adding != url);
                        let host = host(&url).unwrap_or(&url).to_owned();
                        toast(
                            fill(
                                tr("{} could not be added. Is this computer connected to the internet?"),
                                &host,
                            ),
                            None,
                            Message::Toast,
                        )
                    }
                }
            }
            Message::AskRemove(id) => {
                let Some(app) = self.apps.iter().find(|app| app.id == id) else {
                    return Task::none();
                };
                toast(
                    fill(
                        tr("Remove {}? You will have to sign in again if you add it back."),
                        &app.name,
                    ),
                    Some((tr("Remove").to_owned(), Message::Remove(id))),
                    Message::Toast,
                )
            }
            Message::Remove(id) => {
                let name = self
                    .apps
                    .iter()
                    .find(|app| app.id == id)
                    .map(|app| app.name.clone());
                remove(&id);
                self.apps = installed();
                match name {
                    Some(name) => toast(fill(tr("{} was removed."), &name), None, Message::Toast),
                    None => Task::none(),
                }
            }
            Message::Launch(id) => {
                let file = launcher(&id);
                crate::common::spawn("gio", &["launch", &file.to_string_lossy()]);
                Task::none()
            }
            Message::IconsCached => {
                self.caching = false;
                self.site_icons = site_icons();
                Task::none()
            }
            // (tools.rs shows it)
            Message::Toast(_) => Task::none(),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let cosmic_theme::Spacing { space_s, .. } = theme::spacing();
        // The popular sites, then the other web apps, three per line
        let mut tiles: Vec<Element<'_, Message>> = SITES
            .iter()
            .enumerate()
            .map(|(i, &(name, url, caption))| {
                let app = self.app_of(url);
                let adding = self.adding.iter().any(|adding| adding == url);
                let caption = if adding {
                    tr("Adding…").to_owned()
                } else if app.is_some() {
                    tr("Added").to_owned()
                } else {
                    tr(caption).to_owned()
                };
                let icon = app
                    .and_then(|app| icon_file(&app.id))
                    .or_else(|| self.site_icons.get(i).cloned().flatten());
                tile(
                    name.to_owned(),
                    caption,
                    icon,
                    app.is_some(),
                    (!adding).then_some(Message::Site(i)),
                )
            })
            .collect();
        tiles.extend(
            self.apps
                .iter()
                .filter(|app| {
                    !SITES
                        .iter()
                        .any(|(_, url, _)| app_id(url).as_deref() == Some(&app.id))
                })
                .map(|app| {
                    tile(
                        app.name.clone(),
                        host(&app.url).unwrap_or(&app.url).to_owned(),
                        icon_file(&app.id),
                        true,
                        Some(Message::AskRemove(app.id.clone())),
                    )
                }),
        );
        tiles.extend(
            self.adding
                .iter()
                .filter(|url| !SITES.iter().any(|(_, site, _)| site == *url))
                .map(|url| {
                    tile(
                        host(url).unwrap_or(url).to_owned(),
                        tr("Adding…").to_owned(),
                        None,
                        false,
                        None,
                    )
                }),
        );
        let mut grid =
            widget::column::with_capacity(tiles.len().div_ceil(PER_LINE)).spacing(space_s);
        let mut tiles = tiles.into_iter().peekable();
        while tiles.peek().is_some() {
            let mut line = widget::row::with_capacity(PER_LINE).spacing(space_s);
            let mut count = 0;
            for tile in tiles.by_ref().take(PER_LINE) {
                line = line.push(tile);
                count += 1;
            }
            for _ in count..PER_LINE {
                line = line.push(widget::space::horizontal().width(Length::Fill));
            }
            grid = grid.push(line);
        }

        // Another site: its address
        let address_ok = normalize(&self.address).is_some();
        let other = settings::item_row(vec![
            label(
                tr("Another site"),
                tr("Its name and icon are found by themselves.").to_owned(),
            ),
            widget::text_input(tr("For example laposte.fr"), &self.address)
                .on_input(Message::Address)
                .on_submit(|_| Message::AddAddress)
                .width(Length::Fixed(200.0))
                .into(),
            widget::button::suggested(tr("Add"))
                .on_press_maybe(address_ok.then_some(Message::AddAddress))
                .into(),
        ])
        .spacing(space_s);
        let dock = settings::item_row(vec![
            label(
                tr("Also add to the dock"),
                tr("Otherwise its icon shows in the dock while it is open.").to_owned(),
            ),
            widget::toggler(self.dock).on_toggle(Message::Dock).into(),
        ])
        .spacing(space_s);
        widget::column::with_capacity(4)
            .spacing(space_s)
            .push(grid)
            .push(settings::section().add(other).add(dock))
            .push(
                dim_caption(tr(
                    "They open with Firefox: the protection against ads and tracking, and the parental controls, apply to them too. Sign in once in each one.",
                ))
                .wrapping(Wrapping::Word),
            )
            .into()
    }
}

/// A site's tile: its icon (a globe while it has none), its name, a caption,
/// and a tick once added (a "+" before)
fn tile<'a>(
    name: String,
    caption: String,
    icon: Option<PathBuf>,
    added: bool,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let space_s = theme::spacing().space_s;
    let one_line = Ellipsize::End(EllipsizeHeightLimit::Lines(1));
    let icon = match icon {
        Some(file) => widget::icon(widget::icon::from_path(file)).size(32).into(),
        None => tool_icon("web-browser-symbolic", 32),
    };
    let mark = widget::icon::from_name(if added {
        "object-select-symbolic"
    } else {
        "list-add-symbolic"
    })
    .size(16);
    widget::row::with_capacity(3)
        .spacing(space_s)
        .align_y(Alignment::Center)
        .push(icon)
        .push(
            widget::column::with_capacity(2)
                .spacing(2)
                .push(row_title(name).ellipsize(one_line))
                .push(dim_caption(caption).ellipsize(one_line))
                .width(Length::Fill)
                .clip(true),
        )
        .push(mark)
        .padding([0, space_s])
        .height(Length::Fill)
        .apply(widget::button::custom)
        .class(crate::style::tile_style(added))
        .padding(0)
        .width(Length::Fill)
        .height(Length::Fixed(46.0))
        .on_press_maybe(on_press)
        .into()
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

fn data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home().join(".local/share"))
}

fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home().join(".config"))
}

fn launcher(id: &str) -> PathBuf {
    data_home().join(format!("applications/{id}.desktop"))
}

fn icon_paths(id: &str) -> [PathBuf; 2] {
    let icons = data_home().join("icons/hicolor");
    [
        icons.join(format!("256x256/apps/{id}.png")),
        icons.join(format!("scalable/apps/{id}.svg")),
    ]
}

/// The web app's icon file, when it has one
fn icon_file(id: &str) -> Option<PathBuf> {
    icon_paths(id).into_iter().find(|path| path.is_file())
}

/// Where a site's icon is kept, fetched from the site itself (nothing is
/// shipped in the image): PNG, SVG, and a mark saying the site has none
fn cache_paths(url: &str) -> Option<[PathBuf; 3]> {
    let id = app_id(url)?;
    let name = id.strip_prefix(ID_PREFIX).unwrap_or(&id);
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home().join(".cache"))
        .join("celenn/site-icons");
    Some([
        cache.join(format!("{name}.png")),
        cache.join(format!("{name}.svg")),
        cache.join(format!("{name}.none")),
    ])
}

/// A site's icon in the cache
fn cached_icon(url: &str) -> Option<PathBuf> {
    let [png, svg, _] = cache_paths(url)?;
    [png, svg].into_iter().find(|path| path.is_file())
}

/// The site's icon is not in the cache, and the site was not asked for it
/// since yesterday
fn icon_missing(url: &str) -> bool {
    let Some([_, _, none]) = cache_paths(url) else {
        return false;
    };
    let asked_lately = std::fs::metadata(&none)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.elapsed().ok())
        .is_some_and(|age| age.as_secs() < 24 * 3600);
    cached_icon(url).is_none() && !asked_lately
}

fn site_icons() -> Vec<Option<PathBuf>> {
    SITES.iter().map(|(_, url, _)| cached_icon(url)).collect()
}

/// Fetches the popular sites' missing icons, all at once
fn cache_icons() {
    // (icons of older versions, not framed)
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home().join(".cache"));
    let _ = std::fs::remove_dir_all(cache.join("celenn/web-app-icons"));
    let threads: Vec<_> = SITES
        .iter()
        .filter(|(_, url, _)| icon_missing(url))
        .map(|&(_, url, _)| std::thread::spawn(move || cache_icon(url)))
        .collect();
    for thread in threads {
        let _ = thread.join();
    }
}

fn cache_icon(url: &str) {
    let Some([png, svg, none]) = cache_paths(url) else {
        return;
    };
    if let Some(dir) = png.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let page = fetch(url);
    let base = page
        .as_ref()
        .map_or_else(|| url.to_owned(), |(_, final_url)| final_url.clone());
    // (written aside first: the page may read the cache meanwhile)
    let [png_part, svg_part] = [
        png.with_extension("png.part"),
        svg.with_extension("svg.part"),
    ];
    let found = matches!(
        site_icon(
            &png_part,
            &svg_part,
            page.as_ref().map(|(html, _)| html.as_str()),
            url,
            &base
        ),
        Ok(true)
    );
    let _ = std::fs::rename(&png_part, &png);
    let _ = std::fs::rename(&svg_part, &svg);
    if found {
        let _ = std::fs::remove_file(&none);
        // A web app of this site made with a letter or older logo
        if let (Some(id), Some(icon)) = (app_id(url), cached_icon(url))
            && launcher(&id).is_file()
            && set_icon(&id, &icon).is_ok()
        {
            refresh_app_list();
        }
    } else {
        let _ = std::fs::write(&none, "");
    }
}

/// Its Firefox profile (in Firefox's own folder, which the Flatpak sees)
fn profile(id: &str) -> PathBuf {
    let name = id.strip_prefix(ID_PREFIX).unwrap_or(id);
    home().join(format!(".var/app/{FIREFOX}/data/celenn-web-apps/{name}"))
}

fn firefox_installed() -> bool {
    ["/var/lib/flatpak/app", ".local/share/flatpak/app"]
        .iter()
        .any(|dir| {
            let dir = if dir.starts_with('/') {
                PathBuf::from(dir)
            } else {
                home().join(dir)
            };
            dir.join(FIREFOX).is_dir()
        })
}

/// The site's host, without "www." ("https://www.netflix.com/" → netflix.com)
fn host(url: &str) -> Option<&str> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host = rest.split(['/', '?', '#']).next()?;
    let host = host.split(':').next()?;
    Some(host.strip_prefix("www.").unwrap_or(host))
}

/// The launcher id of a site: one per host ("web.whatsapp.com" →
/// io.github.celenn_os.WebApp_web_whatsapp_com)
fn app_id(url: &str) -> Option<String> {
    let host = host(url)?;
    Some(format!("{ID_PREFIX}{}", host.replace(['.', '-'], "_")))
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
    let dir = data_home().join("applications");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut apps: Vec<WebApp> = entries
        .flatten()
        .filter_map(|entry| {
            let file = entry.file_name().into_string().ok()?;
            let id = file.strip_suffix(".desktop")?;
            if !id.starts_with(ID_PREFIX) {
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

/// Makes a web app: its icon, its profile, its launcher; maybe pins it to
/// the dock. The error is its address.
fn create(name: Option<&str>, url: &str, dock: bool) -> Result<WebApp, String> {
    let fail = |_| url.to_owned();
    let id = app_id(url).ok_or_else(|| url.to_owned())?;
    let page = fetch(url);
    // A site we do not know must answer (a typing mistake otherwise)
    if name.is_none() && page.is_none() {
        return Err(url.to_owned());
    }
    let name = match name {
        Some(name) => name.to_owned(),
        None => page
            .as_ref()
            .and_then(|(html, _)| site_name(html))
            .unwrap_or_else(|| name_from_host(host(url).unwrap_or(url))),
    };
    let base = page
        .as_ref()
        .map_or_else(|| url.to_owned(), |(_, final_url)| final_url.clone());
    install_icon(
        &id,
        &name,
        url,
        page.as_ref().map(|(html, _)| html.as_str()),
        &base,
    )
    .map_err(fail)?;
    let profile = profile(&id);
    write_profile(&profile).map_err(fail)?;
    let launcher_file = launcher(&id);
    if let Some(dir) = launcher_file.parent() {
        std::fs::create_dir_all(dir).map_err(fail)?;
    }
    std::fs::write(&launcher_file, desktop_entry(&id, &name, url, &profile)).map_err(fail)?;
    if dock {
        pin(&id, true);
    }
    // The app list and the dock read the new launcher
    let _ = std::process::Command::new("update-desktop-database")
        .arg(data_home().join("applications"))
        .status();
    update_folder();
    Ok(WebApp {
        id,
        name,
        url: url.to_owned(),
    })
}

/// Removes a web app: launcher, icon, profile (its sign-ins), dock pin
fn remove(id: &str) {
    if !id.starts_with(ID_PREFIX) || id.contains('/') {
        return;
    }
    let _ = std::fs::remove_file(launcher(id));
    for icon in icon_paths(id) {
        let _ = std::fs::remove_file(icon);
    }
    let _ = std::fs::remove_dir_all(profile(id));
    pin(id, false);
    update_folder();
}

/// What web apps made by an older version lack, made once per account
/// (a number in ~/.local/state/celenn/web-apps): 2 = the folder's category,
/// framed icons, the site's own logo when the cache now has it
const WEB_APPS_VERSION: &str = "2";

/// Brings web apps made by an older version up to date: launcher with the
/// folder's category and the current keys, framed icon (the site's cached
/// logo when there is one), the folder. Runs when Tools starts and in
/// housekeeping, so people do not have to open the page.
pub fn update_older() {
    let state = xdg_state().join("celenn/web-apps");
    let done = std::fs::read_to_string(&state).is_ok_and(|v| v.trim() == WEB_APPS_VERSION);
    let apps = installed();
    if !done {
        for app in &apps {
            let file = launcher(&app.id);
            let entry = desktop_entry(&app.id, &app.name, &app.url, &profile(&app.id));
            if std::fs::read_to_string(&file).is_ok_and(|content| content != entry) {
                let _ = std::fs::write(&file, entry);
            }
            let source = cached_icon(&app.url).or_else(|| icon_file(&app.id));
            if let Some(source) = source {
                let _ = set_icon(&app.id, &source);
            }
        }
        if !apps.is_empty() {
            refresh_app_list();
        }
        if let Some(dir) = state.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&state, WEB_APPS_VERSION);
    }
    update_folder();
}

fn xdg_state() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home().join(".local/state"))
}

/// The web app's icon, framed, from an icon file (which may be its own)
fn set_icon(id: &str, source: &Path) -> std::io::Result<()> {
    let [png, svg] = icon_paths(id);
    let staged = std::env::temp_dir().join(format!(
        "{}-{}.{}",
        id,
        std::process::id(),
        source
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("png")
    ));
    std::fs::copy(source, &staged)?;
    for path in [&png, &svg] {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let _ = std::fs::remove_file(path);
    }
    let framed = frame_file(&staged, &png, &svg);
    let _ = std::fs::remove_file(&staged);
    framed
}

/// The app list and the dock read launchers and icons again
fn refresh_app_list() {
    let _ = std::process::Command::new("update-desktop-database")
        .arg(data_home().join("applications"))
        .status();
    // (the icon theme's folder changed: icon caches look at its time)
    let hicolor = data_home().join("icons/hicolor");
    if let Ok(dir) = std::fs::File::open(&hicolor) {
        let _ = dir.set_modified(std::time::SystemTime::now());
    }
}

/// Adds the app list's "Web apps" folder while there are web apps, and
/// takes it away with the last one (this account's folders then are the
/// system's again, unless the person changed them)
fn update_folder() {
    let mine = config_home().join(GROUPS);
    let system = std::fs::read_to_string(Path::new("/usr/share").join(GROUPS)).ok();
    let Some(current) = std::fs::read_to_string(&mine)
        .ok()
        .or_else(|| system.clone())
    else {
        return;
    };
    let Some(groups) = with_folder(&current, !installed().is_empty(), tr("Web apps")) else {
        return;
    };
    if groups == current {
        return;
    }
    let same_as_system = system
        .as_deref()
        .and_then(ron_items)
        .zip(ron_items(&groups))
        .is_some_and(|(system, groups)| system == groups);
    if same_as_system {
        let _ = std::fs::remove_file(&mine);
        return;
    }
    if let Some(dir) = mine.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&mine, groups);
}

/// The folders (a RON list) with the web apps' folder or without it; None
/// when they cannot be read
pub fn with_folder(groups: &str, wanted: bool, name: &str) -> Option<String> {
    let mut items = ron_items(groups)?;
    let mark = format!("\"{CATEGORY}\"");
    let present = items.iter().any(|item| item.contains(&mark));
    if wanted == present {
        return Some(groups.to_owned());
    }
    if wanted {
        let name = name.replace(['"', '\\'], "");
        items.push(format!(
            "(\n        name: \"{name}\",\n        icon: \"folder-symbolic\",\n        \
             filter: Categories(\n            categories: [{mark}],\n            \
             exclude: [],\n            include: [],\n        ),\n    )"
        ));
    } else {
        items.retain(|item| !item.contains(&mark));
    }
    let body: String = items.iter().map(|item| format!("    {item},\n")).collect();
    Some(format!("[\n{body}]\n"))
}

/// The items of a RON list, as written (trimmed); None when it is not one
fn ron_items(text: &str) -> Option<Vec<String>> {
    let text = text.trim();
    let inner = text.strip_prefix('[')?.strip_suffix(']')?;
    let mut items = Vec::new();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    let mut start = 0;
    for (at, c) in inner.char_indices() {
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
            ',' if depth == 0 => {
                items.push(inner[start..at].trim().to_owned());
                start = at + 1;
            }
            _ => {}
        }
        if depth < 0 {
            return None;
        }
    }
    if depth != 0 || in_string {
        return None;
    }
    items.push(inner[start..].trim().to_owned());
    items.retain(|item| !item.is_empty());
    Some(items)
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

/// Firefox without its tabs and address bar, opening on the site
fn write_profile(profile: &Path) -> std::io::Result<()> {
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
         user_pref(\"media.eme.enabled\", true);\n",
    )?;
    std::fs::write(
        profile.join("chrome/userChrome.css"),
        "/* Celenn OS web app: only the page */\n\
         #TabsToolbar, #nav-bar, #PersonalToolbar, #sidebar-main, #sidebar-box {\n  \
         visibility: collapse !important;\n}\n",
    )
}

/// The page of a site and the address it ended on (after redirections)
fn fetch(url: &str) -> Option<(String, String)> {
    let output = std::process::Command::new("curl")
        .args([
            "--silent",
            "--location",
            "--max-time",
            "15",
            "--max-filesize",
            "4000000",
            "--compressed",
            "--user-agent",
            "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0",
            // (some sites only answer what looks like a browser)
            "--header",
            "Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            "--header",
            "Accept-Language: en-US,en;q=0.5",
            "--write-out",
            "\n%{url_effective}",
            "--",
            url,
        ])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let body = String::from_utf8_lossy(&output.stdout).into_owned();
    let (html, final_url) = body.rsplit_once('\n')?;
    // curl prints the address even when the site did not answer
    if html.trim().is_empty() {
        return None;
    }
    Some((html.to_owned(), final_url.trim().to_owned()))
}

/// Bytes at an address (an icon)
fn download(url: &str) -> Option<Vec<u8>> {
    let output = std::process::Command::new("curl")
        .args([
            "--silent",
            "--fail",
            "--location",
            "--max-time",
            "10",
            "--max-filesize",
            "2000000",
            "--user-agent",
            "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0",
            "--",
            url,
        ])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    (output.status.success() && !output.stdout.is_empty()).then_some(output.stdout)
}

/// The value of an attribute in a tag (`<link rel="icon" href="...">`)
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(at) = lower[from..].find(name) {
        let start = from + at;
        from = start + name.len();
        // A whole attribute name, followed by =
        let before = lower[..start].chars().last();
        if before.is_some_and(|c| !c.is_ascii_whitespace()) {
            continue;
        }
        let rest = tag[from..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start();
        return match rest.chars().next() {
            Some(quote @ ('"' | '\'')) => rest[1..].split(quote).next(),
            _ => rest
                .split(|c: char| c.is_ascii_whitespace() || c == '>')
                .next(),
        };
    }
    None
}

/// The tags of a kind in a page (`<link ...>`, `<meta ...>`)
fn tags<'a>(html: &'a str, kind: &str) -> Vec<&'a str> {
    let lower = html.to_ascii_lowercase();
    let open = format!("<{kind}");
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = lower[from..].find(&open) {
        let start = from + at;
        let end = lower[start..]
            .find('>')
            .map_or(lower.len(), |end| start + end);
        found.push(&html[start..end]);
        from = end;
    }
    found
}

/// Basic HTML entities in a name
fn unescape(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&quot;", "\"")
        .replace("&nbsp;", " ")
}

/// The site's name: its application-name, og:site_name, or the start of its
/// title
fn site_name(html: &str) -> Option<String> {
    let metas = tags(html, "meta");
    let meta = |key: &str| {
        metas.iter().find_map(|tag| {
            let named = attribute(tag, "name")
                .or_else(|| attribute(tag, "property"))
                .is_some_and(|name| name.eq_ignore_ascii_case(key));
            named
                .then(|| attribute(tag, "content"))
                .flatten()
                .map(str::trim)
                .filter(|content| !content.is_empty())
        })
    };
    let name = meta("application-name")
        .or_else(|| meta("og:site_name"))
        .map(str::to_owned)
        .or_else(|| {
            let lower = html.to_ascii_lowercase();
            let start = lower.find("<title")?;
            let start = start + lower[start..].find('>')? + 1;
            let end = start + lower[start..].find("</title")?;
            let title = html[start..end].trim();
            let first = [" | ", " - ", " – ", " — ", " · ", " : "]
                .iter()
                .fold(title, |title, sep| title.split(sep).next().unwrap_or(title));
            Some(first.trim().to_owned())
        })?;
    let name: String = unescape(&name).chars().take(40).collect();
    (!name.trim().is_empty()).then(|| name.trim().to_owned())
}

/// A name from the host: laposte.fr → Laposte
fn name_from_host(host: &str) -> String {
    let labels: Vec<&str> = host.split('.').collect();
    let main = if labels.len() >= 2 {
        labels[labels.len() - 2]
    } else {
        host
    };
    let mut chars = main.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// An address in a page, made absolute
fn absolute(base: &str, href: &str) -> Option<String> {
    let href = href.trim();
    if href.starts_with("data:") || href.is_empty() {
        return None;
    }
    if href.starts_with("https://") || href.starts_with("http://") {
        return Some(href.to_owned());
    }
    let scheme_end = base.find("://")? + 3;
    let scheme = &base[..scheme_end - 3];
    if let Some(rest) = href.strip_prefix("//") {
        return Some(format!("{scheme}://{rest}"));
    }
    let host_end = base[scheme_end..]
        .find('/')
        .map_or(base.len(), |at| scheme_end + at);
    let origin = &base[..host_end];
    if href.starts_with('/') {
        return Some(format!("{origin}{href}"));
    }
    let path = &base[host_end..];
    let path = path.split(['?', '#']).next().unwrap_or("");
    let dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);
    Some(format!("{origin}{dir}/{href}"))
}

/// The icons a page offers, the best first: its large icons (touch icons
/// and icons with a size of 120 or more), its SVG icon, the others, then the
/// usual addresses
fn icon_candidates(html: Option<&str>, base: &str) -> Vec<String> {
    let mut ranked: Vec<(u32, String)> = Vec::new();
    for tag in html.map(|html| tags(html, "link")).unwrap_or_default() {
        let Some(rel) = attribute(tag, "rel").map(str::to_ascii_lowercase) else {
            continue;
        };
        let rels: Vec<&str> = rel.split_whitespace().collect();
        let touch = rels.iter().any(|r| r.starts_with("apple-touch-icon"));
        if !touch && !rels.contains(&"icon") {
            continue;
        }
        let Some(href) = attribute(tag, "href").and_then(|href| absolute(base, href)) else {
            continue;
        };
        let size = attribute(tag, "sizes")
            .and_then(|sizes| {
                sizes
                    .split_whitespace()
                    .filter_map(|size| size.split(['x', 'X']).next()?.parse::<u32>().ok())
                    .max()
            })
            .unwrap_or(if touch { 180 } else { 0 });
        let svg = href.to_ascii_lowercase().ends_with(".svg")
            || attribute(tag, "type").is_some_and(|kind| kind.contains("svg"));
        let rank = if size >= 120 {
            1000 + size.min(512)
        } else if svg {
            900
        } else {
            size
        };
        ranked.push((rank, href));
    }
    ranked.sort_by_key(|(rank, _)| std::cmp::Reverse(*rank));
    let mut candidates: Vec<String> = ranked.into_iter().map(|(_, href)| href).collect();
    for usual in ["/apple-touch-icon.png", "/favicon.ico"] {
        if let Some(url) = absolute(base, usual) {
            candidates.push(url);
        }
    }
    candidates.dedup();
    candidates
}

/// The site's icon at the icon theme's place: the one in the cache, or its
/// best icon of at least 48 pixels, or else a round letter
fn install_icon(
    id: &str,
    name: &str,
    url: &str,
    html: Option<&str>,
    base: &str,
) -> std::io::Result<()> {
    let [png, svg] = icon_paths(id);
    for path in [&png, &svg] {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let _ = std::fs::remove_file(path);
    }
    if let Some(cached) = cached_icon(url)
        && frame_file(&cached, &png, &svg).is_ok()
    {
        return Ok(());
    }
    if site_icon(&png, &svg, html, url, base)? {
        return Ok(());
    }
    std::fs::write(&svg, letter_icon(name))
}

/// Writes the site's best icon of at least 48 pixels (or 32 when it has no
/// larger one) to `png` or `svg`; false when it has none. Its page's icons
/// first, then the usual icons at the address asked (a site that sends
/// programs to a sign-in page, or that only answers browsers, has its
/// icons there).
fn site_icon(
    png: &Path,
    svg: &Path,
    html: Option<&str>,
    url: &str,
    base: &str,
) -> std::io::Result<bool> {
    let mut candidates: Vec<String> = icon_candidates(html, base).into_iter().take(6).collect();
    for usual in ["/apple-touch-icon.png", "/favicon.ico"] {
        if let Some(usual) = absolute(url, usual)
            && !candidates.contains(&usual)
        {
            candidates.push(usual);
        }
    }
    let mut small: Option<image::DynamicImage> = None;
    for candidate in &candidates {
        let Some(bytes) = download(candidate) else {
            continue;
        };
        let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_ascii_lowercase();
        if head.contains("<svg") {
            std::fs::write(svg, framed_svg(&String::from_utf8_lossy(&bytes)))?;
            return Ok(true);
        }
        let Ok(picture) = image::load_from_memory(&bytes) else {
            continue;
        };
        if picture.width().min(picture.height()) >= 48 {
            save_png(&picture, png)?;
            return Ok(true);
        }
        if small
            .as_ref()
            .is_none_or(|best| best.width() < picture.width())
        {
            small = Some(picture);
        }
    }
    match small {
        Some(picture) if picture.width() >= 32 => save_png(&picture, png).map(|()| true),
        _ => Ok(false),
    }
}

fn save_png(picture: &image::DynamicImage, path: &Path) -> std::io::Result<()> {
    framed(picture)
        .save_with_format(path, image::ImageFormat::Png)
        .map_err(std::io::Error::other)
}

/// An icon file framed (PNG or SVG) into `png` or `svg`
fn frame_file(icon: &Path, png: &Path, svg: &Path) -> std::io::Result<()> {
    if icon.extension().is_some_and(|ext| ext == "svg") {
        let text = std::fs::read_to_string(icon)?;
        std::fs::write(svg, framed_svg(&text))
    } else {
        let picture = image::open(icon).map_err(std::io::Error::other)?;
        save_png(&picture, png)
    }
}

/// A 256 pixels icon with the system icons' margin: the picture without its
/// empty border, at ICON_FILL of the square; a picture that fills its whole
/// square (no transparent corner) gets rounded corners. Framing twice
/// gives the same icon.
fn framed(picture: &image::DynamicImage) -> image::RgbaImage {
    use image::GenericImageView;
    const SIZE: u32 = 256;
    let rgba = picture.to_rgba8();
    let mut bounds: Option<(u32, u32, u32, u32)> = None;
    for (x, y, pixel) in rgba.enumerate_pixels() {
        if pixel[3] > 16 {
            bounds = Some(bounds.map_or((x, y, x, y), |(x0, y0, x1, y1)| {
                (x0.min(x), y0.min(y), x1.max(x), y1.max(y))
            }));
        }
    }
    let Some((x0, y0, x1, y1)) = bounds else {
        return image::RgbaImage::new(SIZE, SIZE);
    };
    let content = picture.view(x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image();
    let (cw, ch) = content.dimensions();
    let opaque_corners = [(0, 0), (cw - 1, 0), (0, ch - 1), (cw - 1, ch - 1)]
        .iter()
        .all(|&(x, y)| content.get_pixel(x, y)[3] > 200);
    let side = (SIZE as f32 * ICON_FILL).round();
    let scale = side / cw.max(ch) as f32;
    let (w, h) = (
        ((cw as f32 * scale).round() as u32).max(1),
        ((ch as f32 * scale).round() as u32).max(1),
    );
    let mut picture =
        image::imageops::resize(&content, w, h, image::imageops::FilterType::Lanczos3);
    if opaque_corners {
        round_corners(&mut picture, w.min(h) as f32 * 0.22);
    }
    let mut icon = image::RgbaImage::new(SIZE, SIZE);
    image::imageops::overlay(
        &mut icon,
        &picture,
        i64::from((SIZE - w) / 2),
        i64::from((SIZE - h) / 2),
    );
    icon
}

/// Makes the corners transparent, with a soft edge
fn round_corners(picture: &mut image::RgbaImage, radius: f32) {
    let (w, h) = (picture.width() as f32, picture.height() as f32);
    for (x, y, pixel) in picture.enumerate_pixels_mut() {
        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
        let cx = px.clamp(radius, w - radius);
        let cy = py.clamp(radius, h - radius);
        let distance = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
        let coverage = (radius + 0.5 - distance).clamp(0.0, 1.0);
        pixel[3] = (f32::from(pixel[3]) * coverage).round() as u8;
    }
}

/// An SVG icon inside a square with the system icons' margin (its own
/// root element placed in the middle, its picture unchanged)
pub fn framed_svg(text: &str) -> String {
    const MARK: &str = "<!-- framed by Celenn OS -->";
    if text.contains(MARK) {
        return text.to_owned();
    }
    let lower = text.to_ascii_lowercase();
    let Some(start) = lower.find("<svg") else {
        return text.to_owned();
    };
    let Some(end) = text[start..].find('>').map(|end| start + end) else {
        return text.to_owned();
    };
    let tag = &text[start..end];
    let (tag, closed) = match tag.strip_suffix('/') {
        Some(tag) => (tag, "/>"),
        None => (tag, ">"),
    };
    let number = |name: &str| {
        attribute(tag, name).and_then(|value| {
            value
                .trim()
                .trim_end_matches("px")
                .parse::<f32>()
                .ok()
                .filter(|value| *value > 0.0)
        })
    };
    let view_box = attribute(tag, "viewbox")
        .map(str::to_owned)
        .or_else(|| Some(format!("0 0 {} {}", number("width")?, number("height")?)));
    let Some(view_box) = view_box else {
        return text.to_owned();
    };
    let mut root = tag.to_owned();
    for name in ["width", "height", "x", "y", "viewBox"] {
        root = without_attribute(&root, name);
    }
    let margin = (1.0 - ICON_FILL) / 2.0 * 100.0;
    let side = ICON_FILL * 100.0;
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\">{MARK}\
         {root} x=\"{margin}\" y=\"{margin}\" width=\"{side}\" height=\"{side}\" \
         viewBox=\"{view_box}\"{closed}{rest}</svg>\n",
        view_box = view_box.replace('"', ""),
        rest = &text[end + 1..],
    )
}

/// A tag without one of its attributes (`name="..."` or `name='...'`)
fn without_attribute(tag: &str, name: &str) -> String {
    let mut out = String::with_capacity(tag.len());
    let mut rest = tag;
    while let Some(found) = rest.find(name) {
        let before = &rest[..found];
        let after = rest[found + name.len()..].trim_start();
        let space_before = before.ends_with(char::is_whitespace);
        if let (true, Some(after)) = (space_before, after.strip_prefix('=')) {
            let after = after.trim_start();
            let quote = after.chars().next().filter(|c| *c == '"' || *c == '\'');
            if let Some(quote) = quote
                && let Some(close) = after[1..].find(quote)
            {
                out.push_str(before.trim_end());
                rest = &after[close + 2..];
                continue;
            }
        }
        out.push_str(&rest[..found + name.len()]);
        rest = &rest[found + name.len()..];
    }
    out.push_str(rest);
    out
}

/// A round icon with the name's first letter, in Celenn OS's blue
pub fn letter_icon(name: &str) -> String {
    let letter = name
        .chars()
        .find(|c| c.is_alphanumeric())
        .map_or('?', |c| c.to_uppercase().next().unwrap_or(c));
    let letter = match letter {
        '&' => "&amp;".to_owned(),
        '<' => "&lt;".to_owned(),
        c => c.to_string(),
    };
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 64\">\
         <circle cx=\"32\" cy=\"32\" r=\"26\" fill=\"#4f8dff\"/>\
         <text x=\"32\" y=\"41\" font-family=\"Open Sans, sans-serif\" font-size=\"26\" \
         font-weight=\"600\" text-anchor=\"middle\" fill=\"#ffffff\">{letter}</text></svg>\n"
    )
}

/// Pins a launcher to the dock, or unpins it (the dock's favorites)
fn pin(id: &str, on: bool) {
    let file = config_home().join("cosmic/com.system76.CosmicAppList/v1/favorites");
    let current = std::fs::read_to_string(&file).or_else(|_| {
        std::fs::read_to_string("/usr/share/cosmic/com.system76.CosmicAppList/v1/favorites")
    });
    let mut favorites: Vec<String> = current
        .unwrap_or_default()
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect();
    let present = favorites.iter().any(|favorite| favorite == id);
    if on == present {
        return;
    }
    if on {
        favorites.push(id.to_owned());
    } else {
        favorites.retain(|favorite| favorite != id);
    }
    let body: String = favorites
        .iter()
        .map(|favorite| format!("    \"{favorite}\",\n"))
        .collect();
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&file, format!("[\n{body}]"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_and_removes_the_folder() {
        // (as in system_files/usr/share/cosmic/com.system76.CosmicAppLibrary)
        let system = "[\n    (\n        name: \"Celenn OS\",\n        icon: \"celenn-logo-symbolic\",\n        \
                      filter: AppIds([\"io.github.celenn_os.Tools\", \"documentation\"]),\n    ),\n    \
                      (\n        name: \"cosmic-office\",\n        icon: \"folder-symbolic\",\n        \
                      filter: Categories(\n            categories: [\"Office\"],\n            \
                      exclude: [],\n            include: [\"thunderbird\"],\n        ),\n    ),\n]\n"
            .to_owned();
        let with = with_folder(&system, true, "Applis web").unwrap();
        assert!(with.contains("name: \"Applis web\""));
        assert!(with.contains("categories: [\"X-Celenn-WebApp\"]"));
        assert_eq!(
            ron_items(&with).unwrap().len(),
            ron_items(&system).unwrap().len() + 1
        );
        assert_eq!(with_folder(&with, true, "Web apps").unwrap(), with);
        let without = with_folder(&with, false, "Web apps").unwrap();
        assert_eq!(ron_items(&without), ron_items(&system));
        assert_eq!(with_folder(&system, false, "Web apps").unwrap(), system);
        assert_eq!(with_folder("not a list", true, "Web apps"), None);
        assert_eq!(ron_items("[(a: \"),(\"), (b: [1, 2])]").unwrap().len(), 2);
    }

    #[test]
    fn frames_svg_icons_once() {
        let icon = "<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\" \
                    width=\"48px\" height='48' fill=\"red\"><rect width=\"48\" height=\"48\"/></svg>";
        let framed = framed_svg(icon);
        assert!(framed.contains("viewBox=\"0 0 48 48\""), "{framed}");
        assert!(framed.contains("fill=\"red\""));
        assert!(framed.contains("width=\"82\""));
        assert!(!framed.contains("48px"));
        assert!(framed.contains("<rect width=\"48\" height=\"48\"/>"));
        assert_eq!(framed_svg(&framed), framed);
    }

    #[test]
    fn frames_full_square_icons() {
        let square = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            180,
            180,
            image::Rgba([30, 200, 90, 255]),
        ));
        let icon = framed(&square);
        assert_eq!(icon.dimensions(), (256, 256));
        // margin all around, rounded corners, the middle painted
        assert_eq!(icon.get_pixel(10, 128)[3], 0);
        assert_eq!(icon.get_pixel(25, 25)[3], 0);
        assert_eq!(icon.get_pixel(128, 128)[3], 255);
        let again = framed(&image::DynamicImage::ImageRgba8(icon.clone()));
        assert_eq!(again.get_pixel(25, 25)[3], 0);
        assert_eq!(again.get_pixel(30, 128)[3], icon.get_pixel(30, 128)[3]);
    }

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
        for (_, url, _) in SITES {
            assert!(normalize(url).is_some(), "{url}");
        }
    }

    #[test]
    fn reads_pages() {
        let html = r#"<html><head><title>La Poste · Colis &amp; courrier</title>
            <link rel="icon" href="/favicon-32.png" sizes="32x32">
            <LINK REL="apple-touch-icon" HREF="//cdn.laposte.fr/touch.png">
            <link rel="icon" type="image/svg+xml" href="img/logo.svg"></head></html>"#;
        assert_eq!(site_name(html).as_deref(), Some("La Poste"));
        let icons = icon_candidates(Some(html), "https://www.laposte.fr/accueil/index.html");
        assert_eq!(icons[0], "https://cdn.laposte.fr/touch.png");
        assert_eq!(icons[1], "https://www.laposte.fr/accueil/img/logo.svg");
        assert_eq!(icons[2], "https://www.laposte.fr/favicon-32.png");
        assert!(icons.contains(&"https://www.laposte.fr/favicon.ico".to_owned()));
        let meta = r#"<meta property="og:site_name" content="Deezer"><title>x</title>"#;
        assert_eq!(site_name(meta).as_deref(), Some("Deezer"));
        assert_eq!(name_from_host("laposte.fr"), "Laposte");
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
        assert!(letter_icon("la poste").contains(">L</text>"));
    }
}
