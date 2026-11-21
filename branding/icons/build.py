#!/usr/bin/env python3
"""Generate the cosmic-blue icons drawn on top of Tela circle.

- places: folders (a planet-like disk in the logo blues, with a folder tab),
  the home, desktop, network and trash icons
- mimetypes: common file types (a light page whose symbol and folded corner
  take the color of the type)
- apps: the COSMIC dock buttons (launcher, workspaces, applications) and
  COSMIC Files, Store and Settings in the Tela circle style, as are the
  Documentation launcher, the System Update launcher, the live ISO's
  welcome tour and installer, and the installed system's first steps tour.
  The Documentation and System Update icons also replace Tela's help-browser
  and system-software-update, which the first steps tour shows, and the
  tour's commands and tiling pages get icons in the same style.
- actions: the symbolic icons of the suspend and restart buttons in the
  panel's power menu (Tela has none for restart, and its suspend icon is
  hard to read). They follow the style of Tela's shutdown icon, which stays.

The icon theme (build_files/icon_theme/compose.py) puts them in front of
Tela; every other icon still comes from Tela.

Usage: python3 build.py  (rewrites build_files/icon_theme/layer/)
"""
import math
import os
import shutil
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
LAYER = REPO / "build_files" / "icon_theme" / "layer"
APPS = REPO / "iso_files" / "welcome"
HICOLOR_APPS = REPO / "system_files/usr/share/icons/hicolor/scalable/apps"
FIRST_STEPS_ICON = HICOLOR_APPS / "cosmic-blue-welcome.svg"
DOCS_ICON = HICOLOR_APPS / "cosmic-blue-docs.svg"
UPDATE_ICON = HICOLOR_APPS / "cosmic-blue-update.svg"
TOUR_STORE_ICON = HICOLOR_APPS / "cosmic-blue-tour-store.svg"
TOOLS_ICON = HICOLOR_APPS / "cosmic-blue-tools.svg"

def stroke(d, width=2.2):
    return (f'<path d="{d}" fill="none" stroke="currentColor" stroke-width="{width}" '
            'stroke-linecap="round" stroke-linejoin="round"/>')


def fill(d):
    return f'<path d="{d}" fill="currentColor"/>'


def dot(x, y, r):
    return fill(f"M{x - r} {y}a{r} {r} 0 1 0 {2 * r} 0a{r} {r} 0 1 0 {-2 * r} 0")


