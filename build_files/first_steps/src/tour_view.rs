// The first steps tour: what its pages and dialogs show (App::view and
// the widgets it is made of)

use super::*;

/// A file name short enough for a title: long ones keep their start and
/// their end ("google-chrome-st…amd64.deb")
pub(super) fn short_name(file: &Path) -> String {
    let name = file.file_name().unwrap_or_default().to_string_lossy();
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= 32 {
        return name.into_owned();
    }
    let start: String = chars[..18].iter().collect();
    let end: String = chars[chars.len() - 12..].iter().collect();
    format!("{start}…{end}")
}

pub(super) fn accessibility_row<'a>(space_s: u16) -> Element<'a, Message> {
    // Accessibility, from the first page: what COSMIC's first-run assistant
    // (skipped, see build_files/20-cosmic.sh) offered
    settings::item_row(vec![
        widget::icon::from_name("celenn-tour-accessibility")
            .size(40)
            .into(),
        two_lines(text::heading(tr("Accessibility")).into(), tr(ACCESSIBILITY)),
        action_button(
            "Settings",
            &Action::Launch(&["cosmic-settings", "accessibility"]),
        ),
    ])
    .spacing(space_s)
    .into()
}

/// A state symbol in front of a row: done (accent color) or to look at
/// A row's symbolic icon, the size of state_icon(), so that the texts of
/// all rows start at the same place
pub(super) fn row_icon<'a>(name: &'static str) -> Element<'a, Message> {
    widget::icon::from_name(name).size(20).into()
}

pub(super) fn state_icon<'a>(done: bool) -> Element<'a, Message> {
    if !done {
        // To do: an empty circle, which the check mark replaces
        return widget::container(widget::space::horizontal().width(Length::Fixed(0.0)))
            .width(20)
            .height(20)
            .class(theme::Container::custom(|theme| {
                let mut color = theme.cosmic().on_bg_color();
                color.alpha = 0.5;
                cosmic::iced::widget::container::Style {
                    border: cosmic::iced::Border {
                        radius: 10.0.into(),
                        width: 2.0,
                        color: color.into(),
                    },
                    ..Default::default()
                }
            }))
            .into();
    }
    widget::icon::from_name("object-select-symbolic")
        .size(20)
        .apply(widget::container)
        .class(theme::Container::custom(|theme| {
            cosmic::iced::widget::container::Style {
                icon_color: Some(theme.cosmic().accent_color().into()),
                ..Default::default()
            }
        }))
        .into()
}

/// A hardware check's symbol: a check mark in the accent color (all
/// right), or information, warning or error symbols in the theme's colors
pub(super) fn status_icon<'a>(status: readiness::Status) -> Element<'a, Message> {
    use readiness::Status;
    let name = match status {
        Status::Good => return state_icon(true),
        Status::Info => "dialog-information-symbolic",
        Status::Warn => "dialog-warning-symbolic",
        Status::Bad => "dialog-error-symbolic",
    };
    widget::icon::from_name(name)
        .size(20)
        .apply(widget::container)
        .class(theme::Container::custom(move |theme| {
            let cosmic = theme.cosmic();
            let color = match status {
                Status::Warn => cosmic.warning_color(),
                Status::Bad => cosmic.destructive_color(),
                _ => cosmic.on_bg_color(),
            };
            cosmic::iced::widget::container::Style {
                icon_color: Some(color.into()),
                ..Default::default()
            }
        }))
        .into()
}

/// The command that plays a short test sound, when the session has one
pub(super) fn test_sound() -> Option<&'static [&'static str]> {
    const SOUNDS: [&[&str]; 2] = [
        &[
            "pw-play",
            "/usr/share/sounds/freedesktop/stereo/audio-test-signal.oga",
        ],
        &["pw-play", "/usr/share/sounds/freedesktop/stereo/bell.oga"],
    ];
    SOUNDS
        .into_iter()
        .find(|command| in_path(command[0]) && Path::new(command[1]).is_file())
}

pub(super) fn heading<'a>(label: &'static str) -> Element<'a, Message> {
    text::heading(tr(label)).width(Length::Fill).into()
}

