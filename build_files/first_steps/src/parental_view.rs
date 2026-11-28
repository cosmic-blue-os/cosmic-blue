// Parental controls: what the window shows (App::view's pages and the
// widgets they are made of)

use super::*;

/// The page around a content: its header (a picture, the title and a
/// caption), then the content; it scrolls only when the window is made
/// smaller than its default size. "Back" is in the header bar.
pub(super) fn frame<'a>(
    picture: Element<'a, Message>,
    title: String,
    caption: Option<String>,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    let cosmic_theme::Spacing {
        space_xxs,
        space_m,
        space_l,
        ..
    } = theme::spacing();
    widget::column::with_capacity(5)
        .push(widget::space::vertical().height(space_xxs))
        .push(style::page_header(picture, title, caption))
        .push(widget::space::vertical().height(space_m))
        .push(content)
        .push(widget::space::vertical().height(space_l))
        .max_width(640.0)
        .width(640.0)
        .apply(widget::container)
        .center_x(Length::Fill)
        .apply(widget::scrollable)
        .into()
}

/// The app's icon, at the size of a page header's picture
pub(super) fn app_icon<'a>() -> Element<'a, Message> {
    widget::icon::from_name(ICON).size(44).into()
}

/// Celenn OS's profile pictures, to pick one for a new child account, cut
/// as rounded squares (the links, such as COSMIC's default picture, are not listed
/// twice)
pub(super) fn faces() -> Vec<(PathBuf, widget::image::Handle)> {
    let mut faces: Vec<PathBuf> = std::fs::read_dir(FACES)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "png"))
        .collect();
    faces.sort();
    faces
        .into_iter()
        .filter_map(|face| {
            let handle = style::rounded_picture(&face, 72)?;
            Some((face, handle))
        })
        .collect()
}

/// A row's text: a first line, and a caption under it
pub(super) fn two_lines<'a>(first: impl Into<String>, caption: String) -> Element<'a, Message> {
    widget::column::with_capacity(2)
        .spacing(2)
        .push(style::row_title(first.into()))
        .push(style::dim_caption(caption).wrapping(Wrapping::Word))
        .width(Length::Fill)
        .into()
}

pub(super) fn avatar<'a>(account: &Account, size: u16) -> Element<'a, Message> {
    match &account.avatar {
        Some(handle) => widget::image(handle.clone())
            .width(size)
            .height(size)
            .into(),
        None => widget::icon::from_name("avatar-default-symbolic")
            .size(size)
            .into(),
    }
}

