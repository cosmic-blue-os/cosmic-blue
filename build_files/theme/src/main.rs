//! Writes the Celenn OS default COSMIC themes (dark and light) with the
//! frosted glass effect turned on, to $XDG_CONFIG_HOME/cosmic.
//!
//! COSMIC apps read the computed theme (com.system76.CosmicTheme.*), not the
//! builder that COSMIC Settings edits, and the frosted glass switches change
//! the computed colors (transparency). So both are generated with the same
//! code as COSMIC Settings, and shipped as system defaults in /usr/share/cosmic.
//!
//! The colors revolve around the "cosmic" blue of the logo and wallpaper:
//! - the accent is a blue from the logo instead of COSMIC's default teal;
//! - the window background is a deep night blue instead of neutral gray, and
//!   the controls get a slight tint of the same hue (COSMIC derives every
//!   surface from these, keeping hue and chroma, so they stay discreet).
//!
//! The window hint (the outline of the active window) is left unset, so it
//! follows the accent, also when the user picks another accent in COSMIC
//! Settings. Every color can still be changed there (Appearance).

use cosmic_config::{ConfigSet, CosmicConfigEntry};
use cosmic_theme::palette::{Srgb, Srgba};
use cosmic_theme::{AlphaMap, BlurStrength, Theme, ThemeBuilder};

/// Dark theme accent: the middle of the logo orbit gradient (#4f8dff).
const ACCENT_DARK: Srgb<u8> = Srgb::new(0x4f, 0x8d, 0xff);
/// Dark theme accent of the images of 2026-11-26 (#266, reverted: the cyan
/// side felt harsh and not "space" enough)
const ACCENT_DARK_3A9DFF: Srgb<u8> = Srgb::new(0x3a, 0x9d, 0xff);
/// Light theme accent: the deep blue of the logo planet (#1d47c4), dark
/// enough to stay readable on light surfaces.
const ACCENT_LIGHT: Srgb<u8> = Srgb::new(0x1d, 0x47, 0xc4);

/// Window background, dark theme: night blue at the lightness of COSMIC's
/// default dark gray (oklch 0.225 0.03, hue of the accent).
const BG_DARK: Srgb<u8> = Srgb::new(0x14, 0x1c, 0x2a);
/// Window background, light theme: a cool, barely blue gray at the lightness
/// of COSMIC's default light gray (oklch 0.88 0.012).
const BG_LIGHT: Srgb<u8> = Srgb::new(0xd3, 0xd8, 0xe0);
/// Tint of buttons and other controls (oklch chroma 0.03, hue of the accent;
/// COSMIC only keeps its hue and chroma).
const NEUTRAL_TINT: Srgb<u8> = Srgb::new(0x5a, 0x64, 0x75);

fn colors(builder: ThemeBuilder, accent: Srgb<u8>, bg: Srgb<u8>) -> ThemeBuilder {
    let bg: Srgb = bg.into_format();
    builder
        .accent(accent.into_format())
        .bg_color(Srgba::new(bg.red, bg.green, bg.blue, 1.0))
        .neutral_tint(NEUTRAL_TINT.into_format())
}

/// The same switches as "Frosted glass" in COSMIC's first-run assistant.
fn frosted(mut builder: ThemeBuilder) -> ThemeBuilder {
    builder.frosted_windows = true;
    builder.frosted_system_interface = true;
    builder.frosted_panel = true;
    builder.frosted_applets = true;
    builder
}

/// Frosted glass settings of COSMIC Settings (Appearance > Frosted glass).
///
/// The default wallpaper is a fairly even blue with thin bright arcs, close
/// to the night blue of windows. With COSMIC's defaults (Thickness Medium,
/// Glass opacity 0.5, about 75 % opaque) the blur erases the arcs and the
/// surface looks plain, not like glass. So:
/// - "Thickness" Low: the arcs stay visible as soft lines behind windows,
///   the panel and the dock, which is what makes the glass readable;
/// - "Glass opacity" 0.2 in the dark theme (surfaces about 45 % opaque,
///   headers and cards about 75 %), 0.35 in the light theme (about 63 %),
///   which keeps dark text readable over the dark wallpaper.
///
/// The panel and the dock use the theme's background, so the same values
/// give windows, panel and dock the same glass.
const THICKNESS: BlurStrength = BlurStrength::Low;
const GLASS_OPACITY_DARK: f32 = 0.2;
const GLASS_OPACITY_LIGHT: f32 = 0.35;

/// Frosted glass of earlier images: (thickness, glass opacity)
const GLASS_0_8: (BlurStrength, f32) = (BlurStrength::High, 0.8); // until 2026-11-19
const GLASS_0_65: (BlurStrength, f32) = (BlurStrength::High, 0.65); // until 2026-11-21

/// The alpha map COSMIC Settings writes for a "Glass opacity" value (same
/// formula as cosmic-settings 1.11.0 and 1.12.0, appearance/mod.rs,
/// Message::Glass).
fn glass_alpha_map(glass: f32) -> AlphaMap {
    let alpha_map = AlphaMap::default();
    let offset = (glass - 0.5) / 0.5
        * if glass < 0.5 {
            alpha_map.extremely_high_2
        } else {
            1.0 - alpha_map.extremely_low
        };
    alpha_map.offset(offset)
}

fn frosted_glass(mut builder: ThemeBuilder, thickness: BlurStrength, glass: f32) -> ThemeBuilder {
    builder.frosted = thickness;
    builder.alpha_map = glass_alpha_map(glass);
    builder
}