impl<T: Tour> App<T> {
    pub(super) fn body<'a>(&'a self, body: &'a Body, space_s: u16) -> Element<'a, Message> {
        let rows: Vec<Element<'a, Message>> = match body {
            Body::Welcome => {
                let illustration: Element<'a, Message> = match &self.welcome {
                    // HERO is 600 x 220 (the logo is square: it keeps its ratio)
                    Some(handle) => widget::svg(handle.clone()).width(600).height(220).into(),
                    None => widget::icon::from_name("computer-symbolic")
                        .size(128)
                        .into(),
                };
                return widget::column::with_capacity(2)
                    .spacing(theme::spacing().space_m)
                    .push(
                        widget::container(illustration)
                            .padding([space_s, 0])
                            .center_x(Length::Fill),
                    )
                    .push(settings::section().add(accessibility_row(space_s)))
                    .into();
            }
            Body::Origin => return self.origin_view(space_s),
            Body::Look => return self.look_view(space_s),
            Body::Profiles => {
                let body = match self.sub {
                    Some(n) => self.category_list(n, space_s),
                    None => {
                        let tiles = (0..CATEGORIES.len()).map(|n| self.tile(n)).collect();
                        let mut body = vec![apps::grid(tiles)];
                        body.extend(self.install_status(space_s));
                        body
                    }
                };
                return widget::column::with_children(body).spacing(space_s).into();
            }
            Body::Essentials(rows) => {
                let body = match self.sub {
                    Some(n) => self.category_list(n, space_s),
                    None => {
                        let rows: Vec<Element<'a, Message>> =
                            rows.iter().map(|row| row_view(row, space_s)).collect();
                        let tiles = vec![self.tile(apps::PASSWORDS), self.tile(DEVELOPMENT)];
                        let mut body = vec![section(rows), apps::grid(tiles)];
                        body.extend(self.install_status(space_s));
                        body
                    }
                };
                return widget::column::with_children(body).spacing(space_s).into();
            }
            Body::Try => Try::ALL
                .iter()
                .map(|thing| {
                    let (title, caption) = thing.texts();
                    let label = match thing {
                        Try::Tiling if self.tiling => "Turn off",
                        Try::Tiling => "Turn on",
                        _ => "Try",
                    };
                    let available = thing.program().is_none_or(in_path);
                    let button = widget::button::standard(tr(label))
                        .on_press_maybe(available.then_some(Message::Try(*thing)));
                    settings::item_row(vec![
                        state_icon(self.tried.contains(thing)),
                        two_lines(text::heading(tr(title)).into(), tr(caption)),
                        button.into(),
                    ])
                    .spacing(space_s)
                    .into()
                })
                .collect(),
            Body::Ready => return self.ready_view(space_s),
            Body::Readiness => return self.readiness_view(space_s),
            Body::Install => return self.install_view(space_s),
            Body::Features(rows) => rows
                .iter()
                .map(|(title, subtitle)| {
                    settings::item_row(vec![two_lines(
                        text::heading(tr(title)).into(),
                        tr(subtitle),
                    )])
                    .into()
                })
                .collect(),
            Body::Rows(rows) => rows.iter().map(|row| row_view(row, space_s)).collect(),
        };
        section(rows)
    }

    pub(super) fn origin_view<'a>(&'a self, space_s: u16) -> Element<'a, Message> {
        let cards = Origin::ALL.iter().fold(
            widget::row::with_capacity(3).spacing(space_s).height(128),
            |row, origin| {
                let (icon, name, caption) = origin.card();
                let card = widget::column::with_capacity(3)
                    .spacing(4)
                    .align_x(Alignment::Center)
                    .push(widget::icon::from_name(icon).size(32))
                    .push(text::heading(name))
                    .push(
                        text::caption(tr(caption))
                            .align_x(Alignment::Center)
                            .wrapping(Wrapping::Word),
                    )
                    .width(Length::Fill)
                    .padding(space_s);
                row.push(
                    widget::button::custom(card)
                        .class(card_style(self.origin == Some(*origin)))
                        .height(Length::Fill)
                        .width(Length::Fill)
                        .on_press(Message::Origin(*origin)),
                )
            },
        );
        // Those who come from Linux: new to it, or the short tour
        let level = (self.origin == Some(Origin::Linux)).then(|| {
            let buttons = [(false, "I am new to it"), (true, "I know it well")]
                .into_iter()
                .fold(
                    widget::row::with_capacity(2).spacing(space_s),
                    |row, (experienced, label)| {
                        let button = if self.experienced == experienced {
                            widget::button::suggested(tr(label))
                        } else {
                            widget::button::standard(tr(label))
                        };
                        row.push(button.on_press(Message::Level(experienced)))
                    },
                );
            settings::section().add(
                settings::item_row(vec![
                    two_lines(
                        text::heading(tr("Linux and you")).into(),
                        tr("I know it well: a shorter tour, in three steps."),
                    ),
                    buttons.into(),
                ])
                .spacing(space_s),
            )
        });
        let scale = settings::item_row(vec![
            widget::icon::from_name("celenn-tour-computer")
                .size(40)
                .into(),
            two_lines(
                text::heading(tr("Screen scale")).into(),
                tr("Text and windows too small or too big? Change the scale of the screen."),
            ),
            action_button(
                "Display settings",
                &Action::Launch(&["cosmic-settings", "displays"]),
            ),
        ])
        .spacing(space_s);
        let mut column = widget::column::with_capacity(4)
            .spacing(theme::spacing().space_m)
            .push(cards)
            .push_maybe(level)
            .push(
                settings::section()
                    .add(accessibility_row(space_s))
                    .add(scale),
            );
        if let Some((done, all)) = self.preinstall_progress() {
            column = column.push(install_progress(
                fill(
                    &fill(tr("Installing your apps: {} of {}"), &done.to_string()),
                    &all.to_string(),
                ),
                tr("They show up in the app library as they arrive."),
                done,
                all,
                space_s,
            ));
        }
        column.into()
    }

    pub(super) fn look_view<'a>(&'a self, space_s: u16) -> Element<'a, Message> {
        let theme_buttons = [(true, "Dark"), (false, "Light")].into_iter().fold(
            widget::row::with_capacity(2).spacing(space_s),
            |row, (dark, label)| {
                let button = if self.dark == dark {
                    widget::button::suggested(tr(label))
                } else {
                    widget::button::standard(tr(label))
                };
                row.push(button.on_press(Message::Dark(dark)))
            },
        );
        // Top bar and dock (Celenn OS's own) or one bar at the bottom
        let layout_buttons = [(false, "Top bar and dock"), (true, "Bar at the bottom")]
            .into_iter()
            .fold(
                widget::row::with_capacity(2).spacing(space_s),
                |row, (bottom, label)| {
                    let button = if self.bottom_bar == bottom {
                        widget::button::suggested(tr(label))
                    } else {
                        widget::button::standard(tr(label))
                    };
                    row.push(button.on_press(Message::Layout(bottom)))
                },
            );
        let dock = settings::item_row(vec![
            two_lines(
                text::heading(tr("Layout")).into(),
                tr("Top bar and dock: like the Mac. One bar at the bottom: like Windows."),
            ),
            layout_buttons.into(),
        ])
        .spacing(space_s);
        let mut column = widget::column::with_capacity(5).spacing(space_s).push(
            settings::section()
                .add(settings::item_row(vec![
                    text::body(tr("Theme")).width(Length::Fill).into(),
                    theme_buttons.into(),
                ]))
                .add(dock),
        );
        if !self.wallpapers.is_empty() {
            let thumbs: Vec<Element<'a, Message>> = self
                .wallpapers
                .iter()
                .enumerate()
                .map(|(i, (path, handle))| {
                    widget::button::image(handle.clone())
                        // For screen readers: "Wallpaper: arcs"
                        .name(format!("{}: {}", tr("Wallpaper"), style::image_name(path)))
                        .width(92)
                        .height(52)
                        .selected(self.wallpaper.as_deref() == Some(path.as_path()))
                        .on_press(Message::Wallpaper(i))
                        .into()
                })
                .collect();
            column = column.push(heading("Wallpaper")).push(
                widget::flex_row(thumbs)
                    .row_spacing(space_s)
                    .column_spacing(space_s),
            );
        }
        if !self.avatars.is_empty() {
            let faces: Vec<Element<'a, Message>> = self
                .avatars
                .iter()
                .enumerate()
                .map(|(i, (path, handle))| {
                    // 11 per row, as wide as the 6 wallpapers above
                    // (6 x 92 + 5 gaps = 11 x 43 + 10 gaps, to a pixel),
                    // rounded squares outlined like the wallpapers
                    style::picture_button(
                        handle.clone(),
                        43,
                        self.avatar == Some(i),
                        Message::Avatar(i),
                    )
                    .name(format!(
                        "{}: {}",
                        tr("Your picture"),
                        style::image_name(path)
                    ))
                    .into()
                })
                .collect();
            column = column.push(heading("Your picture")).push(
                widget::flex_row(faces)
                    .row_spacing(space_s)
                    .column_spacing(space_s),
            );
        }
        column.into()
    }

    /// The progress of the chosen apps, or the card of an install that
    /// did not work
    pub(super) fn install_status<'a>(&'a self, space_s: u16) -> Option<Element<'a, Message>> {
        if let Some((done, all)) = self.install_count() {
            Some(install_progress(
                fill(
                    &fill(tr("Installing: {} of {}"), &done.to_string()),
                    &all.to_string(),
                ),
                tr("They show up in the app library as they arrive."),
                done,
                all,
                space_s,
            ))
        } else {
            self.install_card
                .as_ref()
                .map(|card| card.view(Message::InstallCard))
        }
    }

    /// A category's tile: what is chosen or installed in it, and the
    /// button that opens its list
    pub(super) fn tile<'a>(&'a self, n: usize) -> Element<'a, Message> {
        let running = matches!(self.install, Install::Running { .. });
        apps::tile(
            &CATEGORIES[n],
            apps::tile_caption(&CATEGORIES[n], &self.picks[n], &self.category_installed(n)),
            (!running).then_some(Message::Sub(Some(n))),
        )
    }

    /// A category's apps by group, each with its checkbox (the installed
    /// ones ticked and grey: the tour only installs), what to know, and
    /// the way back to the tiles
    pub(super) fn category_list<'a>(&'a self, n: usize, space_s: u16) -> Vec<Element<'a, Message>> {
        let category = &CATEGORIES[n];
        let installed = self.category_installed(n);
        let mut ticked = installed.clone();
        ticked.extend(self.picks[n].iter().copied());
        let rows = apps::checklist(
            category,
            &ticked,
            |i, tool| !installed.contains(&i) && apps::available(tool),
            move |i, on| Message::Pick(n, i, on),
        );
        let brew = Path::new(devtools::BREW).exists();
        let mut notes: Vec<&str> = Vec::new();
        if !installed.is_empty() {
            notes.push(tr("Ticked and grey: already installed."));
        }
        if n == DEVELOPMENT && !brew {
            notes.push(tr(
                "Homebrew is still being set up: the code editors and terminal tools can be chosen in a few minutes.",
            ));
        } else if !category.note.is_empty() {
            notes.push(tr(category.note));
        }
        let mut body = vec![
            widget::column::with_capacity(2)
                .spacing(space_s)
                .push(text::heading(tr(category.title)))
                .push(section(rows))
                .into(),
        ];
        if !notes.is_empty() {
            body.push(
                text::caption(notes.join(" "))
                    .wrapping(Wrapping::Word)
                    .align_x(Alignment::Center)
                    .width(Length::Fill)
                    .into(),
            );
        }
        body.push(back_to_list());
        body
    }

    /// What this computer needs, found by system::computer(): only what
    /// asks for an action (Celenn OS Tools has the rest)
    pub(super) fn computer_rows<'a>(&'a self, space_s: u16) -> Vec<Element<'a, Message>> {
        let Some(computer) = &self.computer else {
            return Vec::new();
        };
        let item =
            |done: bool, title: String, caption: String, end: Option<Element<'a, Message>>| {
                let mut text_column = widget::column::with_capacity(2)
                    .spacing(2)
                    .push(text::heading(title))
                    .width(Length::Fill);
                if !caption.is_empty() {
                    text_column = text_column.push(dim_caption(caption).wrapping(Wrapping::Word));
                }
                let mut items = vec![state_icon(done), text_column.into()];
                items.extend(end);
                Element::from(settings::item_row(items).spacing(space_s))
            };
        let mut rows = Vec::new();
        if computer.online == Some(false) {
            rows.push(item(
                false,
                tr("No internet connection").to_owned(),
                tr("Apps and updates need it.").to_owned(),
                Some(action_button(
                    "Network settings",
                    &Action::Launch(&["cosmic-settings", "network"]),
                )),
            ));
        }
        match (computer.nvidia, &self.nvidia) {
            (Some(false), Nvidia::Switched) => {
                let mut caption = tr("The NVIDIA edition starts at the next restart.").to_owned();
                if computer.secure_boot {
                    caption.push(' ');
                    caption.push_str(tr("At the restart, a blue screen asks to enroll a key: choose Enroll MOK, Continue, Yes, then type the password celennos (the keyboard is QWERTY there)."));
                }
                rows.push(item(
                    true,
                    tr("NVIDIA graphics card").to_owned(),
                    caption,
                    None,
                ));
            }
            (Some(false), state) => {
                let switching = *state == Nvidia::Switching;
                rows.push(item(
                    false,
                    tr("NVIDIA graphics card").to_owned(),
                    tr(if switching {
                        "Downloading the NVIDIA edition… This can take a while."
                    } else {
                        "The NVIDIA edition of Celenn OS has its driver: about 4 GB to download, then it starts at the next restart."
                    })
                    .to_owned(),
                    Some(
                        widget::button::suggested(tr("Switch"))
                            .on_press_maybe(
                                (!switching && Path::new(system::UPDATES_HELPER).exists())
                                    .then_some(Message::SwitchNvidia),
                            )
                            .into(),
                    ),
                ));
            }
            _ => {}
        }
        if let Some(devices) = &computer.firmware
            && !devices.is_empty()
        {
            rows.push(item(
                false,
                tr("Firmware updates are available").to_owned(),
                devices.join(", "),
                Some(action_button("Open Firmware", &Action::Launch(FIRMWARE))),
            ));
        }
        if let Some(on) = computer.battery_limit {
            rows.push(
                settings::item_row(vec![
                    row_icon("battery-good-symbolic"),
                    two_lines(
                        text::heading(tr("Take care of the battery")).into(),
                        tr("Stop charging at 80 % when the laptop stays plugged in: the battery lasts longer."),
                    ),
                    widget::toggler(on).on_toggle(Message::Battery).into(),
                ])
                .spacing(space_s)
                .into(),
            );
        }
        rows
    }

    /// Live session: what the hardware checks found (readiness.rs)
    pub(super) fn readiness_view<'a>(&'a self, space_s: u16) -> Element<'a, Message> {
        let Some(checks) = &self.readiness else {
            return settings::section()
                .add(
                    settings::item_row(vec![
                        widget::progress_bar::indeterminate_circular()
                            .size(20.0)
                            .into(),
                        text::body(tr("Checking this computer…")).into(),
                    ])
                    .spacing(space_s),
                )
                .into();
        };
        let rows = checks
            .iter()
            .map(|check| {
                let mut items = vec![
                    status_icon(check.status),
                    widget::column::with_capacity(2)
                        .spacing(2)
                        .push(text::heading(tr(check.title)))
                        .push(dim_caption(check.caption.as_str()).wrapping(Wrapping::Word))
                        .width(Length::Fill)
                        .into(),
                ];
                match check.help {
                    Some(readiness::Help::TestSound) => {
                        if let Some(sound) = test_sound() {
                            items.push(
                                widget::button::standard(tr("Test the sound"))
                                    .on_press(Message::Run(Action::Launch(sound)))
                                    .into(),
                            );
                        }
                    }
                    Some(readiness::Help::Disk) => items.push(
                        widget::button::standard(tr("How to do it"))
                            .trailing_icon(widget::button::link::icon())
                            .on_press(Message::Run(Action::OpenUri(install_url("disk"))))
                            .into(),
                    ),
                    None => {}
                }
                settings::item_row(items).spacing(space_s).into()
            })
            .collect();
        let footer = widget::row::with_capacity(2)
            .spacing(space_s)
            .align_y(Alignment::Center)
            .push(
                dim_caption(tr(
                    "A printer or a game controller: plug it in to check it.",
                ))
                .wrapping(Wrapping::Word)
                .width(Length::Fill),
            )
            .push(widget::button::link(tr("Copy the report")).on_press(Message::CopyReport));
        widget::column::with_capacity(2)
            .spacing(space_s)
            .push(section(rows))
            .push(footer)
            .into()
    }

    /// Live session's last page: what to look at before installing (the
    /// checks that are not all right), and the install guide
    pub(super) fn install_view<'a>(&'a self, space_s: u16) -> Element<'a, Message> {
        let issues: Vec<&readiness::Check> = self
            .readiness
            .iter()
            .flatten()
            .filter(|check| check.status >= readiness::Status::Warn)
            .collect();
        let mut rows = Vec::new();
        if let Some(worst) = issues.iter().map(|check| check.status).max() {
            let title = if issues.len() == 1 {
                tr("One thing to look at before installing").to_owned()
            } else {
                fill(
                    tr("{} things to look at before installing"),
                    &issues.len().to_string(),
                )
            };
            let titles: Vec<&str> = issues.iter().map(|check| tr(check.title)).collect();
            rows.push(
                settings::item_row(vec![
                    status_icon(worst),
                    widget::column::with_capacity(2)
                        .spacing(2)
                        .push(text::heading(title))
                        .push(dim_caption(titles.join(", ")).wrapping(Wrapping::Word))
                        .width(Length::Fill)
                        .into(),
                    widget::button::standard(tr("Review"))
                        .on_press(Message::PageOpen(1))
                        .into(),
                ])
                .spacing(space_s)
                .into(),
            );
        }
        rows.push(
            settings::item_row(vec![
                row_icon("help-browser-symbolic"),
                two_lines(
                    text::heading(tr("Installation guide")).into(),
                    tr("Keep Windows next to it, choose where to install, the NVIDIA key."),
                ),
                widget::button::standard(tr("Open"))
                    .trailing_icon(widget::button::link::icon())
                    .on_press(Message::Run(Action::OpenUri(install_url(""))))
                    .into(),
            ])
            .spacing(space_s)
            .into(),
        );
        widget::column::with_capacity(2)
            .spacing(space_s)
            .push(
                widget::container(widget::icon::from_name("celenn-installer").size(128))
                    .padding([space_s, 0])
                    .center_x(Length::Fill),
            )
            .push(section(rows))
            .into()
    }

    pub(super) fn ready_view<'a>(&'a self, space_s: u16) -> Element<'a, Message> {
        let mut done = Vec::new();
        let check = |title: String, caption: Option<String>| -> Element<'a, Message> {
            let mut column = widget::column::with_capacity(2)
                .spacing(2)
                .push(text::heading(title));
            if let Some(caption) = caption {
                column = column.push(dim_caption(caption).wrapping(Wrapping::Word));
            }
            settings::item_row(vec![state_icon(true), column.width(Length::Fill).into()])
                .spacing(space_s)
                .into()
        };
        if self.look_changed {
            done.push(check(tr("Your look is set").to_owned(), None));
        }
        match (&self.install, self.install_count()) {
            (_, Some((finished, all))) => {
                done.push(check(
                    fill(
                        &fill(tr("Installing: {} of {}"), &finished.to_string()),
                        &all.to_string(),
                    ),
                    Some(tr("They show up in the app library as they arrive.").to_owned()),
                ));
            }
            (Install::Done, _) => done.push(check(tr("Your apps are installed").to_owned(), None)),
            _ => {}
        }
        // From the former "Good to know" page: what works differently
        done.push(check(
            tr("Everything updates by itself").to_owned(),
            Some(
                tr("In the background, applied at the next restart. The previous version stays one click away.")
                    .to_owned(),
            ),
        ));
        let backups = settings::item_row(vec![
            row_icon("org.gnome.DejaDup"),
            two_lines(
                text::heading(tr("Protect your files")).into(),
                tr("Plug in a USB drive and turn on automatic backups: Déjà Dup keeps a copy of your home folder."),
            ),
            widget::button::suggested(tr("Set up backups"))
                .on_press_maybe(in_path("deja-dup").then_some(Message::Run(Action::Launch(&["deja-dup"]))))
                .into(),
        ])
        .spacing(space_s);
        let belongings = settings::item_row(vec![
            row_icon("celenn-tour-files"),
            two_lines(
                text::heading(tr("Bring your things")).into(),
                tr("Files, photos from a phone, bookmarks and email from before"),
            ),
            action_button("Show me how", &Action::Launch(&[TOUR, "--belongings"])),
        ])
        .spacing(space_s);
        let help = settings::item_row(vec![
            row_icon("help-browser"),
            two_lines(
                text::heading(tr("Documentation")).into(),
                tr("Updates, software, commands and tips"),
            ),
            widget::button::standard(tr("Open"))
                .trailing_icon(widget::button::link::icon())
                .on_press(Message::Run(Action::OpenUri(docs_url())))
                .into(),
        ])
        .spacing(space_s);
        let mut column = widget::column::with_capacity(5).spacing(space_s);
        // An install that did not work, with "Try again"
        if let Some(card) = &self.install_card {
            column = column.push(card.view(Message::InstallCard));
        }
        let computer = self.computer_rows(space_s);
        column = column.push(section(done));
        if !computer.is_empty() {
            column = column.push(section(computer));
        }
        column
            .push(settings::section().add(backups).add(belongings).add(help))
            .push(
                text::caption(tr(
                    "This tour and the Keyboard shortcuts app stay in the app library.",
                ))
                .align_x(Alignment::Center)
                .width(Length::Fill),
            )
            .into()
    }

    /// The window a downloaded .exe, .msi, .deb or .rpm file opens
    pub(super) fn open_view<'a>(&'a self, file: &'a Path) -> Element<'a, Message> {
        let cosmic_theme::Spacing {
            space_s,
            space_m,
            space_l,
            ..
        } = theme::spacing();
        let kind = system::package_kind(file);
        let name = short_name(file);
        let term = system::search_term(file);
        let explanation = match kind {
            Package::Windows => {
                "This is a Windows program. On Celenn OS, apps come from COSMIC Store: most well-known apps are there, or a good alternative."
            }
            Package::Debian => {
                "This is a package for Debian and Ubuntu. On Celenn OS, apps come from COSMIC Store: most well-known apps are there, or a good alternative."
            }
            Package::Rpm => {
                "This is a package for Fedora. Celenn OS's system is a read-only image: apps come from COSMIC Store instead. Most well-known apps are there, or a good alternative."
            }
        };
        let hint = match kind {
            Package::Windows => {
                "A Windows program with no equivalent? Bottles, in COSMIC Store, runs some of them."
            }
            _ => {
                "For advanced users: distrobox installs it in a container of another distribution."
            }
        };
        let search_label = if term.is_empty() {
            tr("Open COSMIC Store").to_owned()
        } else {
            fill(tr("Search for “{}” in COSMIC Store"), &term)
        };
        let search: &'static [&'static str] = if term.is_empty() {
            &["cosmic-store"]
        } else {
            Box::leak(Box::new(["cosmic-store", Box::leak(term.into_boxed_str())]))
        };
        let mut buttons = widget::row::with_capacity(4)
            .spacing(space_s)
            .push(widget::space::horizontal())
            .push(widget::button::standard(tr("Close")).on_press(Message::Exit));
        if kind == Package::Windows && self.installed.contains("com.usebottles.bottles") {
            buttons = buttons.push(widget::button::standard(tr("Open with Bottles")).on_press(
                Message::OpenWith(&["flatpak", "run", "com.usebottles.bottles"]),
            ));
        }
        buttons = buttons
            .push(widget::button::suggested(search_label).on_press(Message::OpenWith(search)));
        widget::column::with_capacity(6)
            .spacing(space_m)
            .push(
                text::title3(fill(tr("“{}” does not install here"), &name))
                    .wrapping(Wrapping::Word),
            )
            .push(text::body(tr(explanation)).wrapping(Wrapping::Word))
            .push(text::caption(tr(hint)).wrapping(Wrapping::Word))
            // The site's table of Windows and Mac programs and their
            // equivalents (a link: the buttons row is full)
            .push_maybe((kind == Package::Windows).then(|| {
                widget::button::link(tr("See the equivalents of Windows programs"))
                    .trailing_icon(true)
                    .on_press(Message::Run(Action::OpenUri(format!(
                        "{}#alternatives",
                        docs_url()
                    ))))
            }))
            .push(widget::space::vertical())
            .push(buttons)
            .padding(space_l)
            .apply(widget::container)
            .into()
    }

    /// "Send without a cable": how to get photos and files from a phone
    /// with LocalSend (from the "Bring your things" page)
    pub(super) fn phone_view(&self) -> Element<'_, Message> {
        let cosmic_theme::Spacing {
            space_xxs,
            space_s,
            space_m,
            space_l,
            ..
        } = theme::spacing();
        let steps = [
            "On the phone, scan this code and install LocalSend (Play Store or App Store).",
            "Open LocalSend here and on the phone, on the same Wi-Fi.",
            "On the phone, pick the photos, then this computer. They arrive in Downloads. It works the other way too.",
        ]
        .into_iter()
        .enumerate()
        .fold(
            widget::column::with_capacity(3).spacing(space_s),
            |column, (i, step)| {
                column.push(
                    widget::row::with_capacity(2)
                        .spacing(space_s)
                        .align_y(Alignment::Start)
                        .push(text::heading(format!("{}.", i + 1)))
                        .push(text::body(tr(step)).wrapping(Wrapping::Word).width(Length::Fill)),
                )
            },
        );
        let mut middle = widget::row::with_capacity(2)
            .spacing(space_m)
            .align_y(Alignment::Center);
        if Path::new(LOCALSEND_QR).exists() {
            middle = middle.push(
                widget::svg(widget::svg::Handle::from_path(LOCALSEND_QR))
                    .width(140)
                    .height(140),
            );
        }
        middle = middle.push(steps);
        let open = widget::button::suggested(tr("Open LocalSend"));
        let open = if self.installed.contains("org.localsend.localsend_app") {
            open.on_press(Message::OpenWith(LOCALSEND))
        } else {
            open
        };
        widget::column::with_capacity(5)
            .spacing(space_m)
            .push(text::title3(tr("Send without a cable")))
            .push(
                text::caption(tr("With LocalSend, already on this computer. The files go straight through your Wi-Fi, encrypted, not through the internet."))
                    .wrapping(Wrapping::Word),
            )
            .push(middle)
            .push(widget::space::vertical())
            .push(
                widget::row::with_capacity(3)
                    .spacing(space_xxs.max(space_s))
                    .push(widget::space::horizontal())
                    .push(widget::button::standard(tr("Close")).on_press(Message::Exit))
                    .push(open),
            )
            .padding(space_l)
            .apply(widget::container)
            .into()
    }

    /// "Bring your things": files, photos, bookmarks and email from the
    /// old computer (from the last page of the tour)
    pub(super) fn belongings_view(&self) -> Element<'_, Message> {
        let cosmic_theme::Spacing {
            space_s,
            space_m,
            space_l,
            ..
        } = theme::spacing();
        let rows = BELONGINGS
            .iter()
            .map(|row| row_view(row, space_s))
            .collect();
        widget::column::with_capacity(4)
            .spacing(space_m)
            .push(text::title3(tr("Bring your things")))
            .push(section(rows))
            .push(widget::space::vertical())
            .push(
                widget::row::with_capacity(2)
                    .push(widget::space::horizontal())
                    .push(widget::button::suggested(tr("Close")).on_press(Message::Exit)),
            )
            .padding(space_l)
            .apply(widget::container)
            .into()
    }
}

