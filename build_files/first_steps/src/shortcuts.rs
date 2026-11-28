// The "Keyboard shortcuts" app (celenn-tour --shortcuts, Super + F1): what
// the sheet lists, opening and closing it, and the image's check that the
// sheet still matches the shortcuts COSMIC really has.

use super::*;
use std::collections::BTreeSet;

/// The keyboard shortcuts app: (heading, [(keys, what they do)])
pub const SHORTCUTS: &[(&str, &[(&str, &str)])] = &[
    (
        "Desktop",
        &[
            ("Super", "Open the launcher"),
            ("Super + A", "See all your apps"),
            ("Super + W", "See your windows and workspaces"),
            ("Super + Shift + S", "Take a screenshot"),
            ("Super + Escape", "Lock the screen"),
            ("Super + F1", "These keyboard shortcuts"),
            ("Super + Alt + Space", "Quick menu"),
        ],
    ),
    (
        "Windows",
        &[
            ("Alt + Tab", "Go from one window to the next"),
            ("Super + Q", "Close the window"),
            ("Super + M", "Maximize the window"),
            ("Super + G", "Let the window float"),
            ("Super + Y", "Turn tiling on or off"),
            ("Super + ←↑↓→", "Go to a nearby window"),
            ("Super + Shift + ←↑↓→", "Move the window"),
        ],
    ),
    (
        "Workspaces",
        &[
            ("Super + 1 … 9", "Go to a workspace"),
            ("Super + Shift + 1 … 9", "Move the window there"),
            ("Super + Ctrl + ←→", "Next or previous workspace"),
        ],
    ),
    (
        "Apps",
        &[
            ("Super + B", "Web browser"),
            ("Super + E", "Files"),
            ("Ctrl + Alt + T", "Terminal"),
            ("Ctrl + Shift + Escape", "System monitor"),
        ],
    ),
    (
        "Accessibility",
        &[
            ("Super + Alt + S", "Screen reader"),
            ("Super + = / Super + -", "Zoom in and out"),
        ],
    ),
];

/// The columns of the shortcuts sheet: numbers of SHORTCUTS' sections
pub const SHORTCUT_COLUMNS: [&[usize]; 3] = [&[0], &[1, 4], &[2, 3]];

/// Key names drawn translated on the keys ("Shift" is "Maj" in French)
pub const KEY_LABELS: &[&str] = &["Shift", "Ctrl", "Escape", "Space"];

/// The shortcuts sheet already open: close it and say so (toggle.rs)
pub fn close_open_sheet() -> bool {
    crate::toggle::close_if_open("celenn-shortcuts", "--shortcuts")
}