/// A rounded bar in the accent color (or a faint one)
pub(super) fn bar<'a>(width: Length, height: f32, accent: bool) -> Element<'a, Message> {
    widget::container(widget::space::horizontal().width(Length::Fixed(0.0)))
        .width(width)
        .height(Length::Fixed(height))
        .class(theme::Container::custom(move |theme| {
            let cosmic = theme.cosmic();
            let color = if accent {
                cosmic.accent_color()
            } else {
                let mut color = cosmic.on_bg_color();
                color.alpha = 0.15;
                color
            };
            cosmic::iced::widget::container::Style {
                background: Some(cosmic::iced::Color::from(color).into()),
                border: cosmic::iced::Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        }))
        .into()
}

/// Today's screen time of an account, as a caption and a gauge (0 to 1)
pub(super) fn today(account: &Account) -> (String, Option<f32>) {
    let Some(rules) = &account.rules else {
        return (tr("No limits").to_owned(), None);
    };
    let Some(now) = Now::read() else {
        return (String::new(), None);
    };
    let used = account.usage.used_today(&now);
    let verdict = parental_rules::verdict(rules, used, &account.extra, &now);
    match verdict.allowed_today {
        Some(allowed) => (
            tr("Today: {} of {}")
                .replacen("{}", &duration(used), 1)
                .replacen("{}", &duration(allowed), 1),
            Some(if allowed == 0 {
                1.0
            } else {
                (used as f32 / allowed as f32).min(1.0)
            }),
        ),
        None if rules.bedtime_enabled || rules.web_filter => {
            (fill(tr("Today: {}"), &duration(used)), None)
        }
        None => (tr("No limits").to_owned(), None),
    }
}

impl App {
    pub(super) fn home(&self) -> Element<'_, Message> {
        let space_s = theme::spacing().space_s;
        let space_m = theme::spacing().space_m;
        let mut content = widget::column::with_capacity(4).spacing(space_m);

        // A child's account: no administrator rights, and rules (a standard
        // account without rules may be an adult's: it is in "Other accounts")
        let children: Vec<&Account> = self
            .accounts
            .iter()
            .filter(|a| !a.admin && a.rules.is_some())
            .collect();
        let mut section = settings::section().title(tr("Child accounts"));
        for account in children {
            let (caption, gauge) = today(account);
            let mut lines = widget::column::with_capacity(3)
                .spacing(2)
                .push(style::row_title(account.name.clone()))
                .push(style::dim_caption(caption))
                .width(Length::Fill);
            if let Some(gauge) = gauge {
                lines = lines.push(widget::space::vertical().height(4)).push(
                    widget::progress_bar::determinate_linear(gauge)
                        .width(Length::Fill)
                        .girth(6),
                );
            }
            section = section.add(
                widget::button::custom(
                    settings::item_row(vec![
                        avatar(account, 40),
                        lines.into(),
                        widget::icon::from_name("go-next-symbolic").size(16).into(),
                    ])
                    .spacing(space_s),
                )
                .class(theme::Button::MenuItem)
                .padding(0)
                .width(Length::Fill)
                .on_press(Message::Open(account.user.clone())),
            );
        }
        section = section.add(
            settings::item_row(vec![
                two_lines(
                    tr("Add a child account"),
                    tr("An account without administrator rights, ready in one click").to_owned(),
                ),
                widget::button::suggested(tr("Add…"))
                    .on_press(Message::ShowAdd)
                    .into(),
            ])
            .spacing(space_s),
        );
        content = content.push(section);

        let others: Vec<&Account> = self
            .accounts
            .iter()
            .filter(|a| a.admin || a.rules.is_none())
            .collect();
        if !others.is_empty() {
            let mut section = settings::section().title(tr("Other accounts"));
            for account in others {
                let mut row = vec![
                    avatar(account, 40),
                    two_lines(
                        account.name.clone(),
                        tr(if account.admin {
                            "Administrator · no limits"
                        } else {
                            "Standard account · no limits"
                        })
                        .to_owned(),
                    ),
                ];
                if !account.admin {
                    // Its page: the rules are saved at the first change
                    row.push(
                        widget::button::standard(tr("This is a child"))
                            .on_press(Message::Open(account.user.clone()))
                            .into(),
                    );
                }
                section = section.add(settings::item_row(row).spacing(space_s));
            }
            content = content.push(section);
        }
        content = content.push(
            style::dim_caption(tr(
                "Only a parent (an administrator account) can change these settings.",
            ))
            .align_x(Alignment::Center)
            .width(Length::Fill),
        );
        frame(
            app_icon(),
            tr("Parental controls").to_owned(),
            Some(tr("Choose how long and when your children can use this computer.").to_owned()),
            content.into(),
        )
    }

    pub(super) fn child(&self, user: &str) -> Element<'_, Message> {
        let space_s = theme::spacing().space_s;
        let space_m = theme::spacing().space_m;
        let Some(account) = self.account(user) else {
            return frame(
                app_icon(),
                user.to_owned(),
                None,
                widget::column::with_capacity(0).into(),
            );
        };
        let rules = account.rules.clone().unwrap_or_else(Rules::none);
        let toggle = |on: bool, change: fn(&mut Rules, bool)| {
            let rules = rules.clone();
            widget::toggler(on).on_toggle(move |value| {
                let mut rules = rules.clone();
                change(&mut rules, value);
                Message::Rules(rules)
            })
        };
        // - and + by 15 minutes, and the value itself can be typed in
        let minutes_button = |field: Field| -> Element<'_, Message> {
            let shown = match &self.editing {
                Some((editing, typed)) if *editing == field => typed.clone(),
                _ => field.show(field.get(&rules)),
            };
            widget::row::with_capacity(3)
                .push(
                    widget::button::icon(widget::icon::from_name("list-remove-symbolic"))
                        .name(tr("Less time"))
                        .tooltip(tr("Less time"))
                        .on_press(Message::Step(field, false)),
                )
                .push(
                    // A visible field, so that it shows it can be typed in
                    widget::text_input("", shown)
                        .select_on_focus(true)
                        .on_input(move |typed| Message::Edit(field, typed))
                        .on_submit(|_| Message::Commit)
                        .on_unfocus(Message::Commit)
                        .width(Length::Fixed(96.0)),
                )
                .push(
                    widget::button::icon(widget::icon::from_name("list-add-symbolic"))
                        .name(tr("More time"))
                        .tooltip(tr("More time"))
                        .on_press(Message::Step(field, true)),
                )
                .align_y(Alignment::Center)
                .into()
        };

        let mut screen = settings::section().title(tr("Screen time")).add(
            settings::item_row(vec![
                two_lines(
                    tr("Limit screen time"),
                    tr("Only time in front of the screen counts. Warnings 10, 5 and 1 minutes before the end.").to_owned(),
                ),
                toggle(rules.limit_enabled, |r, v| r.limit_enabled = v).into(),
            ])
            .spacing(space_s),
        );
        if rules.limit_enabled {
            screen = screen
                .add(settings::item(
                    tr("Monday to Friday"),
                    minutes_button(Field::Weekdays),
                ))
                .add(settings::item(
                    tr("Saturday and Sunday"),
                    minutes_button(Field::Weekend),
                ));
        }

        let mut bedtime = settings::section().title(tr("Bedtime")).add(
            settings::item_row(vec![
                two_lines(
                    tr("No computer at night"),
                    tr("The session closes and logging in is refused during these hours.")
                        .to_owned(),
                ),
                toggle(rules.bedtime_enabled, |r, v| r.bedtime_enabled = v).into(),
            ])
            .spacing(space_s),
        );
        if rules.bedtime_enabled {
            bedtime = bedtime
                .add(settings::item(
                    tr("From"),
                    minutes_button(Field::BedtimeStart),
                ))
                .add(settings::item(tr("To"), minutes_button(Field::BedtimeEnd)));
        }

        let internet = settings::section().title(tr("Internet")).add(
            settings::item_row(vec![
                widget::column::with_capacity(2)
                    .spacing(2)
                    .push(
                        widget::row::with_capacity(2)
                            .spacing(space_s)
                            .align_y(Alignment::Center)
                            .push(style::row_title(tr("Block unsuitable websites")))
                            .push(
                                text::caption(tr("Recommended"))
                                    .apply(widget::container)
                                    .padding([1, 8])
                                    .class(theme::Container::custom(|theme| {
                                        let cosmic = theme.cosmic();
                                        cosmic::iced::widget::container::Style {
                                            background: Some(cosmic::iced::Color::from(cosmic.accent_color()).into()),
                                            text_color: Some(cosmic::iced::Color::from(cosmic.accent.on)),
                                            border: cosmic::iced::Border {
                                                radius: 10.0.into(),
                                                ..Default::default()
                                            },
                                            ..Default::default()
                                        }
                                    })),
                            ),
                    )
                    .push(
                        style::dim_caption(tr("Adult content, gambling and malicious sites, in every browser. Safe search is forced. Uses the CleanBrowsing family filter."))
                            .wrapping(Wrapping::Word),
                    )
                    .width(Length::Fill)
                    .into(),
                toggle(rules.web_filter, |r, v| r.web_filter = v).into(),
            ])
            .spacing(space_s),
        );

        // One tab at a time, so that the page fits without scrolling
        let tabs = widget::segmented_control::horizontal(&self.tabs)
            .on_activate(Message::Tab)
            .width(Length::Fill);
        let mut content = widget::column::with_capacity(3).spacing(space_m).push(tabs);
        content = match self.tabs.active_data::<Tab>().copied().unwrap_or(Tab::Time) {
            Tab::Time => content.push(screen).push(bedtime),
            Tab::Apps => content.push(self.apps_section(account, &rules)),
            Tab::Internet => content.push(internet),
            Tab::Week => content.push(self.week(account, &rules)),
        };
        let (caption, _) = today(account);
        frame(
            avatar(account, 52),
            account.name.clone(),
            Some(caption),
            content.into(),
        )
    }

    /// The "Applications" section of a child's page
    pub(super) fn apps_section<'a>(
        &'a self,
        account: &Account,
        rules: &Rules,
    ) -> Element<'a, Message> {
        let space_s = theme::spacing().space_s;
        let mut section = settings::section().title(tr("Applications")).add(
            settings::item_row(vec![
                two_lines(
                    tr("Applications validated by a parent"),
                    fill(
                        tr("{} only uses the applications you ticked. Installing an application needs a parent's password."),
                        &account.name,
                    ),
                ),
                widget::toggler(rules.apps_enabled)
                    .on_toggle(Message::AppsEnabled)
                    .into(),
            ])
            .spacing(space_s),
        );
        if rules.apps_enabled {
            let caption = match &self.apps {
                Some(apps) => tr("{} of {}")
                    .replacen(
                        "{}",
                        &apps
                            .iter()
                            .filter(|a| rules.allowed_apps.contains(&a.id))
                            .count()
                            .to_string(),
                        1,
                    )
                    .replacen("{}", &apps.len().to_string(), 1),
                None => String::new(),
            };
            section = section
                .add(
                    settings::item_row(vec![
                        two_lines(tr("Allowed applications"), caption),
                        widget::button::standard(tr("Choose…"))
                            .on_press(Message::OpenApps(account.user.clone()))
                            .into(),
                    ])
                    .spacing(space_s),
                )
                .add(settings::item_row(vec![
                    style::dim_caption(tr("Downloaded programs (AppImages, executable files) do not run on this account."))
                        .wrapping(Wrapping::Word)
                        .width(Length::Fill)
                        .into(),
                ]));
        }
        section.into()
    }

    /// The applications a child may use, to tick
    pub(super) fn apps_page(&self, user: &str) -> Element<'_, Message> {
        let space_s = theme::spacing().space_s;
        let space_m = theme::spacing().space_m;
        let Some(account) = self.account(user) else {
            return frame(
                app_icon(),
                user.to_owned(),
                None,
                widget::column::with_capacity(0).into(),
            );
        };
        let rules = account.rules.clone().unwrap_or_else(Rules::none);
        let title = fill(tr("Applications of {}"), &account.name);
        let mut content = widget::column::with_capacity(4).spacing(space_m).push(
            widget::search_input(tr("Search for an application"), self.search.clone())
                .on_input(Message::Search)
                .on_clear(Message::Search(String::new()))
                .width(Length::Fill),
        );
        if self.apps.is_none() {
            return frame(avatar(account, 52), title, None, content.into());
        }
        let found = self.found_apps();
        // How many are allowed, or found; and both buttons for those
        let allowed_found = found
            .iter()
            .filter(|app| rules.allowed_apps.contains(&app.id))
            .count();
        let count = if self.search.trim().is_empty() {
            tr("{allowed} allowed out of {total}")
                .replace("{allowed}", &allowed_found.to_string())
                .replace("{total}", &found.len().to_string())
        } else if found.len() == 1 {
            tr("One application found").to_owned()
        } else {
            fill(tr("{} applications found"), &found.len().to_string())
        };
        content = content.push(
            widget::row::with_capacity(3)
                .spacing(space_s)
                .align_y(Alignment::Center)
                .push(
                    style::dim_caption(count)
                        .wrapping(Wrapping::Word)
                        .width(Length::Fill),
                )
                .push(widget::button::text(tr("Allow all")).on_press_maybe(
                    (allowed_found < found.len()).then_some(Message::AllowAll(true)),
                ))
                .push(
                    widget::button::text(tr("Block all"))
                        .on_press_maybe((allowed_found > 0).then_some(Message::AllowAll(false))),
                ),
        );
        for (heading, allowed) in [(tr("Allowed"), true), (tr("Blocked"), false)] {
            let mut section = settings::section().title(heading);
            let mut any = false;
            for app in found
                .iter()
                .filter(|a| rules.allowed_apps.contains(&a.id) == allowed)
            {
                any = true;
                let icon = if app.icon.starts_with('/') {
                    widget::icon::from_path(app.icon.clone().into())
                } else {
                    widget::icon::from_name(app.icon.clone()).size(32).handle()
                };
                let id = app.id.clone();
                section = section.add(
                    settings::item_row(vec![
                        widget::icon(icon).size(32).into(),
                        two_lines(app.name.clone(), app.comment.clone()),
                        widget::checkbox(allowed)
                            .on_toggle(move |on| Message::Allow(id.clone(), on))
                            .into(),
                    ])
                    .spacing(space_s),
                );
            }
            if any {
                content = content.push(section);
            }
        }
        content = content.push(
            text::caption(tr(
                "Applications installed later are blocked until you tick them.",
            ))
            .align_x(Alignment::Center)
            .width(Length::Fill),
        );
        frame(avatar(account, 52), title, None, content.into())
    }

    /// The screen time of the last seven days, as bars (always seven: a
    /// day without use has a flat bar), then the most used applications
    pub(super) fn week<'a>(&'a self, account: &Account, rules: &Rules) -> Element<'a, Message> {
        const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
        const HEIGHT: f32 = 60.0;
        /// Room above a bar for its value
        const VALUE: f32 = 16.0;
        let space_s = theme::spacing().space_s;
        let Some(now) = Now::read() else {
            return widget::column::with_capacity(0).into();
        };
        let Some(today) = parental_rules::day_number(&now.date) else {
            return widget::column::with_capacity(0).into();
        };
        let mut known = account.usage.history.clone();
        if account.usage.today.date == now.date {
            known.push(account.usage.today.clone());
        }
        // The seven days that end today, oldest first
        let days: Vec<(u32, Option<&parental_rules::Day>)> = (0..7)
            .rev()
            .map(|back: i64| {
                let weekday = ((i64::from(now.weekday) - 1 - back).rem_euclid(7) + 1) as u32;
                let day = known
                    .iter()
                    .find(|d| parental_rules::day_number(&d.date) == Some(today - back));
                (weekday, day)
            })
            .collect();
        let seconds = |day: &Option<&parental_rules::Day>| day.map_or(0, |d| d.seconds);
        let most = days.iter().map(|(_, d)| seconds(d)).max().unwrap_or(0);
        // The limit line, when every day has the same one and a day came
        // near it (otherwise the bars would be tiny under it)
        let limit = rules
            .daily_limit(1)
            .filter(|_| rules.daily_limit(1) == rules.daily_limit(7))
            .map(|m| u64::from(m) * 60)
            .filter(|limit| most * 4 >= limit * 3);
        let highest = most.max(limit.unwrap_or(0)).max(60);
        let height_of = |secs: u64| secs as f32 / highest as f32 * HEIGHT;
        let mut bars = widget::row::with_capacity(7)
            .spacing(space_s)
            .align_y(Alignment::End)
            .height(Length::Fixed(HEIGHT + VALUE));
        let mut labels = widget::row::with_capacity(7).spacing(space_s);
        for (i, (weekday, day)) in days.iter().enumerate() {
            let used = seconds(day);
            let is_today = i == days.len() - 1;
            bars = bars.push(
                widget::column::with_capacity(2)
                    .spacing(2)
                    .align_x(Alignment::Center)
                    .width(Length::Fill)
                    .push_maybe((used >= 60).then(|| text::caption_heading(duration(used))))
                    .push(bar(Length::Fixed(26.0), height_of(used).max(3.0), used > 0)),
            );
            let label = if is_today {
                tr("Today").to_owned()
            } else {
                DAYS.get(weekday.saturating_sub(1) as usize)
                    .map_or(String::new(), |d| tr(d).to_owned())
            };
            labels = labels.push(
                widget::container(if is_today {
                    text::caption_heading(label)
                } else {
                    text::caption(label)
                })
                .center_x(Length::Fill),
            );
        }
        // The limit as a thin line across the bars, with its value
        let chart: Element<'a, Message> = match limit {
            Some(limit) => cosmic::iced::widget::stack![
                bars,
                widget::column::with_capacity(2)
                    .push(widget::space::vertical().height(Length::Fixed(
                        (HEIGHT + VALUE - height_of(limit) - 14.0).max(0.0)
                    )))
                    .push(
                        widget::container(text::caption(duration(limit)))
                            .align_x(Alignment::End)
                            .width(Length::Fill)
                            .height(Length::Fixed(14.0)),
                    )
                    .push(
                        widget::container(widget::space::horizontal())
                            .width(Length::Fill)
                            .height(Length::Fixed(1.0))
                            .class(theme::Container::custom(|theme| {
                                let mut color = theme.cosmic().on_bg_color();
                                color.alpha = 0.3;
                                cosmic::iced::widget::container::Style {
                                    background: Some(cosmic::iced::Color::from(color).into()),
                                    ..Default::default()
                                }
                            })),
                    ),
            ]
            .into(),
            None => bars.into(),
        };
        // The average of the days the computer was used
        let used: Vec<u64> = days
            .iter()
            .map(|(_, d)| seconds(d))
            .filter(|s| *s > 0)
            .collect();
        let average = used.iter().sum::<u64>() / used.len().max(1) as u64;
        let mut column = widget::column::with_capacity(2).push(
            // (the tab says "This week")
            settings::section().add(
                widget::column::with_capacity(3)
                    .spacing(theme::spacing().space_xxs)
                    .push(chart)
                    .push(labels)
                    .push(
                        widget::container(style::dim_caption(fill(
                            tr("Average: {} a day"),
                            &duration(average),
                        )))
                        .center_x(Length::Fill),
                    )
                    .padding([theme::spacing().space_xxs, space_s]),
            ),
        );
        // The applications in front the longest over the seven days
        let mut apps: std::collections::BTreeMap<&str, u64> = std::collections::BTreeMap::new();
        for (_, day) in &days {
            for (id, secs) in day.iter().flat_map(|d| d.apps.iter()) {
                *apps.entry(id.as_str()).or_default() += secs;
            }
        }
        let mut apps: Vec<(&str, u64)> = apps.into_iter().filter(|(_, s)| *s >= 60).collect();
        apps.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        apps.truncate(5);
        if let Some(&(_, most)) = apps.first() {
            let mut section = settings::section().title(tr("Most used applications"));
            for (id, secs) in apps {
                let app = self.apps.iter().flatten().find(|app| app.matches(id));
                let icon = match app.map(|app| app.icon.as_str()) {
                    Some(icon) if icon.starts_with('/') => widget::icon::from_path(icon.into()),
                    Some(icon) => widget::icon::from_name(icon).size(24).handle(),
                    None => widget::icon::from_name("application-x-executable")
                        .size(24)
                        .handle(),
                };
                // Not among the applications: the last part of its id
                // (org.kde.krita: Krita)
                let name = app.map_or_else(|| pretty_id(id), |app| app.name.clone());
                section = section.add(
                    settings::item_row(vec![
                        widget::icon(icon).size(24).into(),
                        widget::column::with_capacity(2)
                            .spacing(6)
                            .push(style::row_title(name))
                            .push(
                                widget::progress_bar::determinate_linear(secs as f32 / most as f32)
                                    .girth(6)
                                    .width(Length::Fill),
                            )
                            .width(Length::Fill)
                            .into(),
                        // The same width for every time, so that the bars
                        // line up
                        text::body(duration(secs))
                            .align_x(Alignment::End)
                            .width(Length::Fixed(72.0))
                            .into(),
                    ])
                    .spacing(space_s),
                );
            }
            column = column.push(section);
        }
        column.spacing(space_s).into()
    }

    pub(super) fn add(&self) -> Element<'_, Message> {
        let space_s = theme::spacing().space_s;
        let space_m = theme::spacing().space_m;
        let ready = !self.name.trim().is_empty() && !self.password.is_empty() && !self.busy;
        let create: Element<'_, Message> = if self.busy {
            widget::progress_bar::indeterminate_circular().into()
        } else {
            widget::button::suggested(tr("Create"))
                .on_press_maybe(ready.then_some(Message::Create))
                .into()
        };
        // The pictures, in two rows, the one picked outlined
        let mut pictures = widget::column::with_capacity(2).spacing(8);
        for chunk in self.faces.chunks(self.faces.len().div_ceil(2).max(1)) {
            let mut row = widget::row::with_capacity(chunk.len()).spacing(8);
            for (face, handle) in chunk {
                row = row.push(
                    style::picture_button(
                        handle.clone(),
                        36,
                        self.face.as_ref() == Some(face),
                        Message::Face(face.clone()),
                    )
                    // For screen readers: "Picture: rocket"
                    .name(format!(
                        "{}: {}",
                        tr("Picture"),
                        style::image_name(face)
                    )),
                );
            }
            pictures = pictures.push(row);
        }
        let content = widget::column::with_capacity(4)
            .spacing(space_m)
            .push(
                settings::section()
                    .add(settings::item(
                        tr("Name"),
                        widget::text_input(tr("Name"), &self.name)
                            .on_input(Message::Name)
                            .width(Length::Fixed(280.0)),
                    ))
                    .add(settings::item(
                        tr("Password"),
                        widget::secure_input(
                            tr("Password"),
                            &self.password,
                            Some(Message::TogglePassword),
                            self.password_hidden,
                        )
                        .on_input(Message::Password)
                        .on_submit(|_| Message::Create)
                        .width(Length::Fixed(280.0)),
                    ))
                    .add_maybe((!self.faces.is_empty()).then(|| {
                        widget::column::with_capacity(2)
                            .spacing(space_s)
                            .push(text::body(tr("Picture")))
                            .push(pictures)
                            .padding([space_s, 0])
                    })),
            )
            .push(
                text::caption(tr("The child logs in with this name and password."))
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
            )
            .push(
                widget::row::with_capacity(2)
                    .spacing(space_s)
                    .push(widget::space::horizontal().width(Length::Fill))
                    .push(widget::button::standard(tr("Cancel")).on_press(Message::Back))
                    .push(create),
            );
        let picked = self
            .faces
            .iter()
            .find(|(face, _)| self.face.as_ref() == Some(face));
        let picture: Element<'_, Message> = match picked {
            Some((_, handle)) => widget::image(handle.clone()).width(44).height(44).into(),
            None => app_icon(),
        };
        frame(
            picture,
            tr("Add a child account").to_owned(),
            None,
            content.into(),
        )
    }

    /// The child's window: a parent gives more time with their password
    pub(super) fn more_time(&self) -> Element<'_, Message> {
        let cosmic_theme::Spacing {
            space_xxs,
            space_s,
            space_m,
            ..
        } = theme::spacing();
        let user = user_name();
        let account = self.account(&user);
        let name = account.map_or(user.clone(), |account| account.name.clone());
        let picture: Element<'_, Message> = match account {
            Some(account) => avatar(account, 64),
            None => widget::icon::from_name(ICON).size(64).into(),
        };
        let on = !self.no_limit && !self.busy;
        // The amount: - and +, a field to type it in, then ready-made ones
        let shown = self
            .extra_typed
            .clone()
            .unwrap_or_else(|| duration(u64::from(self.extra) * 60));
        let step = |icon: &'static str, up: bool| {
            let name = if up { tr("More time") } else { tr("Less time") };
            widget::button::icon(widget::icon::from_name(icon))
                .name(name)
                .tooltip(name)
                .on_press_maybe(on.then_some(Message::ExtraStep(up)))
        };
        let mut field = widget::text_input("", shown)
            .select_on_focus(true)
            .width(Length::Fixed(96.0));
        if on {
            field = field
                .on_input(Message::ExtraEdit)
                .on_submit(|_| Message::ExtraCommit)
                .on_unfocus(Message::ExtraCommit);
        }
        let amount = settings::item_row(vec![
            style::row_title(tr("Extra time"))
                .width(Length::Fill)
                .into(),
            widget::row::with_capacity(3)
                .push(step("list-remove-symbolic", false))
                .push(field)
                .push(step("list-add-symbolic", true))
                .align_y(Alignment::Center)
                .into(),
        ])
        .align_y(Alignment::Center);
        let choices = EXTRA_CHOICES
            .into_iter()
            .fold(
                widget::row::with_capacity(EXTRA_CHOICES.len()).spacing(space_xxs),
                |row, minutes| {
                    let label = duration(u64::from(minutes) * 60);
                    let picked = self.extra_typed.is_none() && self.extra == minutes;
                    // Same width each, the text centered
                    let class = if picked {
                        theme::Button::Suggested
                    } else {
                        theme::Button::Standard
                    };
                    row.push(
                        widget::button::custom(text::body(label).center().width(Length::Fill))
                            .padding([5, 8])
                            .width(Length::Fill)
                            .class(class)
                            .on_press_maybe(on.then_some(Message::Extra(minutes))),
                    )
                },
            )
            .width(Length::Fill);
        let no_limit = settings::item_row(vec![
            two_lines(
                tr("No limit today"),
                tr("Bedtime included, until tomorrow morning").to_owned(),
            ),
            widget::toggler(self.no_limit)
                .on_toggle_maybe((!self.busy).then_some(Message::NoLimit))
                .into(),
        ])
        .spacing(space_s);
        // Only with bedtime hours: whether the more time goes past them
        let bedtime = self
            .child
            .as_ref()
            .map(|child| &child.rules)
            .filter(|rules| rules.bedtime_enabled && rules.bedtime_start != rules.bedtime_end)
            .map(|rules| {
                let start = parental_rules::clock(rules.bedtime_start);
                let caption = if self.keep_bedtime {
                    fill(tr("The session closes at {} all the same."), &start)
                } else {
                    fill(tr("Bedtime ({}) does not apply to this time."), &start)
                };
                settings::item_row(vec![
                    two_lines(tr("Keep to bedtime"), caption),
                    widget::toggler(self.keep_bedtime)
                        .on_toggle_maybe(on.then_some(Message::KeepBedtime))
                        .into(),
                ])
                .spacing(space_s)
            });
        let section = settings::section()
            .add(amount)
            .add(choices)
            .add_maybe(bedtime)
            .add(no_limit);

        // Now, and once given
        let (status, after) = self
            .child
            .as_ref()
            .map(|child| child.texts(self.no_limit, self.extra, self.keep_bedtime))
            .unwrap_or_default();

        let give: Element<'_, Message> = if self.busy {
            widget::progress_bar::indeterminate_circular().into()
        } else {
            let label = if self.no_limit {
                tr("Remove the limit").to_owned()
            } else {
                fill(tr("Give {}"), &duration(u64::from(self.extra) * 60))
            };
            widget::button::suggested(label)
                .on_press(Message::Give)
                .into()
        };
        let centered = |line: String| style::dim_caption(line).center().width(Length::Fill);
        let mut column = widget::column::with_capacity(8)
            .spacing(space_s)
            .padding([0, space_m, space_m, space_m])
            .align_x(Alignment::Center)
            .push(picture)
            .push(
                widget::column::with_capacity(2)
                    .spacing(space_xxs)
                    .push(
                        text::title3(fill(tr("More time for {}"), &name))
                            .center()
                            .width(Length::Fill),
                    )
                    .push_maybe(status.map(centered)),
            )
            .push(section);
        if let Some(after) = after {
            column = column.push(text::body(after).center().width(Length::Fill));
        }
        column
            .push(centered(
                tr("A parent confirms with their password.").to_owned(),
            ))
            .push(
                widget::row::with_capacity(3)
                    .spacing(space_s)
                    .push(widget::space::horizontal().width(Length::Fill))
                    .push(widget::button::standard(tr("Cancel")).on_press(Message::Close))
                    .push(give),
            )
            .max_width(420.0)
            .apply(widget::container)
            .center_x(Length::Fill)
            .into()
    }
}