/// The origin cards: a tile, with an accent border when chosen
pub(super) fn card_style(selected: bool) -> theme::Button {
    crate::style::tile_style(selected)
}

/// "Back to the list", under a category's list of apps
pub(super) fn back_to_list<'a>() -> Element<'a, Message> {
    widget::button::standard(tr("Back to the list"))
        .on_press(Message::Sub(None))
        .apply(widget::container)
        .center_x(Length::Fill)
        .into()
}

/// A row of a page: title (keys as keys, "A → B" with an arrow), caption
/// and maybe a button
pub(super) fn row_view<'a>(row: &'a Row, space_s: u16) -> Element<'a, Message> {
    let mut items = vec![two_lines(title_line(tr(row.title)), tr(row.caption))];
    if let Some((label, action)) = &row.button {
        items.push(action_button(label, action));
    }
    settings::item_row(items).spacing(space_s).into()
}

/// Rows in a COSMIC settings section
pub(super) fn section<'a>(rows: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    rows.into_iter()
        .fold(settings::section(), |section, row| section.add(row))
        .into()
}

/// A line of progress: text, caption and bar
pub(super) fn install_progress<'a>(
    title: String,
    caption: &'static str,
    done: usize,
    all: usize,
    space_s: u16,
) -> Element<'a, Message> {
    let progress = if all == 0 {
        0.0
    } else {
        done as f32 / all as f32
    };
    settings::section()
        .add(
            widget::column::with_capacity(3)
                .spacing(space_s)
                .push(text::heading(title))
                .push(widget::determinate_linear(progress).width(Length::Fill))
                .push(dim_caption(caption).wrapping(Wrapping::Word))
                .apply(widget::container)
                .padding([space_s, space_s + 8]),
        )
        .into()
}

/// A row's text: a first line, and a caption under it (like the screen
/// reader row of cosmic-initial-setup's accessibility page)
pub(super) fn two_lines<'a>(
    first: Element<'a, Message>,
    caption: &'static str,
) -> Element<'a, Message> {
    widget::column::with_capacity(2)
        .spacing(2)
        .push(first)
        .push(with_keys(caption))
        .width(Length::Fill)
        .into()
}
