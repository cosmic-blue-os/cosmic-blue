#!/usr/bin/env bash
# Assemble the website into the given directory (default: _site).
# The logo and the wallpaper come from the repository, so the site always
# shows the current branding. Needs ImageMagick.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
out="${1:-${root}/_site}"

rm -rf "${out}"
mkdir -p "${out}/img"

# Live ISO download: ISO_BASE_URL is the public URL of the R2 bucket. Until
# it is set, link to the live ISO workflow runs (needs a GitHub login).
# One ISO per image: celenn, and celenn-nvidia for NVIDIA cards.
iso_name="celenn-live-amd64.iso"
iso_nvidia_name="celenn-nvidia-live-amd64.iso"
runs="https://github.com/celenn-os/celenn/actions/workflows/build-live-iso.yml?query=branch%3Amain+is%3Asuccess"

# Size of each ISO online (Content-Length), shown next to the download button
declare -A iso_sizes=()
if [[ -n "${ISO_BASE_URL:-}" ]]; then
  for kind in main nvidia; do
    name="${iso_name}"
    [[ "${kind}" == nvidia ]] && name="${iso_nvidia_name}"
    bytes="$(curl -fsSIL --retry 2 --max-time 20 "${ISO_BASE_URL%/}/${name}" 2>/dev/null \
      | tr -d '\r' | awk 'tolower($1) == "content-length:" { n = $2 } END { print n }' || true)"
    if [[ "${bytes}" =~ ^[0-9]+$ ]] && (( bytes > 100000000 )); then
      iso_sizes[${kind}]="$(awk -v b="${bytes}" 'BEGIN { printf "%.1f", b / 1e9 }')"
      echo "${name}: ${iso_sizes[${kind}]} GB"
    fi
  done
fi