/// The Celenn OS dark and light themes with the given dark accent and
/// frosted glass (thickness, glass opacity of the dark theme, of the light
/// theme).
fn celenn(
    accent_dark: Srgb<u8>,
    thickness: BlurStrength,
    glass_dark: f32,
    glass_light: f32,
) -> [ThemeBuilder; 2] {
    [
        frosted_glass(
            colors(frosted(ThemeBuilder::dark()), accent_dark, BG_DARK),
            thickness,
            glass_dark,
        )
        .icon_theme(ICON_THEME),
        frosted_glass(
            colors(frosted(ThemeBuilder::light()), ACCENT_LIGHT, BG_LIGHT),
            thickness,
            glass_light,
        )
        .icon_theme(ICON_THEME),
    ]
}

/// The icon theme, part of the theme since COSMIC 1.12 (libcosmic 4c853eb;
/// COSMIC 1.11 read it from com.system76.CosmicTk, still shipped in
/// system_files for older accounts).
const ICON_THEME: &str = "celenn";

/// The names COSMIC Settings shows for the default themes (Appearance), and
/// the constellations (theme bundles) that hold them. The same in every
/// language.
const NAME_DARK: &str = "Celenn Dark";
const NAME_LIGHT: &str = "Celenn Light";
const SLUG_DARK: &str = "celenn-dark";
const SLUG_LIGHT: &str = "celenn-light";

fn named(mut builder: ThemeBuilder, name: &str) -> ThemeBuilder {
    builder.palette.as_mut().name = name.to_owned();
    builder
}

/// The constellations (COSMIC theme bundles) "Celenn Dark" and "Celenn
/// Light", in the form that COSMIC Settings lists as the system's own
/// themes (the same as System76's, pop-os/cosmic-theme-bundles): a preset
/// ("base") of the theme and of its builder, and a package id. Their
/// wallpaper (the Background part) is a copy of our per-theme default,
/// added by build_files/10-system-files.sh.
///
/// They hold exactly the default themes: COSMIC Settings compares the live
/// theme with them, and would otherwise save the default as an extra theme.
/// The frosted glass settings are not part of a constellation: COSMIC keeps
/// them when another one is picked.
fn bundles(dark: &ThemeBuilder, light: &ThemeBuilder) {
    for (slug, builder) in [(SLUG_DARK, dark), (SLUG_LIGHT, light)] {
        let id = format!("com.system76.CosmicThemeBundle.{slug}");
        let config = |part: &str, version| {
            cosmic_config::Config::new(&format!("{id}.{part}"), version).expect("bundle config")
        };
        config("Theme", Theme::VERSION)
            .set("base", builder.clone().build())
            .expect("write bundle theme");
        config("Builder", ThemeBuilder::VERSION)
            .set("base", builder)
            .expect("write bundle builder");
        let package = config("Package", 1);
        package
            .set("package_id", &id)
            .expect("write bundle package id");
        package
            .set("version", "1.0.0")
            .expect("write bundle package version");
    }
}

/// Writes a dark and a light theme (builder and computed theme).
fn write(dark: ThemeBuilder, light: ThemeBuilder) {
    dark.write_entry(&ThemeBuilder::dark_config().expect("dark builder config"))
        .expect("write dark builder");
    dark.build()
        .write_entry(&Theme::dark_config().expect("dark theme config"))
        .expect("write dark theme");

    light
        .write_entry(&ThemeBuilder::light_config().expect("light builder config"))
        .expect("write light builder");
    light
        .build()
        .write_entry(&Theme::light_config().expect("light theme config"))
        .expect("write light theme");
}

/// Without an argument: the Celenn OS themes. With "stock", "frosted",
/// "glass-0.8", "glass-0.65" or "accent-3a9dff": themes from earlier images,
/// which accounts may still hold as a copy (COSMIC's own default, written by
/// the first-run assistant, the same with frosted glass, the default before
/// the blue colors, the blue themes with earlier frosted glass settings, and
/// the short-lived cyan accent). They go into
/// /usr/share/celenn/previous-defaults.list, so that
/// celenn-migrate-defaults can tell them from a user's own choice.
fn main() {
    match std::env::args().nth(1).as_deref() {
        None => {
            let [dark, light] = celenn(
                ACCENT_DARK,
                THICKNESS,
                GLASS_OPACITY_DARK,
                GLASS_OPACITY_LIGHT,
            );
            let (dark, light) = (named(dark, NAME_DARK), named(light, NAME_LIGHT));
            write(dark.clone(), light.clone());
            bundles(&dark, &light);
        }
        Some("accent-3a9dff") => write_pair(celenn(
            ACCENT_DARK_3A9DFF,
            THICKNESS,
            GLASS_OPACITY_DARK,
            GLASS_OPACITY_LIGHT,
        )),
        Some("glass-0.8") => write_pair(celenn(ACCENT_DARK, GLASS_0_8.0, GLASS_0_8.1, GLASS_0_8.1)),
        Some("glass-0.65") => write_pair(celenn(
            ACCENT_DARK,
            GLASS_0_65.0,
            GLASS_0_65.1,
            GLASS_0_65.1,
        )),
        Some("stock") => write(ThemeBuilder::dark(), ThemeBuilder::light()),
        Some("frosted") => write(
            frosted(ThemeBuilder::dark()),
            frosted(ThemeBuilder::light()),
        ),
        Some(other) => panic!("unknown theme variant {other}"),
    }
}

fn write_pair([dark, light]: [ThemeBuilder; 2]) {
    write(dark, light);
}