# Symbols drawn in a 24x24 box centered on 0,0, in currentColor
GLYPHS = {
    "documents": stroke("M-6 -9h8l4 4v14h-12z") + stroke("M-3 -1h6M-3 3h6M-3 7h4", 1.8),
    "download": stroke("M0 -9v13M-6 -2l6 6 6-6") + stroke("M-8 9h16"),
    "music": stroke("M-3 6v-13l10 -3v13") + dot(-3, 6, 3.6) + dot(7, 3, 3.6),
    "pictures": stroke("M-9 -7h18v14h-18z") + stroke("M-9 5l6-6 5 5 3-3 4 4", 2) + dot(4, -3.5, 2.2),
    "videos": stroke("M-9 -7h18v14h-18z") + fill("M-2.5 -4v8l7 -4z"),
    "desktop": stroke("M-10 -8h20v13h-20z") + stroke("M-4 9h8M0 5v4"),
    "home": stroke("M-9 0l9-8 9 8M-6 -2v10h12v-10") + fill("M-2 8v-5h4v5z"),
    "templates": stroke("M-8 -9h16v18h-16z") + stroke("M-4 -4h8M-4 0h8M-4 4h4", 1.8),
    "publicshare": dot(-6, 0, 3) + dot(6, -6, 3) + dot(6, 6, 3) + stroke("M-6 0l12-6M-6 0l12 6", 2),
    "network": stroke("M0 -9a9 9 0 1 0 0.01 0z")
    + stroke("M-9 0h18M0 -9c-4 4-4 14 0 18M0 -9c4 4 4 14 0 18", 1.8),
    "recent": stroke("M0 -9a9 9 0 1 0 0.01 0z") + stroke("M0 -5v5l4 3"),
    "trash": stroke("M-9 -6h18M-3 -6v-3h6v3M-7 -6l1.5 15h11l1.5-15") + stroke("M-2 -1v6M2 -1v6", 1.8),
    "trash-full": stroke("M-9 -4h18M-7 -4l1.5 13h11l1.5-13") + stroke("M-5 -7l3-4 3 3 3-3 3 4", 1.8)
    + stroke("M-2 1v4M2 1v4", 1.8),
    "text": stroke("M-6 -6h12M-6 -2h12M-6 2h12M-6 6h7", 2),
    "document": fill("M-7 -8h14v4h-14z") + stroke("M-6 1h12M-6 5h12M-6 9h8", 1.8),
    "spreadsheet": stroke("M-7 -7h14v15h-14zM-7 -2h14M-7 3h14M-1 -7v15", 1.8),
    "presentation": stroke("M-8 -7h16v11h-16z", 2) + stroke("M0 4v5M-4 9h8", 2)
    + fill("M-4 1v-4h2.5v4zM-0.5 1v-6h2.5v6zM3 1v-2.5h2.5v2.5z"),
    "pdf": stroke("M-7 7c3-1 6-6 7-12c0-3-2-3-2 0c0 3 3 9 9 10c2 0 2-2 0-2c-5 0-11 3-14 6c-1 1 0 2 1 1", 1.9),
    "image": stroke("M-9 7l6-7 4 4 3-3 5 6z", 2) + dot(4, -5, 2.6),
    "audio": stroke("M-3 6v-13l9 -2.5v13") + dot(-3, 6, 3.4) + dot(6, 3.5, 3.4),
    "video": fill("M-5 -8v16l13-8z"),
    "archive": fill("M-3 -11h3v2.5h-3zM0 -8.5h3v2.5h-3zM-3 -6h3v2.5h-3zM0 -3.5h3v2.5h-3z")
    + stroke("M-3.5 1h7v8h-7zM-3.5 5h7", 2),
    "script": stroke("M-7 -5l5 5-5 5M1 6h7", 2.4),
    "code": stroke("M-4 -6l-6 6 6 6M4 -6l6 6-6 6M2 -9l-4 18", 2.2),
    "font": stroke("M-8 9l8-18 8 18M-4.5 2h9", 2.4),
    "disc": stroke("M0 -9a9 9 0 1 0 0.01 0z") + dot(0, 0, 2.5),
    "package": stroke("M0 -9l8 4v10l-8 4-8-4v-10zM-8 -5l8 4 8-4M0 -1v10", 2),
    "unknown": stroke("M-4 -4a4 4 0 1 1 6 3.5c-1.5 1-2 2-2 3.5", 2.4) + dot(0, 8, 1.6),
}


def svg(body, defs=""):
    return ('<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64">'
            f"<defs>{defs}</defs>{body}</svg>\n")


def glyph(name, x, y, scale, color):
    if not name:
        return ""
    return f'<g color="{color}" transform="translate({x} {y}) scale({scale})">{GLYPHS[name]}</g>'


# --- Folders -----------------------------------------------------------------

FOLDER_DEFS = (
    '<clipPath id="disk"><circle cx="32" cy="32" r="28"/></clipPath>'
    '<linearGradient id="front" x1=".15" y1="0" x2=".85" y2="1">'
    '<stop offset="0" stop-color="#78aeff"/><stop offset=".55" stop-color="#4f8dff"/>'
    '<stop offset="1" stop-color="#2f6bff"/></linearGradient>'
)


def folder(name, open_=False):
    """A planet-like folder, the size of Tela circle's icons.

    The folder is a disk: the inside, the back panel with its tab, the
    lighter front panel, which starts lower when the folder is open. The
    inside stays a light enough blue for the disk's top edge to read as a
    clean round on dark backgrounds (a dark navy one merged into them).
    """
    front = 22.0 if open_ else 18.6
    disk = (
        '<g clip-path="url(#disk)">'
        '<rect x="4" y="4" width="56" height="56" fill="#5d93f5"/>'
        '<path d="M4 11.3H29.2L33.1 15.2H60V60H4z" fill="#2f63d8"/>'
        f'<path d="M4 {front}H60V60H4z" fill="url(#front)"/>'
        f'<path d="M4 {front + 0.6}H60" stroke="#d6e8ff" stroke-width="1.2" opacity=".8"/>'
        "</g>"
        + glyph(name, 32, front + (60 - front) * 0.45, 1.1, "#ffffff")
    )
    # Drawn at 56 px, scaled to the 54 px disk of Tela circle's icons
    return svg(f'<g transform="translate(32 32) scale({27 / 28:.4f}) translate(-32 -32)">{disk}</g>', FOLDER_DEFS)