# Desktop screenshots for the screenshots page and the home page preview,
# from SCREENSHOTS_DIR (filled by website/fetch-screenshots.sh in CI). Files
# are named like 03-desktop.png; the pages show a screenshot only if it is
# there (blocks between <!-- shot:NAME --> and <!-- /shot -->).
shots_dir="${SCREENSHOTS_DIR:-}"
shots=""
zoom_shots="parental-controls-child parental-more-time first-steps-2 first-steps-3 tools-overview tools-software updates"
# The home page shows these app windows alone, at their real size: cut out
# along the window's light blue border
win_shots="first-steps-2 updates parental-controls-child parental-more-time tools-software"
# Prints the window's box (WxH+X+Y) in the screenshot: the pixels of the
# border color, kept only where they form lines at least 200 px long (the top
# bar, 40 px, is left out)
window_box() {
  convert "$1" -crop 1920x960+0+40 +repage \
    \( -clone 0 -channel B -separate +channel -threshold 88% \) \
    \( -clone 0 -channel R -separate +channel -threshold 27% \) \
    \( -clone 0 -channel R -separate +channel -negate -threshold 33% \) \
    \( -clone 0 -channel G -separate +channel -threshold 47% \) \
    -delete 0 -evaluate-sequence min \
    \( -clone 0 -morphology Open rectangle:1x200 \) \
    \( -clone 0 -morphology Open rectangle:200x1 \) \
    -delete 0 -evaluate-sequence max -format '%@' info:
}
if [[ -n "${shots_dir}" && -f "${shots_dir}/info.env" ]]; then
  mkdir -p "${out}/img/screenshots"
  for file in "${shots_dir}"/*.png "${shots_dir}"/*.ppm; do
    [[ -e "${file}" ]] || continue
    name="$(basename "${file%.*}")"
    name="${name#[0-9][0-9]-}"
    [[ " ${shots} " == *" ${name} "* ]] && continue
    convert "${file}" -strip -quality 90 -define webp:method=6 "${out}/img/screenshots/${name}.webp"
    convert "${file}" -resize 960x -strip -quality 85 -define webp:method=6 "${out}/img/screenshots/${name}-960.webp"
    # The "Why Celenn OS" page shows app windows closer: the middle of the
    # screen, where the window opens
    if [[ " ${zoom_shots} " == *" ${name} "* ]]; then
      convert "${file}" -gravity center -crop 1200x760+0-40 +repage -strip -quality 85 \
        -define webp:method=6 "${out}/img/screenshots/${name}-zoom.webp"
    fi
    if [[ " ${win_shots} " == *" ${name} "* ]]; then
      box="$(window_box "${file}")"
      if [[ ! "${box}" =~ ^([0-9]+)x([0-9]+)\+([0-9]+)\+([0-9]+)$ ]] \
          || (( BASH_REMATCH[1] < 300 || BASH_REMATCH[2] < 300 )); then
        echo "::error file=website/build.sh::no app window found in screenshot ${name} (box: ${box})"
        exit 1
      fi
      convert "${file}" -crop "${BASH_REMATCH[1]}x${BASH_REMATCH[2]}+${BASH_REMATCH[3]}+$(( BASH_REMATCH[4] + 40 ))" \
        +repage -strip -quality 88 -define webp:method=6 "${out}/img/screenshots/${name}-win.webp"
    fi
    shots+=" ${name}"
  done
  # shellcheck source=/dev/null
  source "${shots_dir}/info.env"
elif [[ -n "${shots_dir}" ]]; then
  # CI always passes the screenshots of the latest image
  echo "::error file=website/build.sh::SCREENSHOTS_DIR=${shots_dir} has no screenshots (see website/fetch-screenshots.sh)"
  exit 1
fi
# The "Why Celenn OS" page needs its screenshots: fail when the boot test
# no longer takes one (renamed or removed) rather than drop it silently
for name in ${zoom_shots} ${win_shots}; do
  if ! grep -qE "[0-9]{2}-${name%-[0-9]}" "${root}/tests/vm-lib.sh"; then
    echo "::error file=website/build.sh::screenshot ${name} (zoom_shots or win_shots) is not taken by tests/vm-lib.sh"
    exit 1
  fi
  if [[ -n "${shots}" && " ${shots} " != *" ${name} "* ]]; then
    echo "::error file=website/build.sh::screenshot ${name} (zoom_shots or win_shots) is missing from the latest boot test"
    exit 1
  fi
done
# With screenshots (CI), every screenshot a page shows must be in the latest
# boot test: a page never keeps an old one, nor silently drops one
if [[ -n "${shots}" ]]; then
  missing=0
  for name in $(find "${root}/website" -name '*.html' -exec grep -ohE '<!-- shot:[a-z0-9-]+ -->' {} + \
      | sed -E 's/<!-- shot:([a-z0-9-]+) -->/\1/' | sort -u); do
    if [[ " ${shots} " != *" ${name} "* ]]; then
      echo "::error file=website/build.sh::screenshot ${name} is shown on the website but missing from the latest boot test"
      missing=1
    fi
  done
  (( missing == 0 )) || exit 1
fi
[[ -n "${shots}" ]] && shots+=" shots-info" || shots="no-shots"

# The same pages in every language: English at the root (the default), the
# others in their own folder (fr/, de/...).
languages=(en de es fr it pt-br)
pages=(index.html install/index.html docs/index.html docs/troubleshooting/index.html screenshots/index.html why/index.html)
all_pages=()
for lang in "${languages[@]}"; do
  for page in "${pages[@]}"; do
    [[ "${lang}" == en ]] && all_pages+=("${page}") || all_pages+=("${lang}/${page}")
  done
done

# Keep the translations in step with the English pages: same sections, same
# commands, same screenshots, and a link to every language in the menu.
sync_errors=0
outline() {
  grep -oE 'id="[^"]+"|<pre>|<!-- /?(shot(:[a-z0-9-]+)?|shots-info|no-shots|checksum|/checksum) -->|data-(nvidia|main)-only|data-iso-size="[a-z]+"|hreflang="[A-Za-z-]+"' "$1"
}
for page in "${pages[@]}"; do
  for lang in "${languages[@]:1}"; do
    if ! diff <(outline "${root}/website/${page}") <(outline "${root}/website/${lang}/${page}") > /dev/null; then
      echo "::error file=website/${lang}/${page}::not in step with website/${page} (sections, commands, screenshots or language links differ)"
      sync_errors=1
    fi
  done
done
(( sync_errors == 0 )) || exit 1

# A command in a code block starts at the left edge: indenting the HTML
# inside <pre> shows as spaces before the command (only the continuation of
# a line ending in a backslash may be indented)
indent_errors=0
for page in "${all_pages[@]}"; do
  if ! perl -0ne '
      my $bad = 0;
      while (/<pre[^>]*>(.*?)<\/pre>/gs) {
        my @lines = split /\n/, $1, -1;
        for my $i (1 .. $#lines) {
          next unless $lines[$i] =~ /^[ \t]/;
          next if $lines[$i - 1] =~ /\\\s*$/;
          print STDERR "  $lines[$i]\n";
          $bad = 1;
        }
      }
      exit $bad;' "${root}/website/${page}"; then
    echo "::error file=website/${page}::a line of a code block starts with spaces (HTML indentation inside <pre>)"
    indent_errors=1
  fi
done
(( indent_errors == 0 )) || exit 1

for page in "${all_pages[@]}"; do
  mkdir -p "$(dirname "${out}/${page}")"
  if [[ -n "${ISO_BASE_URL:-}" ]]; then
    sed -e "s|@ISO_NVIDIA_URL@|${ISO_BASE_URL%/}/${iso_nvidia_name}|g" \
        -e "s|@ISO_URL@|${ISO_BASE_URL%/}/${iso_name}|g" \
      "${root}/website/${page}" > "${out}/${page}"
    # The size of the ISOs online, in GB with one decimal (a decimal comma
    # in every language but English); the size written in the page stays if
    # it cannot be read
    for kind in main nvidia; do
      size="${iso_sizes[${kind}]:-}"
      [[ -n "${size}" ]] || continue
      [[ "${page}" == */* && "${page}" != docs/* && "${page}" != screenshots/* ]] && size="${size/./,}"
      sed -i "s|<span data-iso-size=\"${kind}\">[^<]*</span>|<span data-iso-size=\"${kind}\">${size}</span>|" "${out}/${page}"
    done
  else
    sed -e 's|<!-- checksum -->.*<!-- /checksum -->||' \
        -e "s|@ISO_NVIDIA_URL@|${runs}|g" -e "s|@ISO_URL@|${runs}|g" \
      "${root}/website/${page}" > "${out}/${page}"
  fi
  SHOTS="${shots}" perl -0pi -e '
    my %have = map { $_ => 1 } split " ", $ENV{SHOTS};
    s{^[ \t]*<!-- (?:shot:([\w-]+)|(shots-info|no-shots)) -->.*?<!-- /(?:shot|shots-info|no-shots) -->\n}
     {$have{$1 // $2} ? $& : ""}gmse;
    s{<!-- /?(shot(:[\w-]+)?|shots-info|no-shots) -->}{}g;' "${out}/${page}"
  sed -i -e "s|@SHOTS_DATE@|${SHOTS_DATE:-}|g" -e "s|@SHOTS_COMMIT@|${SHOTS_COMMIT:-}|g" \
    -e "s|@SHOTS_RUN@|${SHOTS_RUN:-}|g" "${out}/${page}"
  # The screenshots keep their names from one build to the next: the version
  # in their address makes browsers and link previews load the new ones
  shots_v="${SHOTS_VERSION:-${SHOTS_COMMIT:-}}"
  if [[ -n "${shots_v}" ]]; then
    sed -i -E "s#(img/screenshots/[a-z0-9-]+\.webp)#\1?v=${shots_v}#g" "${out}/${page}"
  fi
done
cp "${root}/website/style.css" "${root}/website/site.js" "${out}/"
cp "${root}/branding/logo/celenn-logo.svg" "${out}/img/celenn-logo.svg"
cp "${root}/branding/logo/celenn-logo-horizontal-dark.svg" \
   "${out}/img/celenn-logo-horizontal-dark.svg"

# Header image: WebP (about a quarter of the JPEG size), JPEG kept for old
# browsers and for link previews (og:image)
wallpaper="${root}/system_files/usr/share/backgrounds/celenn/celenn.jpg"
for width in 1280 2560; do
  convert "${wallpaper}" -resize "${width}x" -strip -interlace Plane \
    -sampling-factor 4:2:0 -quality 82 "${out}/img/wallpaper-${width}.jpg"
  convert "${wallpaper}" -resize "${width}x" -strip -quality 80 \
    -define webp:method=6 "${out}/img/wallpaper-${width}.webp"
done

# The site's own font (Inter, SIL Open Font License)
cp -r "${root}/website/fonts" "${out}/"

# Link preview image (og:image): the wallpaper, the logo and, when the
# screenshots are there, the desktop with Celenn OS Tools open
og_shot=()
if [[ -f "${out}/img/screenshots/tools-overview.webp" ]]; then
  og_shot=( '(' "${out}/img/screenshots/tools-overview.webp" -resize 880x -bordercolor '#8fc4ff' -border 2 ')'
            -gravity north -geometry +0+196 -composite )
fi
convert "${wallpaper}" -resize 1200x630^ -gravity center -extent 1200x630 \
  -fill '#050a1a' -colorize 30% \
  '(' "${root}/website/og-logo.png" -resize x79 ')' -gravity north -geometry +0+70 -composite \
  "${og_shot[@]}" -strip -quality 85 "${out}/img/og.jpg"

# Every image a page links to must be in the site: screenshots, logo,
# wallpaper (src, srcset, href and og:image addresses)
broken=0
for page in "${all_pages[@]}"; do
  dir="$(dirname "${out}/${page}")"
  while read -r ref; do
    ref="${ref%%\?*}"
    if [[ ! -f "${dir}/${ref}" ]]; then
      echo "::error file=website/${page}::image ${ref} is not in the site"
      broken=1
    fi
  done < <(grep -oE '(src|srcset|href|content)="[^"]+"' "${out}/${page}" \
    | sed -E 's/^[a-z]+="//; s/"$//' | tr ',' '\n' | awk '{ print $1 }' \
    | grep -E '\.(webp|png|jpe?g|svg|gif)(\?|$)' | grep -vE '^(https?:)?//' || true)
  # Absolute addresses of this site (og:image) are checked too
  while read -r ref; do
    ref="${ref%%\?*}"
    [[ -f "${out}/${ref#https://celenn-os.github.io/celenn/}" ]] && continue
    echo "::error file=website/${page}::image ${ref} is not in the site"
    broken=1
  done < <(grep -oE 'https://celenn-os\.github\.io/celenn/[^" ]+\.(webp|png|jpe?g|svg|gif)' "${out}/${page}" || true)
done
(( broken == 0 )) || exit 1

# Stop GitHub Pages from running Jekyll on the files
touch "${out}/.nojekyll"
