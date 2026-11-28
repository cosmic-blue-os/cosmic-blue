// The text styles of Celenn OS's apps (the tours, Tools, Parental
// controls), so that their rows look alike: a row's title in semibold and
// its caption dimmed, as in COSMIC Settings.

// (Each program uses its part)
#![allow(dead_code)]

use cosmic::theme;
use cosmic::widget::{self, text};
use std::borrow::Cow;

/// A row's title
pub fn row_title<'a>(title: impl Into<Cow<'a, str>> + 'a) -> widget::Text<'a, cosmic::Theme> {
    text::heading(title)
}

/// A row's caption, dimmed under its title
pub fn dim_caption<'a>(caption: impl Into<Cow<'a, str>> + 'a) -> widget::Text<'a, cosmic::Theme> {
    text::caption(caption).class(theme::Text::Custom(|theme| {
        let mut color = theme.cosmic().on_bg_color();
        color.alpha = 0.72;
        cosmic::iced::widget::text::Style {
            color: Some(color.into()),
            ..Default::default()
        }
    }))
}

/// A picture's name from its file, for screen readers:
/// "celenn-shooting-star.jpg" -> "shooting star"
pub fn image_name(path: &std::path::Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy())
        .unwrap_or_default()
        .trim_start_matches("celenn-")
        .replace(['-', '_'], " ")
}

/// A keyboard key: its name in a small rounded box (the tour's tips, the
/// shortcuts sheet, the quick menu)
pub fn keycap<'a, M: 'a>(name: &'a str) -> cosmic::Element<'a, M> {
    use cosmic::Apply;
    text::caption(name)
        .apply(widget::container)
        .padding([1, 6])
        .class(theme::Container::custom(|theme| {
            let cosmic = theme.cosmic();
            let mut background = cosmic.on_bg_color();
            background.alpha = 0.1;
            let mut border = cosmic.on_bg_color();
            border.alpha = 0.35;
            cosmic::iced::widget::container::Style {
                background: Some(cosmic::iced::Color::from(background).into()),
                border: cosmic::iced::Border {
                    color: border.into(),
                    width: 1.0,
                    radius: 5.0.into(),
                },
                ..Default::default()
            }
        }))
        .into()
}

/// The top of a page, as in COSMIC Settings: its picture (an icon in the
/// style of the dock's buttons, or a person's picture), its title and a
/// dimmed caption, on the left, so that the page's rows come right after
pub fn page_header<'a, M: 'static>(
    picture: cosmic::Element<'a, M>,
    title: impl Into<Cow<'a, str>> + 'a,
    caption: Option<String>,
) -> cosmic::Element<'a, M> {
    use cosmic::iced::core::text::Wrapping;
    use cosmic::iced::{Alignment, Length};
    let lines = widget::column::with_capacity(2)
        .spacing(2)
        .push(text::title3(title))
        .push_maybe(caption.map(|caption| dim_caption(caption).wrapping(Wrapping::Word)))
        .width(Length::Fill);
    // Top-aligned: the picture and the title stay at the same place on
    // every page, however many lines the caption has
    widget::row::with_capacity(2)
        .spacing(theme::spacing().space_s)
        .align_y(Alignment::Start)
        .push(picture)
        .push(lines)
        .width(Length::Fill)
        .into()
}

/// A tile (the tour's origin cards, the kinds of apps): a settings-row
/// background, an accent border when chosen, a little lighter under the
/// mouse (a custom style is told whether
/// the button has the keyboard focus, not whether it is selected)
pub fn tile_style(selected: bool) -> theme::Button {
    fn style(selected: bool, theme: &cosmic::Theme, hover: bool) -> cosmic::widget::button::Style {
        let cosmic = theme.cosmic();
        let component = &theme.current_container().component;
        let mut style = cosmic::widget::button::Style::new();
        style.background = Some(cosmic::iced::Background::Color(
            if hover {
                component.hover
            } else {
                component.base
            }
            .into(),
        ));
        style.border_radius = cosmic.corner_radii.radius_s.into();
        style.text_color = Some(component.on.into());
        style.icon_color = Some(if selected {
            cosmic.accent.base.into()
        } else {
            component.on.into()
        });
        if selected {
            style.border_width = 2.0;
            style.border_color = cosmic.accent.base.into();
        }
        style
    }
    theme::Button::Custom {
        active: Box::new(move |_, theme| style(selected, theme, false)),
        disabled: Box::new(move |theme| style(selected, theme, false)),
        hovered: Box::new(move |_, theme| style(selected, theme, true)),
        pressed: Box::new(move |_, theme| style(selected, theme, true)),
    }
}

