// Web apps (webapps.rs): their icons. A web app's icon is its site's own
// (its "apple-touch-icon" or favicon, read by webapp_site.rs), or a round
// letter when the site has none; framed with the margin that the system's
// icons have (a site's logo fills its whole square), at the icon theme's
// place (webapp_launcher.rs). The popular sites' icons are fetched once
// into ~/.cache/celenn/site-icons when the tool opens: nothing is shipped
// in the image.

use crate::webapp_launcher::{ID_PREFIX, app_id, icon_paths};
use crate::webapp_site::{absolute, attribute, download, fetch, icon_candidates};
use crate::webapps::SITES;
use std::path::{Path, PathBuf};

/// How much of an icon's square its picture fills (as the system's icons)
pub const ICON_FILL: f32 = 0.82;

/// Where a site's icon is kept, fetched from the site itself (nothing is
/// shipped in the image): PNG, SVG, and a mark saying the site has none
pub fn cache_paths(url: &str) -> Option<[PathBuf; 3]> {
    let id = app_id(url)?;
    let name = id.strip_prefix(ID_PREFIX).unwrap_or(&id);
    let cache = crate::system::xdg("XDG_CACHE_HOME", ".cache").join("celenn/site-icons");
    Some([
        cache.join(format!("{name}.png")),
        cache.join(format!("{name}.svg")),
        cache.join(format!("{name}.none")),
    ])
}

/// A site's icon in the cache
pub fn cached_icon(url: &str) -> Option<PathBuf> {
    let [png, svg, _] = cache_paths(url)?;
    [png, svg].into_iter().find(|path| path.is_file())
}

/// The site's icon is not in the cache, and the site was not asked for it
/// since yesterday
pub fn icon_missing(url: &str) -> bool {
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

pub fn site_icons() -> Vec<Option<PathBuf>> {
    SITES.iter().map(|(_, url, _)| cached_icon(url)).collect()
}

/// Fetches the popular sites' missing icons, all at once
pub fn cache_icons() {
    // (icons of older versions, not framed)
    let cache = crate::system::xdg("XDG_CACHE_HOME", ".cache");
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

pub fn cache_icon(url: &str) {
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
    } else {
        let _ = std::fs::write(&none, "");
    }
}

/// The site's icon at the icon theme's place: the one in the cache, or its
/// best icon of at least 48 pixels, or else a round letter
pub fn install_icon(
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
pub fn site_icon(
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

pub fn save_png(picture: &image::DynamicImage, path: &Path) -> std::io::Result<()> {
    framed(picture)
        .save_with_format(path, image::ImageFormat::Png)
        .map_err(std::io::Error::other)
}

/// An icon file framed (PNG or SVG) into `png` or `svg`
pub fn frame_file(icon: &Path, png: &Path, svg: &Path) -> std::io::Result<()> {
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
pub fn framed(picture: &image::DynamicImage) -> image::RgbaImage {
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
pub fn round_corners(picture: &mut image::RgbaImage, radius: f32) {
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
pub fn without_attribute(tag: &str, name: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn letters_for_sites_without_icon() {
        assert!(letter_icon("la poste").contains(">L</text>"));
    }
}
