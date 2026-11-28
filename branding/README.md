# Celenn OS branding

## Logo

The "orbit c": an open orbit around a planet, with a moon in the gap. The
wordmark "celenn" is Montserrat Light, stored as outlines in `logo/glyphs.json`
(written by `logo/glyphs.py`) so the SVGs need no font. The same file holds the
full name "Celenn OS", which the default wallpaper and the boot screen show.

- `logo/celenn-logo.svg`: the mark (installed as the `celenn-logo` icon)
- `logo/celenn-logo-symbolic.svg`: the mark in one color (installed as
  `celenn-logo-symbolic`, the icon of the Celenn OS group in the App
  Library)
- `logo/celenn-readme-banner.svg`: the README header, the logo and the
  name in white on the night sky of the tour (`tour/build.py`)
- `logo/celenn-logo.png`: the mark, 512 px (Artifact Hub)
- `logo/celenn-logo-horizontal-dark.svg` / `-light.svg`: mark + wordmark,
  for dark and light backgrounds
- `logo/celenn-os-logo-horizontal-dark.svg`: mark + "Celenn OS", for dark
  backgrounds; its text alone is the name on the boot screen and in the
  default wallpaper's corner

## Boot screen

The `Celenn OS` Plymouth theme, in
`system_files/usr/share/plymouth/themes/celenn/`. It is Fedora's BGRT
theme (the PC maker's logo from the firmware, for a flicker-free boot) with
our images:

- `bgrt-fallback.png`: the mark at 192x192, shown in the middle when the
  firmware has no logo (virtual machines, some PCs)
- `watermark.png`: "Celenn OS" alone, 70 px high, at the bottom (without
  the mark: it is a "C", which read as a double C before the name)

Regenerate both with `node plymouth/render.js` (Playwright).
- the spinner: Fedora's frames tinted with the orbit blues at build time
  (`build_files/plymouth/tint.py`)

The theme file itself is generated from Fedora's `bgrt.plymouth` during the
build (`build_files/20-cosmic.sh`), so it follows Fedora's settings.

Regenerate the SVGs with `python3 logo/build.py`.

## Wallpaper

All 5120x2880, in `system_files/usr/share/backgrounds/celenn/`:

- `celenn-arcs.jpg`: the logo mark, left of centre, its orbit
  echoing outwards in fainter arcs, and the name "Celenn OS" small near the
  bottom right corner, whole on 16:9, 16:10 and 3:2 screens once COSMIC crops the
  wallpaper to fill (4:3, 5:4 and 21:9 screens cut part of it), with no stars
  next to it (`wallpaper/shaders/echoes.frag`, `wallpaper/logo_mark.py`), the
  default COSMIC wallpaper and the greeter background
- `celenn-deep-spiral.jpg`: a flat illustration, a calm night-blue field
  with a large tone-on-tone spiral and small round stars, offered in Settings
- `celenn-eclipse.jpg`: a deep blue planet in front of its star, ringed with
  blue light and soft rays, offered in Settings
- `celenn-galaxy.jpg`: a tilted spiral galaxy, offered in Settings
- `celenn-galactic-river.jpg`: a diagonal river of stars and gas, offered in
  Settings
- `celenn-gas-veils.jpg`: veils of blue gas around a star cluster, offered
  in Settings
- `celenn-line-orbits.jpg`: line art, bundles of orbit lines from two
  corners, offered in Settings
- `celenn-ringed-planet.jpg`: a giant ringed planet towards the lower
  right, its thin rings sweeping across the frame, offered in Settings
- `celenn-orbits.jpg`: a planetary system seen from above its plane (a
  bright star, shaded planets, a ringed giant, an asteroid belt) over the
  Milky Way, the previous default, offered in Settings
- `celenn.jpg`: the logo as a scene, offered in Settings
- `celenn-waves.jpg`: layered ribbons of light, offered in Settings
- `celenn-milky-way.jpg`: the Milky Way above the Earth, a photo taken
  on the International Space Station (NASA, ISS064-E-55946, public domain),
  graded with the Celenn OS palette, offered in Settings

COSMIC Settings lists the wallpapers in the order of the folders on disk
(alphabetical in the deployed image), so the file names put the default
(arcs) and the deep spiral first, then the others in alphabetical order. The build moves
COSMIC's own wallpapers to `/usr/share/backgrounds/cosmic-desktop` so that they
come after ours (`build_files/20-cosmic.sh`).

Keep the main elements inside the centre so they survive cropping to 16:10,
4:3 and 21:9. `wallpaper/build.sh` renders them all (the NASA original is
downloaded); the output is deterministic (fixed seeds). Keep the file names:
accounts that picked a wallpaper in Settings point to its path.

The first steps tour shows the wallpapers as small thumbnails (224x126 JPEGs
in `system_files/usr/share/celenn/wallpapers/`, same file names): run
`python3 branding/tour/thumbnails.py` from the repository root after adding, removing or changing a wallpaper
(the image build checks that each wallpaper has its thumbnail).

There are no light versions: the same wallpaper serves both themes. Later
COSMIC releases read a wallpaper per theme (`com.system76.CosmicBackground.Dark`
/ `.Light`); both point to the default.

The eight shader wallpapers are GLSL fragment shaders in `wallpaper/shaders/`
(`common.glsl` holds the palette, noise, stars and the film-like finish),
rendered tile by tile with WebGL 2 in Playwright's headless Chromium by
`wallpaper/shaders/render.js NAME OUT.png [WIDTH HEIGHT]`. They keep the
top panel and the dock readable: no black edges, nothing dazzling behind
them.

`wallpaper/orbits.py` draws the planetary system with numpy over
the sky of `wallpaper/sky.py`. The waves are drawn as SVG by
`wallpaper/concept.py`, rendered by `wallpaper/render.js` and finished by
`wallpaper/compose.py concept` (a fine grain against banding). `wallpaper/milky_way.py` crops and grades the photo.

The logo scene is built from two layers:

- `wallpaper/sky.py`: sky gradient, procedural Milky Way and stars (numpy),
  also the background of the orbits
- `wallpaper/foreground.py`: the logo as a scene, a planet inside a
  glowing open orbit with a moon in the gap (SVG, rendered by
  `wallpaper/render.js` with Playwright's Chromium)

`wallpaper/compose.py dark` dims the
sky, adds a blue glow behind the planet and a fine grain against banding, and
writes the JPEG.

## fastfetch

`fastfetch` (an alias of `ublue-fastfetch`) shows the mark as terminal art
next to the system information, in place of Bluefin's dinosaurs:

- `system_files/usr/share/ublue-os/fastfetch/celenn-dark.txt` /
  `-light.txt`: the mark as color pixels, two per terminal cell, 20 rows by
  49 columns so it stays round in COSMIC Terminal, whose cells are about 2.4
  times higher than wide. COSMIC Terminal draws the usual block elements
  shorter than the cell (lines between rows), and its translucent background
  shows any opaque stand-in for it (reverse video, blended edges) as dark
  squares. So cells with both halves use a background color, and the halves
  are the upper and lower half glyphs of the Celenn Prompt font
  (`prompt-font/`), which span exactly half of the line. The
  light version darkens the moon and the planet highlight. Regenerate them
  with `python3 fastfetch/build.py` (needs `rsvg-convert`, numpy and
  Pillow); it picks the size and position of the logo that give the
  cleanest curves.
- `system_files/usr/share/ublue-os/fastfetch.jsonc`: the modules (image, OS,
  COSMIC version, hardware...), with Nerd Font icons
- `system_files/usr/bin/ublue-fastfetch`: picks the logo for the COSMIC
  dark/light mode; `ublue-bling-fastfetch` gives the keys the COSMIC accent
  color. A user's own `~/.config/fastfetch/config.jsonc` is used instead
  when it exists.

## Shell prompt font

`system_files/usr/share/fonts/celenn/celenn-prompt.ttf`, generated
by `python3 prompt-font/build.py <output>` (fontTools): the arrows, the logo
mark and the icons of the Starship prompt
(`system_files/usr/share/celenn/starship.toml`), in plane 16 private use
code points. The arrows span COSMIC Terminal's whole line (1.4 times the font
size, with its default font Noto Sans Mono); Nerd Font arrows only reach the
font's own height there. Rebuild the font after any change to the script:
the image copies the committed font, and the Lint workflow fails when it is
not the one the script builds (`python3 prompt-font/build.py --check <font>`).

## Icons

The `Celenn OS` icon theme is Tela circle, recolored with the accent
(`build_files/icon_theme/`), with a few icons of our own in front of it.
`icons/build.py` draws them as SVG:

- folders, home, desktop, network and trash (`build_files/icon_theme/layer/places/`):
  a disk in the logo blues with a folder tab and a white symbol
- common file types (`build_files/icon_theme/layer/mimetypes/`): a light page
  whose symbol and folded corner take the color of the type (documents blue,
  spreadsheets green, archives amber...). Language-specific icons (Python,
  Rust...) stay Tela's.
- the live ISO's welcome tour and installer (`iso_files/welcome/*.svg`): the
  mark on a night-blue disk
- the first steps tour's page icons and the Keyboard shortcuts app's icon
  (`system_files/usr/share/icons/hicolor/scalable/apps/celenn-tour-*.svg`),
  in the style of the dock buttons: an accent disk, white symbols, the less
  important parts in translucent white

The files are named like Tela's, and the theme also takes every name that
Tela links to them (`build_files/icon_theme/compose.py`). Regenerate them with
`python3 icons/build.py` after a change.

## Avatars

The profile pictures offered in COSMIC Settings, 512x512 PNGs in
`system_files/usr/share/pixmaps/faces/`: a white drawing on a colored
gradient with a few faint stars, in the style of the dock icons, one hue per
avatar. `pop-robot.png`, the picture COSMIC Settings gives accounts without
one, is a link to `planet.png`.

- `avatars/make.py`: draws the SVGs into `avatars/svg/`
- `avatars/bbox.js` measures each drawing; `avatars/offsets.json` holds the
  shifts that center it
- `avatars/earth-ortho.py`: the land of the Earth avatar
  (`avatars/earth-land.txt`), from Natural Earth
- `avatars/export.js`: renders the PNGs with Playwright's Chromium

Regenerate them with `python3 avatars/make.py` then
`node avatars/export.js ../system_files/usr/share/pixmaps/faces`, and
compress them losslessly with
`oxipng -o 4 --strip safe ../system_files/usr/share/pixmaps/faces/*.png`
(the same pixels in under a third of the size).

## Licenses

The project's own artwork (logo, wallpapers except the photo, icons drawn by
`icons/build.py`, the avatars, the prompt font, the Plymouth images) is under the
repository's Apache-2.0 license. Third-party parts shipped in the image:

| Part | Source | License |
|------|--------|---------|
| Wordmark letters (outlines in `logo/glyphs.json`) | Montserrat Light | SIL Open Font License 1.1 |
| Milky Way photo (`celenn-milky-way.jpg`) | NASA, ISS064-E-55946 | Public domain |
| Land outlines of the Earth avatar (`avatars/earth-land.txt`) | [Natural Earth](https://www.naturalearthdata.com), 1:110m land | Public domain |
| Icon theme base | [Tela circle](https://github.com/vinceliuice/Tela-circle-icon-theme), recolored | No license file in the upstream repository (the author's other themes are GPL-3.0) |
| Cursor theme | [Bibata](https://github.com/ful1e5/Bibata_Cursor), recolored | GPL-3.0 (its LICENSE ships with the theme) |
| Boot spinner frames | Fedora's Plymouth spinner theme, tinted | GPL-2.0-or-later |