# Tela's file names: the icon theme also takes every Tela alias of them.
PLACES = {
    "default-folder": None,
    "default-folder-open": "open",
    "default-folder-documents": "documents",
    "default-folder-download": "download",
    "default-folder-music": "music",
    "default-folder-pictures": "pictures",
    "default-folder-video": "videos",
    "default-folder-templates": "templates",
    "default-folder-publicshare": "publicshare",
    "default-folder-recent": "recent",
    "default-user-desktop": "desktop",
    "default-user-home": "home",
    "default-network": "network",
    "user-trash": "trash",
    "user-trash-full": "trash-full",
}

# --- File types --------------------------------------------------------------

# Symbol -> (color, Tela names). The first name gets the file, the others are
# links to it. Tela's own aliases of all of them follow (compose.py).
MIMETYPES = {
    "text": ("#4f8dff", ["text-x-generic", "text-markdown", "text-x-readme", "text-richtext"]),
    "document": ("#4f8dff", ["x-office-document", "libreoffice-oasis-text", "application-vnd.ms-word",
                             "application-msword-template",
                             "application-vnd.oasis.opendocument.text-template"]),
    "spreadsheet": ("#3fbf7f", ["x-office-spreadsheet", "libreoffice-oasis-spreadsheet",
                                "application-vnd.ms-excel",
                                "application-vnd.oasis.opendocument.spreadsheet-template"]),
    "presentation": ("#ff8a4c", ["x-office-presentation", "libreoffice-oasis-presentation",
                                 "application-vnd.ms-powerpoint",
                                 "application-vnd.oasis.opendocument.presentation-template"]),
    "pdf": ("#ff5d6c", ["application-pdf"]),
    "image": ("#2ec4c4", ["image-x-generic"]),
    "audio": ("#ff6fb0", ["audio-x-generic"]),
    "video": ("#a873ff", ["video-x-generic"]),
    "archive": ("#f0a83a", ["package-x-generic", "application-x-archive", "application-x-zip",
                            "application-x-tar", "application-x-compressed-tar",
                            "application-x-bzip-compressed-tar", "application-x-lzma-compressed-tar",
                            "application-x-gzip", "application-x-bzip", "application-x-rar"]),
    "script": ("#8ea6d8", ["text-x-script", "application-x-shellscript", "application-x-executable"]),
    "code": ("#5fc8ff", ["text-html", "text-xml", "application-json", "application-x-yaml", "text-css"]),
    "font": ("#8ea6d8", ["font-x-generic"]),
    "disc": ("#4f8dff", ["application-x-cd-image", "application-x-iso9660-image"]),
    "package": ("#4f8dff", ["application-x-rpm", "application-vnd.flatpak.ref", "application-vnd.flatpak",
                            "application-vnd.appimage"]),
    "unknown": ("#8a94ad", ["unknown", "application-octet-stream"]),
}

PAGE = "M14 5H42L54 17V55A4 4 0 0 1 50 59H14A4 4 0 0 1 10 55V9A4 4 0 0 1 14 5z"
PAGE_DEFS = ('<linearGradient id="page" x1="0" y1="0" x2="0" y2="1">'
             '<stop offset="0" stop-color="#f7faff"/><stop offset="1" stop-color="#e3ebfa"/></linearGradient>')


def page(name, color):
    """A light page with a folded corner in the color of the file type."""
    body = (f'<path d="{PAGE}" fill="url(#page)" stroke="#b9c6e2" stroke-width="1"/>'
            f'<path d="M42 5V13A4 4 0 0 0 46 17H54z" fill="{color}"/>'
            + glyph(name, 32, 34, 1.05, color))
    return svg(body, PAGE_DEFS)


