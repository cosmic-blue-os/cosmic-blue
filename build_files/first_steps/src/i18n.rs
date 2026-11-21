// Translations shared by cosmic-blue's libcosmic apps (the tours, Updates):
// gettext PO files read at build time. Each program (crate root) defines
//   LANGUAGES      (code, PO file contents), one per language
//   TEMPLATE       contents of its .pot file, the list of English texts
//   PO_DIR, TEMPLATE_NAME  where they are, for the messages of `check`

use crate::{LANGUAGES, PO_DIR, TEMPLATE, TEMPLATE_NAME};
use std::sync::OnceLock;

/// The translations of a PO file: (English text, translated text), without
/// the header. Only what our files use: msgid and msgstr strings, on one or
/// more lines, with \" \\ and \n escapes.
pub fn parse_po(po: &str) -> Vec<(String, String)> {
    fn unquote(line: &str) -> Option<String> {
        let inner = line.trim().strip_prefix('"')?.strip_suffix('"')?;
        let mut out = String::new();
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => out.push('\n'),
                    Some(other) => out.push(other),
                    None => {}
                }
            } else {
                out.push(c);
            }
        }
        Some(out)
    }
    let mut entries = Vec::new();
    let (mut msgid, mut msgstr) = (None::<String>, None::<String>);
    let mut flush = |msgid: &mut Option<String>, msgstr: &mut Option<String>| {
        if let (Some(id), Some(text)) = (msgid.take(), msgstr.take()) {
            if !id.is_empty() {
                entries.push((id, text));
            }
        }
    };
    for line in po.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("msgid ") {
            flush(&mut msgid, &mut msgstr);
            msgid = unquote(rest);
        } else if let Some(rest) = line.strip_prefix("msgstr ") {
            msgstr = unquote(rest);
        } else if line.starts_with('"') {
            let target = if msgstr.is_some() {
                &mut msgstr
            } else {
                &mut msgid
            };
            if let (Some(text), Some(more)) = (target.as_mut(), unquote(line)) {
                text.push_str(&more);
            }
        }
    }
    flush(&mut msgid, &mut msgstr);
    entries
}

/// The index in LANGUAGES of a locale name ("de_CH.UTF-8"): the same
/// language and region, else the same language
fn find_language(name: &str) -> Option<usize> {
    let name = name.split(['.', '@']).next().unwrap_or_default();
    let lang = name.split('_').next().unwrap_or_default();
    LANGUAGES
        .iter()
        .position(|(code, _)| *code == name)
        .or_else(|| LANGUAGES.iter().position(|(code, _)| *code == lang))
        .or_else(|| {
            LANGUAGES
                .iter()
                .position(|(code, _)| code.split('_').next() == Some(lang))
        })
}

/// The session language, when it has a translation: its index in LANGUAGES
fn language_index() -> Option<usize> {
    static LANGUAGE: OnceLock<Option<usize>> = OnceLock::new();
    *LANGUAGE.get_or_init(|| {
        // Same order as gettext: LANGUAGE (a list), LC_ALL, LC_MESSAGES, LANG
        let mut names = Vec::new();
        if let Ok(list) = std::env::var("LANGUAGE") {
            names.extend(list.split(':').map(str::to_owned));
        }
        for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(value) = std::env::var(var) {
                names.push(value);
            }
        }
        for name in names.iter().filter(|name| !name.is_empty()) {
            if let Some(index) = find_language(name) {
                return Some(index);
            }
            let lang = name.split(['_', '.', '@']).next().unwrap_or_default();
            if matches!(lang, "en" | "C" | "POSIX") {
                return None;
            }
        }
        None
    })
}

/// The session language code ("fr", "pt_BR"), when it has a translation
#[allow(dead_code)] // (used by the tours and Tools, not by Parental controls)
pub fn language() -> Option<&'static str> {
    language_index().map(|index| LANGUAGES[index].0)
}

/// Translates an English text to the session language (English when the
/// text has no translation)
pub fn tr(text: &'static str) -> &'static str {
    static TRANSLATIONS: OnceLock<Vec<(String, String)>> = OnceLock::new();
    let Some(index) = language_index() else {
        return text;
    };
    TRANSLATIONS
        .get_or_init(|| parse_po(LANGUAGES[index].1))
        .iter()
        .find(|(english, translated)| english == text && !translated.is_empty())
        .map_or(text, |(_, translated)| translated.as_str())
}

/// Image build test: every text of the program is in the template and
/// translated in every language, and no file has texts that are no longer
/// used
pub fn check(texts: &[&str]) -> bool {
    let mut errors = 0;
    let template: Vec<String> = parse_po(TEMPLATE).into_iter().map(|(id, _)| id).collect();
    for text in texts {
        if !template.iter().any(|id| id == text) {
            eprintln!("Not in {PO_DIR}/{TEMPLATE_NAME}: {text}");
            errors += 1;
        }
    }
    for id in &template {
        if !texts.contains(&id.as_str()) {
            eprintln!("No longer used, remove it from {PO_DIR}/{TEMPLATE_NAME}: {id}");
            errors += 1;
        }
    }
    for (code, po) in LANGUAGES {
        let entries = parse_po(po);
        for text in texts {
            if !entries.iter().any(|(id, tr)| id == text && !tr.is_empty()) {
                eprintln!("No {code} translation for: {text}");
                errors += 1;
            }
        }
        for (id, _) in &entries {
            if !texts.contains(&id.as_str()) {
                eprintln!("No longer used, remove it from {PO_DIR}/{code}.po: {id}");
                errors += 1;
            }
        }
    }
    println!(
        "{} texts, {} languages, {errors} errors",
        texts.len(),
        LANGUAGES.len()
    );
    errors == 0
}