/// A child's rules and time used, read in its session for the "more time"
/// window
#[derive(Clone, Debug)]
pub(super) struct ChildTime {
    rules: Rules,
    used: u64,
    extra: Vec<parental_rules::Grant>,
    now: Now,
}

impl ChildTime {
    pub(super) fn read() -> Option<ChildTime> {
        let user = user_name();
        let rules = Rules::load(&user)?;
        let now = Now::read()?;
        let used = Usage::load(&user).used_today(&now);
        Some(ChildTime {
            rules,
            used,
            extra: parental_rules::load_extra(&user),
            now,
        })
    }

    /// When the session closes, in minutes after midnight (None: not today)
    pub(super) fn end(&self, extra: &[parental_rules::Grant]) -> Option<(u64, u32)> {
        let remaining = parental_rules::verdict(&self.rules, self.used, extra, &self.now)
            .remaining
            .filter(|remaining| *remaining > 0)?;
        let end = (self.now.minute + remaining.div_ceil(60) as u32) % (24 * 60);
        Some((remaining, end))
    }

    /// What the window says now, and once the time is given
    pub(super) fn texts(
        &self,
        no_limit: bool,
        minutes: u32,
        keep_bedtime: bool,
    ) -> (Option<String>, Option<String>) {
        let before = self.end(&self.extra);
        let status = before
            .map(|(remaining, _)| fill(tr("The session closes in {}."), &duration(remaining)));
        let after = if no_limit {
            if self.rules.bedtime_enabled && self.rules.bedtime_start != self.rules.bedtime_end {
                fill(
                    tr("No limit until tomorrow at {}."),
                    &parental_rules::clock(self.rules.bedtime_end),
                )
            } else {
                tr("No limit for the rest of the day.").to_owned()
            }
        } else {
            let mut extra = self.extra.clone();
            extra.push(parental_rules::Grant::new(
                self.now.unix,
                minutes,
                !keep_bedtime,
            ));
            match (self.end(&extra), before) {
                (Some((_, new)), Some((_, old))) if new != old => {
                    tr("The session will close at {new} instead of {old}.")
                        .replace("{new}", &parental_rules::clock(new))
                        .replace("{old}", &parental_rules::clock(old))
                }
                (Some((_, new)), _) => fill(
                    tr("The session will close at {}."),
                    &parental_rules::clock(new),
                ),
                (None, _) => return (status, None),
            }
        };
        (status, Some(after))
    }
}