# --- Applications ------------------------------------------------------------

# --- COSMIC dock buttons ------------------------------------------------------

# Drawn like Tela circle's application icons: a disk in the accent, white
# symbols, the less important parts in translucent white.
# The disk darkens slightly towards the bottom, as Tela's lower half does, but
# in the disk's own fill: a shape laid over the disk would add a second
# anti-aliased edge (a darker rim) and a seam across the middle.
def shaded(color, amount=0.1):
    rgb = [int(color[i:i + 2], 16) for i in (1, 3, 5)]
    return "#" + "".join(f"{round(c * (1 - amount)):02x}" for c in rgb)


def disk(color, r=27):
    defs = ('<linearGradient id="disk-fill" x1="0" y1="0" x2="0" y2="1">'
            f'<stop offset=".5" stop-color="{color}"/><stop offset="1" stop-color="{shaded(color)}"/>'
            '</linearGradient>')
    return defs, f'<circle cx="32" cy="32" r="{r}" fill="url(#disk-fill)"/>'


STORE_COLOR = "#2ec4c4"
SETTINGS_COLOR = "#5f6b86"


def tela_app(body, color="#4f8dff"):
    defs, circle = disk(color)
    return svg(circle + body, defs)


def tile(x, y, size, opacity=None, radius=1.6):
    alpha = f' opacity="{opacity}"' if opacity else ""
    return f'<rect x="{x}" y="{y}" width="{size}" height="{size}" rx="{radius}" fill="#fff"{alpha}/>'


# The app library: a grid of applications
APPLICATIONS = tela_app("".join(
    tile(18 + col * 10, 18 + row * 10, 8, None if (row, col) == (1, 1) else ".75")
    for row in range(3) for col in range(3)
))
# Workspaces: two windows, one behind the other
WORKSPACES = tela_app(
    '<rect x="15" y="17" width="23" height="17" rx="2" fill="#fff" opacity=".45"/>'
    '<rect x="26" y="28" width="23" height="19" rx="2" fill="#fff"/>'
    '<path d="M28 28h19a2 2 0 0 1 2 2v2H26v-2a2 2 0 0 1 2-2z" fill="#2f6bff"/>'
)
# The launcher: search over applications
LAUNCHER = tela_app(
    tile(16, 16, 9, ".45") + tile(16, 27, 9, ".45") + tile(27, 16, 9, ".45")
    + '<circle cx="34" cy="34" r="9.5" fill="#4f8dff"/>'
    '<circle cx="34" cy="34" r="7.5" fill="none" stroke="#fff" stroke-width="3.6"/>'
    '<path d="M39.6 39.6l7.4 7.4" stroke="#fff" stroke-width="4.4" stroke-linecap="round"/>'
)


def store(color):
    """The store: a shopping bag with the logo's "c"."""
    return tela_app(
        '<path d="M25 24v-3a7 7 0 0 1 14 0v3" fill="none" stroke="#fff" stroke-width="3" stroke-linecap="round"/>'
        '<path d="M18.5 24h27a1.5 1.5 0 0 1 1.5 1.6l-1.4 19.6a3 3 0 0 1-3 2.8H21.4a3 3 0 0 1-3-2.8L17 25.6'
        'a1.5 1.5 0 0 1 1.5-1.6z" fill="#fff"/>'
        f'<path d="M37.2 31.5A6.5 6.5 0 1 0 37.2 40.5" fill="none" stroke="{color}" stroke-width="2.6" '
        'stroke-linecap="round"/>'
        f'<circle cx="32" cy="36" r="2.4" fill="{color}"/>',
        color,
    )


STORE = store(STORE_COLOR)
# The same bag in the accent, for the first steps tour's apps page, where
# every page icon is blue. The dock keeps the teal one.
TOUR_STORE = store("#4f8dff")
# The settings: two sliders
SETTINGS = tela_app(
    '<rect x="16" y="21" width="32" height="4" rx="2" fill="#fff" opacity=".45"/>'
    '<rect x="16" y="39" width="32" height="4" rx="2" fill="#fff" opacity=".45"/>'
    '<rect x="16" y="21" width="22" height="4" rx="2" fill="#fff"/>'
    '<rect x="16" y="39" width="10" height="4" rx="2" fill="#fff"/>'
    '<circle cx="38" cy="23" r="5" fill="#fff"/><circle cx="26" cy="41" r="5" fill="#fff"/>',
    SETTINGS_COLOR,
)

