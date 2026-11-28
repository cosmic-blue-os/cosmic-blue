// Web apps (webapps.rs): reading a site. Its page (with curl, as a browser
// would ask), its name, and the addresses of its icons, best first. The
// page is read with a few patterns rather than an HTML parser: only <link>,
// <meta> and <title> tags of the head matter, and the unit tests below keep
// the patterns honest on the forms sites use.

/// The page of a site and the address it ended on (after redirections)
pub fn fetch(url: &str) -> Option<(String, String)> {
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
pub fn download(url: &str) -> Option<Vec<u8>> {
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
pub fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
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
pub fn tags<'a>(html: &'a str, kind: &str) -> Vec<&'a str> {
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
pub fn unescape(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&quot;", "\"")
        .replace("&nbsp;", " ")
}

/// The site's name: its application-name, og:site_name, or the start of its
/// title
pub fn site_name(html: &str) -> Option<String> {
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
pub fn name_from_host(host: &str) -> String {
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
pub fn absolute(base: &str, href: &str) -> Option<String> {
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
pub fn icon_candidates(html: Option<&str>, base: &str) -> Vec<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