// The sheet itself, a view of the tour's App (main.rs)
impl<T: Tour> App<T> {
    /// The "Keyboard shortcuts" app
    pub(super) fn shortcuts_view(&self) -> Element<'_, Message> {
        let cosmic_theme::Spacing {
            space_xxs,
            space_s,
            space_m,
            space_l,
            ..
        } = theme::spacing();
        // Three columns of sections, so that every shortcut shows at once
        let columns = SHORTCUT_COLUMNS.iter().map(|sections| {
            sections
                .iter()
                .map(|&i| {
                    // A compact card per section: every row on screen at once
                    let (title, rows) = SHORTCUTS[i];
                    let mut card = widget::column::with_capacity(rows.len() * 2);
                    for (n, (keys, description)) in rows.iter().enumerate() {
                        if n > 0 {
                            card = card.push(widget::divider::horizontal::light());
                        }
                        card = card.push(
                            widget::row::with_capacity(2)
                                .push(
                                    text::body(tr(description))
                                        .wrapping(Wrapping::Word)
                                        .width(Length::Fill),
                                )
                                .push(shortcut_keys(keys))
                                .spacing(space_s)
                                .align_y(Alignment::Center)
                                .padding([space_xxs, space_s]),
                        );
                    }
                    widget::column::with_capacity(2)
                        .spacing(space_xxs)
                        .push(text::heading(tr(title)))
                        .push(
                            card.apply(widget::container)
                                .padding([space_xxs, 0])
                                .class(theme::Container::Card),
                        )
                })
                .fold(
                    widget::column::with_capacity(2).spacing(space_m),
                    |column, section| column.push(section),
                )
                .width(Length::FillPortion(1))
                .into()
        });
        // The two notes under the columns, beside the button, so that the
        // longest languages still fit without scrolling
        widget::column::with_capacity(2)
            .spacing(space_m)
            .push(
                widget::row::with_children(columns.collect::<Vec<Element<'_, Message>>>())
                    .spacing(space_m),
            )
            .push(
                widget::row::with_capacity(2)
                    .spacing(space_m)
                    .align_y(Alignment::Center)
                    .push(
                        widget::column::with_capacity(2)
                            .spacing(2)
                            .push(
                                text::caption(tr(
                                    "Super is the key with the Windows logo on most keyboards.",
                                ))
                                .wrapping(Wrapping::Word),
                            )
                            .push(text::caption(tr("Escape closes this window.")))
                            .width(Length::Fill),
                    )
                    .push(
                        widget::button::standard(tr("Change them in Settings")).on_press_maybe(
                            in_path("cosmic-settings").then_some(Message::Run(Action::Launch(&[
                                "cosmic-settings",
                                "keyboard",
                            ]))),
                        ),
                    ),
            )
            .padding([space_s, space_l, space_l, space_l])
            .apply(widget::scrollable)
            .into()
    }
}

/// The keys of a sheet's shortcut, with their names translated
fn shortcut_keys(keys: &'static str) -> Element<'static, Message> {
    let mut row = widget::row::with_capacity(5)
        .spacing(6)
        .align_y(Alignment::Center);
    for (i, combination) in keys.split(" / ").enumerate() {
        if i > 0 {
            row = row.push(text::caption("/"));
        }
        let names: Vec<&'static str> = combination
            .split(' ')
            .filter(|word| !word.is_empty() && *word != "+")
            .map(|name| {
                KEY_LABELS
                    .iter()
                    .find(|label| **label == name)
                    .map_or(name, |label| tr(label))
            })
            .collect();
        row = row.push(key_combo(&names));
    }
    row.into()
}

/// One key combination: its modifiers (sorted) and its key, None for a
/// modifier alone (Super opens the launcher)
type Binding = (Vec<String>, Option<String>);

/// The key combinations a line of the sheet stands for, as COSMIC's
/// shortcut files write them: "Super + Shift + ←↑↓→" is four, "Super + 1 …
/// 9" nine, "Super + = / Super + -" two
fn bindings(keys: &str) -> Vec<Binding> {
    let mut all = Vec::new();
    for combination in keys.split(" / ") {
        let mut parts: Vec<&str> = combination.split(" + ").collect();
        let last = parts.pop().unwrap_or_default();
        let mut modifiers: Vec<String> = parts.iter().map(|part| (*part).to_owned()).collect();
        let keys: Vec<Option<String>> = match last {
            "Super" | "Shift" | "Ctrl" | "Alt" => {
                modifiers.push(last.to_owned());
                vec![None]
            }
            "1 … 9" => (1..=9).map(|n| Some(n.to_string())).collect(),
            "←↑↓→" => ["Left", "Up", "Down", "Right"]
                .map(|key| Some(key.to_owned()))
                .to_vec(),
            "←→" => vec![Some("Left".to_owned()), Some("Right".to_owned())],
            "=" => vec![Some("equal".to_owned())],
            "-" => vec![Some("minus".to_owned())],
            "Space" => vec![Some("space".to_owned())],
            key if key.chars().count() == 1 => vec![Some(key.to_lowercase())],
            key => vec![Some(key.to_owned())],
        };
        modifiers.sort();
        for key in keys {
            all.push((modifiers.clone(), key));
        }
    }
    all
}

/// The key combinations of a COSMIC shortcut file (RON):
/// `(modifiers: [Super, Shift], key: "s"): Action` or `(modifiers: [Super]):`
fn parse(file: &str) -> BTreeSet<Binding> {
    let mut found = BTreeSet::new();
    for line in file.lines() {
        let Some(rest) = line.trim().strip_prefix("(modifiers: [") else {
            continue;
        };
        let Some((modifiers, rest)) = rest.split_once(']') else {
            continue;
        };
        let mut modifiers: Vec<String> = modifiers
            .split(',')
            .map(str::trim)
            .filter(|modifier| !modifier.is_empty())
            .map(str::to_owned)
            .collect();
        modifiers.sort();
        let key = rest
            .trim_start()
            .strip_prefix(", key: \"")
            .and_then(|rest| rest.split_once('"'))
            .map(|(key, _)| key.to_owned());
        found.insert((modifiers, key));
    }
    found
}

/// The sheet's lines whose key combinations none of these files has
fn missing(files: &[String]) -> Vec<&'static str> {
    let known: BTreeSet<Binding> = files.iter().flat_map(|file| parse(file)).collect();
    SHORTCUTS
        .iter()
        .flat_map(|(_, rows)| rows.iter())
        .map(|(keys, _)| *keys)
        .filter(|keys| {
            bindings(keys)
                .iter()
                .any(|binding| !known.contains(binding))
        })
        .collect()
}