# The Documentation launcher: an open book with the logo's "c"
DOCS = tela_app(
    '<path d="M32 22c-4.5-3.5-10-4.5-15-4.5v24c5 0 10.5 1 15 4.5z" fill="#fff" opacity=".75"/>'
    '<path d="M32 22c4.5-3.5 10-4.5 15-4.5v24c-5 0-10.5 1-15 4.5z" fill="#fff"/>'
    '<path d="M21 25.5c2.7 .3 5.2 .9 7.5 1.8M21 30.5c2.7 .3 5.2 .9 7.5 1.8M21 35.5c2.7 .3 5.2 .9 7.5 1.8" '
    'stroke="#4f8dff" stroke-width="1.8" stroke-linecap="round" fill="none"/>'
    '<path d="M42.6 27.2A5 5 0 1 0 42.6 34.8" fill="none" stroke="#4f8dff" stroke-width="2.4" '
    'stroke-linecap="round"/>'
    '<circle cx="39" cy="31" r="1.9" fill="#4f8dff"/>'
)

# The live ISO installer: an arrow going down into a tray
INSTALLER = tela_app(
    '<path d="M17 36v6a4 4 0 0 0 4 4h22a4 4 0 0 0 4-4v-6" fill="none" stroke="#fff" stroke-opacity=".45" '
    'stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/>'
    '<rect x="29.5" y="15" width="5" height="17" rx="2.5" fill="#fff"/>'
    '<path d="M22.5 28.5a2.2 2.2 0 0 1 3.1-3.1L32 31.8l6.4-6.4a2.2 2.2 0 0 1 3.1 3.1l-8 8a2.1 2.1 0 0 1-3 0z" fill="#fff"/>')

# The welcome tour (live ISO) and first steps tour (installed system): a rocket
WELCOME = tela_app(
    '<g transform="translate(32 32) rotate(45) scale(1.05)">'
    '<path d="M-7.5 1L-13 7.5V13L-7.5 9.5zM7.5 1L13 7.5V13L7.5 9.5z" fill="#fff" opacity=".45"/>'
    '<path d="M-4 11.5h8C4 15 2 18 0 20.5C-2 18 -4 15 -4 11.5z" fill="#fff" opacity=".45"/>'
    '<path d="M0 -17C6 -12 7.5 -5 7.5 2V9h-15V2C-7.5 -5 -6 -12 0 -17z" fill="#fff"/>'
    '<circle cx="0" cy="-3" r="3.6" fill="#4f8dff"/></g>'
)


def circle_arrow(r, start, end, opacity=None, head=6.2, width=5):
    """A clockwise arc around the center, from angle start to end (degrees,
    counterclockwise from the right), ending in an arrowhead."""
    def at(angle, radius=r):
        a = math.radians(angle)
        return 32 + radius * math.cos(a), 32 - radius * math.sin(a)
    (x0, y0), (x1, y1) = at(start), at(end)
    a = math.radians(end)
    tip = (x1 + math.sin(a) * head * 1.05, y1 + math.cos(a) * head * 1.05)
    (bx1, by1), (bx2, by2) = at(end, r + head), at(end, r - head)
    alpha = f' opacity="{opacity}"' if opacity else ""
    return (f'<g{alpha}><path d="M{x0:.2f} {y0:.2f}A{r} {r} 0 0 1 {x1:.2f} {y1:.2f}" fill="none" '
            f'stroke="#fff" stroke-width="{width}" stroke-linecap="round"/>'
            f'<path d="M{tip[0]:.2f} {tip[1]:.2f}L{bx1:.2f} {by1:.2f}L{bx2:.2f} {by2:.2f}z" fill="#fff" '
            'stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/></g>')


# The System Update launcher and the tour's updates page: two circling arrows,
# the lower one translucent
UPDATE = tela_app(circle_arrow(13.5, 165, 40) + circle_arrow(13.5, 345, 220, ".45"))

