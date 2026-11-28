// The first steps tour: texts with keyboard shortcuts, the keys drawn as
// keycaps ("Super + A")

use super::*;

/// Names of keys found in the texts (and their translations), drawn as
/// keyboard keys when they are part of a combination such as "Super + A"
pub(super) const KEY_NAMES: &[&str] = &[
    "Super", "Alt", "Ctrl", "Strg", "Shift", "Maj", "Umschalt", "Mayús", "Maiusc", "Tab", "Esc",
    "Escape", "Échap", "Enter", "Print", "←", "↑", "↓", "→",
];

pub(super) fn is_key(word: &str) -> bool {
    KEY_NAMES.contains(&word) || (word.chars().count() == 1 && !matches!(word, "+" | "/" | "…"))
}

/// The pieces of a line: plain words, and key combinations (their keys)
#[derive(Debug, PartialEq)]
pub(super) enum Piece<'s> {
    Word(&'s str),
    Keys(Vec<&'s str>),
}

pub(super) fn pieces(line: &str) -> Vec<Piece<'_>> {
    let words: Vec<&str> = line.split(' ').filter(|word| !word.is_empty()).collect();
    let mut pieces = Vec::new();
    let mut i = 0;
    while i < words.len() {
        // A key, then "+" and a key, maybe more: a combination
        let strip = |word: &str| -> usize {
            let trimmed = word.trim_end_matches([',', '.', ':', ';', ')']);
            let key = if trimmed.is_empty() { word } else { trimmed };
            key.len()
        };
        let starts = is_key(words[i])
            && words.get(i + 1) == Some(&"+")
            && words
                .get(i + 2)
                .is_some_and(|next| is_key(&next[..strip(next)]));
        if !starts {
            pieces.push(Piece::Word(words[i]));
            i += 1;
            continue;
        }
        let mut keys = vec![words[i]];
        let mut rest = None;
        i += 1;
        while words.get(i) == Some(&"+") {
            let Some(next) = words.get(i + 1) else { break };
            let len = strip(next);
            if !is_key(&next[..len]) {
                break;
            }
            keys.push(&next[..len]);
            i += 2;
            if len < next.len() {
                rest = Some(&next[len..]);
                break;
            }
            // Arrows or a range after the last "+": "← ↑ ↓ →", "1 … 9"
            while rest.is_none()
                && words.get(i).is_some_and(|word| {
                    matches!(*word, "←" | "↑" | "↓" | "→" | "…")
                        || (keys.last().is_some_and(|last| *last == "…")
                            && word.chars().all(|c| c.is_ascii_digit()))
                })
            {
                keys.push(words[i]);
                i += 1;
            }
        }
        pieces.push(Piece::Keys(keys));
        if let Some(rest) = rest {
            pieces.push(Piece::Word(rest));
        }
    }
    pieces
}

/// A combination of keys: "Super + A" as two keys joined by "+"
pub(super) fn key_combo<'a>(keys: &[&'a str]) -> Element<'a, Message> {
    let mut row = widget::row::with_capacity(keys.len() * 2)
        .spacing(3)
        .align_y(Alignment::Center);
    let mut previous: Option<&str> = None;
    for key in keys {
        // "+" between the keys, but not inside a group of arrows or a range
        let grouped = previous.is_some_and(|p| matches!(p, "←" | "↑" | "↓" | "→" | "…"))
            || matches!(*key, "↑" | "↓" | "→" | "…")
            || previous == Some("…");
        if previous.is_some() && !grouped {
            row = row.push(text::caption("+"));
        }
        row = row.push(if *key == "…" {
            text::caption("…").into()
        } else {
            crate::style::keycap(key)
        });
        previous = Some(key);
    }
    row.into()
}

/// A row's title; "A → B" shows A, an arrow and B, and keys as keys
pub(super) fn title_line<'a>(title: &'a str) -> Element<'a, Message> {
    let part = |part: &'a str| -> Element<'a, Message> {
        let pieces = pieces(part);
        if !pieces.iter().any(|piece| matches!(piece, Piece::Keys(_))) {
            return text::heading(part).into();
        }
        pieces
            .into_iter()
            .fold(
                widget::row::with_capacity(3)
                    .spacing(4)
                    .align_y(Alignment::Center),
                |row, piece| match piece {
                    Piece::Word(word) => row.push(text::heading(word)),
                    Piece::Keys(keys) => row.push(key_combo(&keys)),
                },
            )
            .into()
    };
    match title.split_once(" → ") {
        Some((from, to)) => widget::row::with_capacity(3)
            .spacing(8)
            .align_y(Alignment::Center)
            .push(part(from))
            .push(widget::icon::from_name("go-next-symbolic").size(16))
            .push(part(to))
            .into(),
        None => part(title),
    }
}

/// A caption where key combinations show as keys; each line of the text is
/// a line on screen, and long lines wrap between words
pub(super) fn with_keys<'a>(caption: &'a str) -> Element<'a, Message> {
    if !caption
        .lines()
        .any(|line| pieces(line).iter().any(|p| matches!(p, Piece::Keys(_))))
    {
        return dim_caption(caption).wrapping(Wrapping::Word).into();
    }
    caption
        .lines()
        .fold(
            widget::column::with_capacity(2).spacing(4),
            |column, line| {
                let words: Vec<Element<'a, Message>> = pieces(line)
                    .into_iter()
                    .map(|piece| match piece {
                        Piece::Word(word) => dim_caption(word).into(),
                        Piece::Keys(keys) => key_combo(&keys),
                    })
                    .collect();
                column.push(
                    widget::flex_row(words)
                        .column_spacing(4)
                        .row_spacing(4)
                        .align_items(Alignment::Center),
                )
            },
        )
        .width(Length::Fill)
        .into()
}
