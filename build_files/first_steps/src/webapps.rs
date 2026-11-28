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
//       letter when the site has none
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
/// The launcher's key that keeps the web app's address
const URL_KEY: &str = "X-CosmicBlue-WebApp-Url";
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
        let cosmic_theme::Spacing {
            space_xs, space_s, ..
        } = theme::spacing();
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
            widget::column::with_capacity(tiles.len().div_ceil(PER_LINE)).spacing(space_xs);
        let mut tiles = tiles.into_iter().peekable();
        while tiles.peek().is_some() {
            let mut line = widget::row::with_capacity(PER_LINE).spacing(space_xs);
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
        .height(Length::Fixed(44.0))
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
        .join("celenn/web-app-icons");
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
/// in the last week
fn icon_missing(url: &str) -> bool {
    let Some([_, _, none]) = cache_paths(url) else {
        return false;
    };
    let asked_lately = std::fs::metadata(&none)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.elapsed().ok())
        .is_some_and(|age| age.as_secs() < 7 * 24 * 3600);
    cached_icon(url).is_none() && !asked_lately
}

fn site_icons() -> Vec<Option<PathBuf>> {
    SITES.iter().map(|(_, url, _)| cached_icon(url)).collect()
}

/// Fetches the popular sites' missing icons, all at once
fn cache_icons() {
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
            &base
        ),
        Ok(true)
    );
    let _ = std::fs::rename(&png_part, &png);
    let _ = std::fs::rename(&svg_part, &svg);
    if found {
        let _ = std::fs::remove_file(&none);
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
                url: value(URL_KEY)?,
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
         Categories=Network;\n\
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
            "--user-agent",
            "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0",
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
    if let Some(cached) = cached_icon(url) {
        let target = if cached.extension().is_some_and(|ext| ext == "svg") {
            &svg
        } else {
            &png
        };
        if std::fs::copy(&cached, target).is_ok() {
            return Ok(());
        }
    }
    if site_icon(&png, &svg, html, base)? {
        return Ok(());
    }
    std::fs::write(&svg, letter_icon(name))
}

/// Writes the site's best icon of at least 48 pixels (or 32 when it has no
/// larger one) to `png` or `svg`; false when it has none
fn site_icon(png: &Path, svg: &Path, html: Option<&str>, base: &str) -> std::io::Result<bool> {
    let mut small: Option<image::DynamicImage> = None;
    for candidate in icon_candidates(html, base).iter().take(6) {
        let Some(bytes) = download(candidate) else {
            continue;
        };
        let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_ascii_lowercase();
        if head.contains("<svg") {
            std::fs::write(svg, &bytes)?;
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
    let picture = picture.resize(256, 256, image::imageops::FilterType::Lanczos3);
    picture
        .save_with_format(path, image::ImageFormat::Png)
        .map_err(std::io::Error::other)
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
         <circle cx=\"32\" cy=\"32\" r=\"30\" fill=\"#4f8dff\"/>\
         <text x=\"32\" y=\"43\" font-family=\"Open Sans, sans-serif\" font-size=\"30\" \
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