# cosmic-blue Tools (the ujust commands as buttons): a wrench, with a
# screwdriver behind it in translucent white
TOOLS = tela_app(
    '<g transform="translate(32 32) rotate(45)">'
    '<rect x="-2.6" y="-17" width="5.2" height="14" rx="1.2" fill="#fff" opacity=".45"/>'
    '<rect x="-4.2" y="-3" width="8.4" height="20" rx="4.2" fill="#fff" opacity=".45"/></g>'
    '<mask id="wrench-jaw"><rect x="-20" y="-24" width="40" height="44" fill="#fff"/>'
    '<path d="M-3.2 -25h6.4v8.5a3.2 3.2 0 0 1 -6.4 0z" fill="#000"/></mask>'
    '<g transform="translate(32 32) rotate(-45)"><g mask="url(#wrench-jaw)">'
    '<circle cx="0" cy="-12" r="8.6" fill="#fff"/>'
    '<rect x="-3.4" y="-8" width="6.8" height="25" rx="3.4" fill="#fff"/></g></g>'
)

# The first steps tour's commands page: a prompt in a terminal window
COMMANDS = tela_app(
    '<rect x="15" y="18" width="34" height="28" rx="4" fill="none" stroke="#fff" stroke-opacity=".45" '
    'stroke-width="3.5"/>'
    '<path d="M22.5 26.5l5.5 5-5.5 5" fill="none" stroke="#fff" stroke-width="3.6" stroke-linecap="round" '
    'stroke-linejoin="round"/>'
    '<rect x="31" y="35" width="11" height="3.6" rx="1.8" fill="#fff"/>'
)
# The tour's tiling page: three windows side by side
TILING = tela_app(
    '<rect x="16" y="17" width="15" height="30" rx="2.5" fill="#fff"/>'
    '<rect x="33" y="17" width="15" height="14" rx="2.5" fill="#fff" opacity=".45"/>'
    '<rect x="33" y="33" width="15" height="14" rx="2.5" fill="#fff" opacity=".45"/>'
)