/// `--check-shortcuts FILE...`: every shortcut the sheet shows is in COSMIC's
/// shortcut files (its defaults and our custom ones), so that the sheet does
/// not go on showing a shortcut a new COSMIC version moved
pub fn check(paths: &[String]) -> bool {
    let mut files = Vec::new();
    for path in paths {
        match std::fs::read_to_string(path) {
            Ok(file) => files.push(file),
            Err(error) => {
                eprintln!("{path}: {error}");
                return false;
            }
        }
    }
    if files.is_empty() {
        eprintln!("Usage: celenn-tour --check-shortcuts FILE...");
        return false;
    }
    let missing = missing(&files);
    for keys in &missing {
        eprintln!("Keyboard shortcuts sheet: {keys} is not a COSMIC shortcut any more");
    }
    missing.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(modifiers: &[&str], key: Option<&str>) -> Binding {
        (
            modifiers.iter().map(|m| (*m).to_owned()).collect(),
            key.map(str::to_owned),
        )
    }

    #[test]
    fn sheet_lines() {
        assert_eq!(bindings("Super"), vec![binding(&["Super"], None)]);
        assert_eq!(
            bindings("Super + Shift + S"),
            vec![binding(&["Shift", "Super"], Some("s"))]
        );
        assert_eq!(bindings("Super + Shift + ←↑↓→").len(), 4);
        assert_eq!(bindings("Super + 1 … 9").len(), 9);
        assert_eq!(
            bindings("Super + = / Super + -"),
            vec![
                binding(&["Super"], Some("equal")),
                binding(&["Super"], Some("minus"))
            ]
        );
    }

    #[test]
    fn shortcut_files() {
        let file = r#"{
    (modifiers: [Super, Shift], key: "s"): System(Screenshot),
    (modifiers: [Super]): System(Launcher),
    (modifiers: [], key: "Print"): System(Screenshot),
}"#;
        let found = parse(file);
        assert!(found.contains(&binding(&["Shift", "Super"], Some("s"))));
        assert!(found.contains(&binding(&["Super"], None)));
        assert!(found.contains(&binding(&[], Some("Print"))));
        assert_eq!(found.len(), 3);
    }

    #[test]
    fn missing_shortcuts() {
        // Only Super + Q: everything else of the sheet is missing
        let only = vec![r#"(modifiers: [Super], key: "q"): Close,"#.to_owned()];
        let missing = missing(&only);
        assert!(!missing.contains(&"Super + Q"));
        assert!(missing.contains(&"Super + W"));
    }
}
