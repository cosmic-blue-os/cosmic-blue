// Web apps, a tool of Celenn OS Tools (tools.rs): a site in its own
// window, with its icon in the app list and in the dock, as on a phone.
// This file is the page; the rest is in four modules:
//   webapp_launcher.rs  ids, launcher, Firefox profile, dock pin
//   webapp_folder.rs    the "Web apps" folder of the app list
//   webapp_icons.rs     icons: fetched, framed, or a round letter
//   webapp_site.rs      reading a site: its page, name and icons
// Firefox's system policies (/usr/share/celenn/firefox: uBlock Origin,
// no telemetry, HTTPS-Only...) apply to every profile, and so do the
// parental controls (the DNS filter, Firefox allowed or not for a child).
// People sign in once in each web app: each has its own cookies.
// "Also add to the dock" (off by default) pins it to the dock's favorites.

use crate::common::{ToastAsk, dim_caption, label, row_title, toast, tool_icon};
use crate::i18n::{fill, tr};
use crate::webapp_folder::update_folder;
use crate::webapp_icons::{cache_icons, frame_file, icon_missing, install_icon, site_icons};
use crate::webapp_launcher::{
    CATEGORY, WebApp, app_id, applications, desktop_entry, firefox_installed, host, icon_file,
    icon_paths, installed, launcher, normalize, pin, profile, valid_id, write_profile,
};
use crate::webapp_site::{fetch, name_from_host, site_name};
use cosmic::app::Task;
use cosmic::iced::core::text::{Ellipsize, EllipsizeHeightLimit, Wrapping};
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, settings};
use cosmic::{Apply, Element, cosmic_theme, theme};
use std::path::PathBuf;

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
        // (the image's screenshot test asks for no fetching: the tiles then
        // show the web symbol, whatever the sites answer)
        if self.caching
            || crate::common::preview("offline")
            || !SITES.iter().any(|(_, url, _)| icon_missing(url))
        {
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
        .arg(applications())
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
    // (an id names folders that are deleted: only this tool's own)
    if !valid_id(id) {
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

/// Web apps made by an older version: launcher with the folder's category,
/// framed icon
fn update_older() {
    let mut changed = false;
    for app in installed() {
        let file = launcher(&app.id);
        let Ok(content) = std::fs::read_to_string(&file) else {
            continue;
        };
        if content.contains(CATEGORY) {
            continue;
        }
        if let Some(icon) = icon_file(&app.id) {
            let [png, svg] = icon_paths(&app.id);
            let _ = frame_file(&icon, &png, &svg);
        }
        let entry = desktop_entry(&app.id, &app.name, &app.url, &profile(&app.id));
        changed |= std::fs::write(&file, entry).is_ok();
    }
    if changed {
        let _ = std::process::Command::new("update-desktop-database")
            .arg(applications())
            .status();
    }
    update_folder();
}