# The first steps tour's page icons, in the same style as the dock buttons
# (and the Keyboard shortcuts app's icon)
# Your computer: a screen on its stand
TOUR_COMPUTER = tela_app(
    '<path d="M27.5 43h9l1.5 5h-12z" fill="#fff" opacity=".45"/>'
    '<rect x="21" y="47" width="22" height="3.4" rx="1.7" fill="#fff"/>'
    '<rect x="14" y="16" width="36" height="27" rx="3.5" fill="#fff"/>'
    '<rect x="17.5" y="19.5" width="29" height="20" rx="1.5" fill="#4f8dff"/>'
    '<path d="M17.5 33.5L31 19.5h8L17.5 39.5z" fill="#fff" opacity=".3"/>'
)
# Keyboard shortcuts: a keyboard
TOUR_KEYBOARD = tela_app(
    '<rect x="12" y="20" width="40" height="25" rx="4" fill="#fff"/>'
    + "".join(f'<rect x="{16 + i * 6.6:.1f}" y="24" width="5" height="5" rx="1.2" fill="#4f8dff" opacity=".55"/>'
              for i in range(5))
    + "".join(f'<rect x="{19.3 + i * 6.6:.1f}" y="30.5" width="5" height="5" rx="1.2" fill="#4f8dff" opacity=".55"/>'
              for i in range(4))
    + '<rect x="22" y="37.5" width="20" height="4" rx="1.2" fill="#4f8dff"/>'
)
# Your bearings: a compass
TOUR_BEARINGS = tela_app(
    '<circle cx="32" cy="32" r="16" fill="none" stroke="#fff" stroke-opacity=".45" stroke-width="3.5"/>'
    '<path d="M32 18.5l5 13.5h-10z" fill="#fff"/>'
    '<path d="M32 45.5l5-13.5h-10z" fill="#fff" opacity=".45"/>'
    '<circle cx="32" cy="32" r="2.6" fill="#4f8dff"/>'
)
# Try it: a pointer that clicks
TOUR_TRY = tela_app(
    '<path d="M19.5 22.5l-3.5-3.5M25 18v-5M19 27h-5" stroke="#fff" stroke-opacity=".45" stroke-width="3" '
    'stroke-linecap="round"/>'
    '<path d="M24 22.5L44 31.5l-8.6 2.8 6.3 8.6-4.6 3.4-6.3-8.6-5.2 7.4z" fill="#fff" '
    'stroke="#fff" stroke-width="1.5" stroke-linejoin="round"/>'
)
# Good to know: a light bulb
TOUR_TIPS = tela_app(
    '<path d="M32 14a12 12 0 0 1 7 21.7c-1.3 1-2 2.3-2 3.8V40h-10v-.5c0-1.5-.7-2.8-2-3.8A12 12 0 0 1 32 14z" '
    'fill="#fff"/>'
    '<rect x="27" y="42" width="10" height="3.2" rx="1.6" fill="#fff" opacity=".45"/>'
    '<rect x="28.5" y="46.5" width="7" height="3.2" rx="1.6" fill="#fff" opacity=".45"/>'
    '<path d="M28.5 25.5a5 5 0 0 1 4-3.5" fill="none" stroke="#4f8dff" stroke-width="2.4" stroke-linecap="round"/>'
)
# Accessibility (the welcome page's row): a person, arms open
TOUR_ACCESSIBILITY = tela_app(
    '<circle cx="32" cy="18.5" r="4" fill="#fff"/>'
    '<path d="M19 26.5l13 2 13-2M32 28.5v8M32 36.5l-6 11M32 36.5l6 11" fill="none" stroke="#fff" '
    'stroke-width="3.6" stroke-linecap="round" stroke-linejoin="round"/>'
)
TOUR_ICONS = {
    "cosmic-blue-tour-accessibility": TOUR_ACCESSIBILITY,
    "cosmic-blue-tour-computer": TOUR_COMPUTER,
    "cosmic-blue-tour-keyboard": TOUR_KEYBOARD,
    "cosmic-blue-tour-bearings": TOUR_BEARINGS,
    "cosmic-blue-tour-try": TOUR_TRY,
    "cosmic-blue-tour-tips": TOUR_TIPS,
}


# Icon -> names (the desktop entries of the dock buttons and of the apps
# they open). The first name gets the file, the others are links to it.
# COSMIC Files: a folder, its back panel with the tab in translucent white
FILES = tela_app(
    '<path d="M15 20a2.5 2.5 0 0 1 2.5-2.5h9l3.5 3.5h17.5a2.5 2.5 0 0 1 2.5 2.5v20a2.5 2.5 0 0 1'
    '-2.5 2.5h-30a2.5 2.5 0 0 1-2.5-2.5z" fill="#fff" opacity=".45"/>'
    '<path d="M15 27.5a2.5 2.5 0 0 1 2.5-2.5h30a2.5 2.5 0 0 1 2.5 2.5v16a2.5 2.5 0 0 1'
    '-2.5 2.5h-30a2.5 2.5 0 0 1-2.5-2.5z" fill="#fff"/>'
)

DOCK = {
    "applications": (APPLICATIONS, ["com.system76.CosmicPanelAppButton", "com.system76.CosmicAppLibrary"]),
    "workspaces": (WORKSPACES, ["com.system76.CosmicPanelWorkspacesButton", "com.system76.CosmicWorkspaces",
                                "com.system76.CosmicAppletWorkspaces"]),
    "launcher": (LAUNCHER, ["com.system76.CosmicPanelLauncherButton", "com.system76.CosmicLauncher"]),
    "store": (STORE, ["com.system76.CosmicStore"]),
    "settings": (SETTINGS, ["com.system76.CosmicSettings"]),
    "files": (FILES, ["com.system76.CosmicFiles"]),
    # Tela's update icon, which the first steps tour shows on its updates page
    "update": (UPDATE, ["system-software-update"]),
    # Tela's help icon, which the first steps tour shows on its last page
    "help": (DOCS, ["help-browser"]),
    # The icons the tour shows on its commands and tiling pages. Only these
    # names, which nothing else installed uses: the terminal app keeps its icon.
    "commands": (COMMANDS, ["org.gnome.Terminal"]),
    "tiling": (TILING, ["cs-tiling"]),
}