/// How round the corners of a person's picture are, as a share of its side:
/// the same shape at every size (9 px at 43 px, like the wallpapers'
/// thumbnails in the tour)
const CORNER: f32 = 0.21;

/// The corner radius of a person's picture `size` pixels wide
fn corner(size: f32) -> f32 {
    (size * CORNER).round()
}

/// A person's picture (a PNG or JPEG file) as a square with rounded corners
/// and a smooth edge, at `size` pixels a side: the image widget's own
/// rounding has a jagged edge. COSMIC shows these pictures round; the login
/// screen keeps them round too. None when the file cannot be read.
pub fn rounded_picture(path: &std::path::Path, size: u32) -> Option<widget::image::Handle> {
    let picture = image::open(path).ok()?;
    let mut picture = picture
        .resize_to_fill(size, size, image::imageops::FilterType::CatmullRom)
        .into_rgba8();
    for (x, y, pixel) in picture.enumerate_pixels_mut() {
        let coverage = rounded_coverage(size as f32, x as f32 + 0.5, y as f32 + 0.5);
        pixel[3] = (f32::from(pixel[3]) * coverage).round() as u8;
    }
    Some(widget::image::Handle::from_rgba(
        size,
        size,
        picture.into_raw(),
    ))
}

/// How much of the pixel centred on (x, y) lies inside a rounded square
/// `size` pixels wide (0 to 1): its signed distance to the edge, so that
/// the edge is smooth
fn rounded_coverage(size: f32, x: f32, y: f32) -> f32 {
    let half = size / 2.0;
    let radius = corner(size);
    let qx = (x - half).abs() - (half - radius);
    let qy = (y - half).abs() - (half - radius);
    let outside = qx.max(0.0).hypot(qy.max(0.0));
    let distance = outside + qx.max(qy).min(0.0) - radius;
    (0.5 - distance).clamp(0.0, 1.0)
}

/// A button for a person's picture, `size` pixels wide: the selected (or
/// focused) one outlined in the accent color, a fainter outline on hover,
/// drawn over the picture's edge in its rounded shape, as the image buttons
/// of the wallpapers
pub fn picture_button<'a, M: Clone + 'a>(
    handle: widget::image::Handle,
    size: u16,
    selected: bool,
    on_press: M,
) -> widget::Button<'a, M> {
    let ring = move |width: f32, alpha: f32, theme: &cosmic::Theme| {
        let mut style = widget::button::Style::new();
        style.border_radius = corner(f32::from(size)).into();
        style.border_width = width;
        let mut color: cosmic::iced::Color = theme.cosmic().accent.base.into();
        color.a = alpha;
        style.border_color = color;
        style
    };
    widget::button::custom(widget::image(handle).width(size).height(size))
        .class(theme::Button::Custom {
            active: Box::new(move |focused, theme| {
                ring(if selected || focused { 2.0 } else { 0.0 }, 1.0, theme)
            }),
            disabled: Box::new(move |theme| ring(0.0, 1.0, theme)),
            hovered: Box::new(move |_, theme| ring(2.0, if selected { 1.0 } else { 0.5 }, theme)),
            pressed: Box::new(move |_, theme| ring(2.0, 1.0, theme)),
        })
        .padding(0)
        .on_press(on_press)
}

#[cfg(test)]
mod tests {
    use super::rounded_coverage;

    #[test]
    fn rounded_square_edges() {
        // The middle and the middle of each side are covered
        assert_eq!(rounded_coverage(43.0, 21.5, 21.5), 1.0);
        assert_eq!(rounded_coverage(43.0, 0.5, 21.5), 1.0);
        assert_eq!(rounded_coverage(43.0, 21.5, 42.5), 1.0);
        // The corner pixel is outside, the rounding is symmetric
        assert_eq!(rounded_coverage(43.0, 0.5, 0.5), 0.0);
        assert_eq!(
            rounded_coverage(43.0, 2.5, 4.5),
            rounded_coverage(43.0, 40.5, 38.5)
        );
        // The edge of the rounding is partly covered (smooth)
        let edge = rounded_coverage(43.0, 1.5, 4.5);
        assert!(edge > 0.0 && edge < 1.0, "{edge}");
    }
}