# Session action symbolic icons, 16x16, in Tela's symbolic gray. They are
# drawn with strokes, then outlined into plain fills: GTK recolors symbolic
# icons by forcing their fill, which would fill an unoutlined stroke.
# (names, path); the first name gets the file, the others are links to it.
SYMBOLIC_COLOR = "#565656"
ACTIONS = [
    # A crescent moon
    (["system-suspend-symbolic"],
     "M7.31,2.04 C6.12,4.03 6.45,6.57 8.11,8.18 C9.77,9.8 12.32,10.06 14.28,8.82 C14,12.13 11.2,14.66 7.88,14.6 C4.56,14.54 1.86,11.91 1.71,8.59 C1.55,5.27 4,2.4 7.31,2.04 Z"),
    # An open ring with a clockwise arrowhead, like the shutdown ring
    (["system-reboot-symbolic", "system-restart-symbolic"],
     "M14.07,6.17 Q16.18,10.68 12.46,14 Q8.74,17.31 4.5,14.71 Q0.25,12.11 1.52,7.29 Q2.65,3.02 6.69,2.4 L6.63,0.84 Q6.63,0.71 6.7,0.6 Q6.8,0.47 6.96,0.44 Q7.12,0.41 7.26,0.5 L10.74,2.88 Q10.8,2.91 10.83,2.96 Q10.94,3.09 10.92,3.26 Q10.9,3.42 10.77,3.52 L7.46,6.14 Q7.35,6.22 7.22,6.23 Q7.06,6.23 6.94,6.12 Q6.82,6.01 6.81,5.84 L6.76,4.43 Q4.2,4.98 3.45,7.8 Q2.57,11.18 5.54,13.01 Q8.52,14.83 11.13,12.51 Q13.74,10.18 12.26,7.01 Q12.09,6.64 12.23,6.25 Q12.37,5.86 12.74,5.68 Q13.12,5.51 13.51,5.65 Q13.9,5.79 14.07,6.17 Z"),
]


def symbolic(d):
    return ('<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">'
            f'<path fill="{SYMBOLIC_COLOR}" fill-rule="evenodd" d="{d}"/></svg>\n')


def main():
    shutil.rmtree(LAYER, ignore_errors=True)
    places = LAYER / "places"
    mimetypes = LAYER / "mimetypes"
    apps = LAYER / "apps"
    places.mkdir(parents=True)
    mimetypes.mkdir()
    apps.mkdir()
    actions = LAYER / "actions"
    actions.mkdir()

    for names, d in ACTIONS:
        (actions / f"{names[0]}.svg").write_text(symbolic(d))
        for alias in names[1:]:
            os.symlink(f"{names[0]}.svg", actions / f"{alias}.svg")

    for icon, names in DOCK.values():
        (apps / f"{names[0]}.svg").write_text(icon)
        for alias in names[1:]:
            os.symlink(f"{names[0]}.svg", apps / f"{alias}.svg")

    for name, symbol in PLACES.items():
        is_open = symbol == "open"
        (places / f"{name}.svg").write_text(folder(None if is_open else symbol, open_=is_open))

    for symbol, (color, names) in MIMETYPES.items():
        (mimetypes / f"{names[0]}.svg").write_text(page(symbol, color))
        for alias in names[1:]:
            os.symlink(f"{names[0]}.svg", mimetypes / f"{alias}.svg")

    (APPS / "cosmic-blue-welcome.svg").write_text(WELCOME)
    # Also the icon of the installed system's first steps tour
    FIRST_STEPS_ICON.write_text(WELCOME)
    DOCS_ICON.write_text(DOCS)
    UPDATE_ICON.write_text(UPDATE)
    TOOLS_ICON.write_text(TOOLS)
    TOUR_STORE_ICON.write_text(TOUR_STORE)
    # Bring your things: COSMIC Files' folder
    for name, icon in {**TOUR_ICONS, "cosmic-blue-tour-files": FILES}.items():
        (HICOLOR_APPS / f"{name}.svg").write_text(icon)
    (APPS / "cosmic-blue-installer.svg").write_text(INSTALLER)


if __name__ == "__main__":
    main()
