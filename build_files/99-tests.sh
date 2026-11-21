#!/usr/bin/bash
# Simple checks: if one fails, the build fails.
set -euox pipefail

# ublue signing keys present (otherwise updates from ghcr.io/ublue-os are impossible)
jq -e '.transports.docker."ghcr.io/ublue-os"' /etc/containers/policy.json >/dev/null
# No Bluefin wallpapers
test ! -e /usr/share/backgrounds/bluefin
# Profile pictures: the 22 cosmic-blue avatars only (512x512 PNG), and the
# picture COSMIC Settings uses for accounts without one (pop-robot.png, a
# Pop!_OS file) is the planet avatar
faces=/usr/share/pixmaps/faces
test ! -e "${faces}/bluefin"
test "$(find "${faces}" -maxdepth 1 -type f -name '*.png' | wc -l)" -eq 22
test "$(find "${faces}" -mindepth 1 -maxdepth 1 ! -name '*.png' | wc -l)" -eq 0
test "$(readlink -f "${faces}/pop-robot.png")" = "${faces}/planet.png"
for face in "${faces}"/*.png; do
    # PNG signature, then the IHDR width and height (bytes 16-23)
    test "$(od -An -tx1 -N8 "${face}" | tr -d ' ')" = 89504e470d0a1a0a
    test "$(od -An -tu1 -j16 -N8 "${face}" | tr -s ' ')" = " 0 0 2 0 0 0 2 0"
done
# cosmic-blue wallpapers: arcs (the logo and name) is the COSMIC default, the others are
# extras. No light duplicates.
bg=/usr/share/backgrounds/cosmic-blue
for name in arcs deep-spiral eclipse galactic-river galaxy gas-veils line-orbits \
        ringed-planet orbits waves milky-way; do
    test -s "${bg}/cosmic-blue-${name}.jpg"
done
test -s "${bg}/cosmic-blue.jpg"
test ! -e "${bg}/cosmic-blue-light.jpg"
test ! -e "${bg}/cosmic-blue-orbits-light.jpg"
test ! -e "${bg}/cosmic-blue-constellation.jpg"
test ! -e "${bg}/cosmic-blue-emblem.jpg"
grep -q "Path(\"${bg}/cosmic-blue-arcs.jpg\")" \
    /usr/share/cosmic/com.system76.CosmicBackground/v1/all
# Per-theme defaults (used by COSMIC releases after 1.11): the same wallpaper
# for both themes
grep -q "Path(\"${bg}/cosmic-blue-arcs.jpg\")" \
    /usr/share/cosmic/com.system76.CosmicBackground.Dark/v1/all
grep -q "Path(\"${bg}/cosmic-blue-arcs.jpg\")" \
    /usr/share/cosmic/com.system76.CosmicBackground.Light/v1/all
test -s /usr/share/icons/hicolor/scalable/apps/cosmic-blue-logo.svg
# fastfetch: cosmic-blue logo and config instead of Bluefin's, a valid
# config, keys in the accent color, and Nerd Font symbols for its icons
test ! -e /usr/share/ublue-os/bluefin-logos
test ! -e /etc/ublue-os/fastfetch.json
test -s /usr/share/ublue-os/fastfetch/cosmic-blue-dark.txt
test -s /usr/share/ublue-os/fastfetch/cosmic-blue-light.txt
grep -q 'cosmic-blue-${MODE}.txt' /usr/bin/ublue-fastfetch
sed '/^ *\/\//d' /usr/share/ublue-os/fastfetch.jsonc | jq empty
if grep -qE 'fastfetch-user-count|bazaar' /usr/share/ublue-os/fastfetch.jsonc; then
    echo "Bluefin fastfetch config left in place"; exit 1
fi
test "$(HOME=/tmp/fastfetch-test ublue-bling-fastfetch)" = "38;2;79;141;255"
test "$(HOME=/tmp/fastfetch-test ublue-bling-fastfetch --mode)" = dark
HOME=/tmp/fastfetch-test fastfetch --config /usr/share/ublue-os/fastfetch.jsonc \
    --pipe true --structure os:kernel | grep cosmic-blue
# (no grep -q: it would end fc-list with SIGPIPE, exit 141 under pipefail)
fc-list : family | grep 'Symbols Nerd Font' >/dev/null
# Logo art: only background colors and upper half blocks (gapless in
# COSMIC Terminal), and the ff alias
if LC_ALL=C.UTF-8 grep -q '[▁▂▃▄▅▆▇█▏▎▍▌▋▊▉▐▖▗▘▝▚▞▙▛▜▟]' /usr/share/ublue-os/fastfetch/cosmic-blue-*.txt; then
    echo "fastfetch logo uses glyphs that leave gaps in COSMIC Terminal"; exit 1
fi
grep -q "alias ff='ublue-fastfetch'" /etc/profile.d/cosmic-blue-fastfetch.sh
# Terminal welcome banner (uwelcome): cosmic-blue links and tips, no
# Bluefin links, launchers or commands left
# (uwelcome only reads config.toml since projectbluefin/common #1662)
test ! -e /etc/uwelcome/config.json
[[ "$(grep -E '^url = ' /etc/uwelcome/config.toml)" == 'url = "https://github.com/cosmic-blue-os/cosmic-blue/issues"
url = "https://github.com/cosmic-blue-os/cosmic-blue/discussions"
url = "https://cosmic-blue-os.github.io/cosmic-blue/docs/"' ]]
grep -qx 'commands = \["cosmic-blue-tips"\]' /etc/uwelcome/config.toml
# The banner's guard is only set in interactive shells (not in the shell
# that starts the desktop session, whose environment every terminal inherits)
grep -qx 'case \$- in' /etc/profile.d/uwelcome.sh
# Tips: every language has all the tips, and the session language picks them
tips_dir=/usr/share/cosmic-blue/tips
for tips in "${tips_dir}"/*.txt; do
    [[ "$(grep -c . "${tips}")" == "$(grep -c . "${tips_dir}/en.txt")" ]] \
        || { echo "${tips}: not the same number of tips as en.txt"; exit 1; }
done
for locale in en_US fr_FR de_DE es_ES it_IT pt_BR ja_JP; do
    tip="$(env -u LANGUAGE -u LC_ALL -u LC_MESSAGES LANG="${locale}.UTF-8" cosmic-blue-tips)"
    test -n "${tip}"
    file="${tips_dir}/${locale}.txt"
    [[ -f "${file}" ]] || file="${tips_dir}/${locale%%_*}.txt"
    [[ -f "${file}" ]] || file="${tips_dir}/en.txt"
    grep -qF -- "${tip%%(*}" "${file}" || { echo "cosmic-blue-tips: ${locale} tip not from ${file}"; exit 1; }
done
if grep -rlE 'projectbluefin\.io|bluefin-cli|ujust report([^-]|$)' /etc/uwelcome \
    || grep -rl 'projectbluefin\.io' /usr/share/ublue-os/just /usr/share/applications; then
    echo "Bluefin links or commands left in the welcome banner, ujust or launchers"; exit 1
fi
test ! -e /usr/share/applications/discourse.desktop
test ! -e /usr/share/applications/io.projectbluefin.Report.desktop
grep -q '^Exec=/usr/bin/cosmic-blue-docs$' /usr/share/applications/documentation.desktop
grep -qx 'Icon=cosmic-blue-docs' /usr/share/applications/documentation.desktop
test -s /usr/share/icons/hicolor/scalable/apps/cosmic-blue-docs.svg
# No ChairLift (Bluefin's Control Center) or its helpers
if compgen -G '/usr/share/applications/*chairlift*' >/dev/null \
    || compgen -G '/usr/share/polkit-1/actions/*chairlift*' >/dev/null \
    || compgen -G '/usr/share/glib-2.0/schemas/*chairlift*' >/dev/null \
    || compgen -G '/usr/share/ublue-os/homebrew/preinstall.d/*chairlift*' >/dev/null; then
    echo "ChairLift files left in the image"; exit 1
fi
test ! -e /usr/bin/chairlift-helper
test ! -e /usr/libexec/bootc-update-stage
# Dark theme by default (COSMIC and GTK)
grep -qx true /usr/share/cosmic/com.system76.CosmicTheme.Mode/v1/is_dark
test "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.desktop.interface color-scheme)" = "'prefer-dark'"
# Dock: Firefox and Thunderbird are flatpaks (COSMIC's default lists the
# Firefox RPM id "firefox"); the other favorites are installed apps
APP_LIST=/usr/share/cosmic/com.system76.CosmicAppList/v1/favorites
for app in org.mozilla.firefox org.mozilla.thunderbird; do
    grep -q "\"${app}\"" "${APP_LIST}"
    grep -q "^\[Flatpak Preinstall ${app}\]" /usr/share/flatpak/preinstall.d/cosmic-blue.preinstall
done
for app in $(grep -o '"com\.system76\.[A-Za-z]*"' "${APP_LIST}" | tr -d '"'); do
    test -e "/usr/share/applications/${app}.desktop"
done
# App Library: our own apps in a "cosmic-blue" group (and not in COSMIC's
# System/Utilities groups); every visible launcher of ours must be listed
APP_GROUPS=/usr/share/cosmic/com.system76.CosmicAppLibrary/v1/groups
grep -q 'name: "cosmic-blue"' "${APP_GROUPS}"
test -s /usr/share/icons/hicolor/symbolic/apps/cosmic-blue-logo-symbolic.svg
for desktop in /usr/share/applications/io.github.cosmic_blue_os.*.desktop \
    /usr/share/applications/documentation.desktop; do
    if grep -qx 'NoDisplay=true' "${desktop}"; then
        continue
    fi
    app="$(basename "${desktop}" .desktop)"
    grep -q "\"${app}\"" "${APP_GROUPS}"
done
# Extra shortcuts (custom, so they show and can be changed in COSMIC Settings)
# next to COSMIC's own defaults, without taking one of their key combinations
SHORTCUTS=/usr/share/cosmic/com.system76.CosmicSettings.Shortcuts/v1
test -s "${SHORTCUTS}/defaults"
grep -q 'System(Terminal)' "${SHORTCUTS}/custom"
for combo in '\[Ctrl, Alt\], key: "t"' '\[Super\], key: "e"' \
             '\[Super, Shift\], key: "s"' '\[Ctrl, Shift\], key: "Escape"'; do
    grep -q "${combo}" "${SHORTCUTS}/custom"
    if grep -q "${combo}" "${SHORTCUTS}/defaults"; then exit 1; fi
done
# Floating top panel (gap from the screen edge, as COSMIC Settings' "Gap"
# toggle sets it)
PANEL=/usr/share/cosmic/com.system76.CosmicPanel.Panel/v1
grep -qx true "${PANEL}/anchor_gap"
grep -qx 4 "${PANEL}/margin"
# Dock hidden when a window overlaps it, as COSMIC Settings' "Automatically
# hide dock" toggle sets it
grep -qx OnOverlap /usr/share/cosmic/com.system76.CosmicPanel.Dock/v1/autohide
# Firefox defaults (policies), linked into the flatpak's systemconfig
# extension at boot; Bluefin's prefs and copy hook are gone
FIREFOX_POLICIES=/usr/share/cosmic-blue/firefox/policies/policies.json
jq -e '.policies.DisableTelemetry and .policies.DisableAppUpdate
    and .policies.ExtensionSettings."uBlock0@raymondhill.net".installation_mode == "normal_installed"' \
    "${FIREFOX_POLICIES}" >/dev/null
SYSTEMCONFIG=/var/lib/flatpak/extension/org.mozilla.firefox.systemconfig/x86_64/stable
# (Fedora 45's systemd-tmpfiles refuses a --root that does not exist yet)
mkdir -p /tmp/tmpfiles-root
SYSTEMD_LOG_LEVEL=warning systemd-tmpfiles --create --root=/tmp/tmpfiles-root - \
    </usr/lib/tmpfiles.d/cosmic-blue-firefox.conf
test "$(readlink "/tmp/tmpfiles-root${SYSTEMCONFIG}")" = /usr/share/cosmic-blue/firefox
rm -rf /tmp/tmpfiles-root
test ! -e /usr/share/ublue-os/firefox-config
test ! -e /usr/share/ublue-os/privileged-setup.hooks.d/99-flatpaks.sh
# Terminal: the cosmic-blue color schemes are the defaults, dark and light
TERM_CONFIG=/usr/share/cosmic/com.system76.CosmicTerm/v1
for kind in dark light; do
    grep -qx '"cosmic-blue"' "${TERM_CONFIG}/syntax_theme_${kind}"
    grep -q 'name: "cosmic-blue"' "${TERM_CONFIG}/color_schemes_${kind}"
done
# Shell prompt: the cosmic-blue Starship config is valid and renders a path
# segment (Starship prints a warning for each config error)
mkdir -p /tmp/starship-test
PROMPT_OUT="$(cd /tmp && HOME=/tmp/starship-test \
    STARSHIP_CONFIG=/usr/share/cosmic-blue/starship.toml starship prompt --status=1 2>&1)"
printf '%s\n' "${PROMPT_OUT}" | cat -v
if printf '%s' "${PROMPT_OUT}" | grep -qiE 'warn|error'; then exit 1; fi
printf '%s' "${PROMPT_OUT}" | grep -q '/tmp'
grep -q 'STARSHIP_CONFIG="${COSMIC_BLUE_STARSHIP}"' /etc/profile.d/90-starship.sh
# The prompt's arrows, logo and icons, and the fastfetch logo's half cells,
# come from the cosmic-blue Prompt font
for cp in 10cb00 10cb01 10cb02 10cb03 10cb04 10cb05 10cb06; do
    fc-list --format '%{file}\n' ":charset=${cp}" | grep -F /cosmic-blue-prompt.ttf >/dev/null
done
if grep -q 'Fedora\|\\ue0b0' /usr/share/cosmic-blue/starship.toml; then exit 1; fi
grep -q 'io.missioncenter.MissionCenter' /usr/share/flatpak/preinstall.d/cosmic-blue.preinstall
# Fedora's own wallpapers stay available
rpm -q "f${FEDORA_MAJOR_VERSION}-backgrounds-base"
# Each wallpaper is listed once in COSMIC Settings: no aliases, no copies
test -z "$(find /usr/share/backgrounds -type l)"
# cosmic-blue's wallpapers come first in COSMIC Settings (folder order)
test "$(find /usr/share/backgrounds -mindepth 1 -maxdepth 1 -printf '%f\n' | LC_ALL=C sort | head -n1)" = cosmic-blue
test ! -e /usr/share/backgrounds/cosmic
test -z "$(find /usr/share/backgrounds -type f -exec sha256sum {} + | cut -d' ' -f1 | sort | uniq -d)"
grep -q '^LOGO=cosmic-blue-logo$' /usr/lib/os-release
# Links to the repository, also from cosmic-blue-nvidia
grep -qx 'HOME_URL="https://github.com/cosmic-blue-os/cosmic-blue"' /usr/lib/os-release
grep -qx 'BUG_REPORT_URL="https://github.com/cosmic-blue-os/cosmic-blue/issues"' /usr/lib/os-release
# The live ISO installer matches its profile on VARIANT_ID
grep -qE '^VARIANT_ID=cosmic-blue(-nvidia)?$' /usr/lib/os-release
# Boot screen (Plymouth): cosmic-blue theme by default, with our logo images,
# the blue spinner and Fedora's other images, and included in the initramfs
test "$(plymouth-set-default-theme)" = cosmic-blue
PLYMOUTH_THEME=/usr/share/plymouth/themes/cosmic-blue
grep -qx "ImageDir=${PLYMOUTH_THEME}" "${PLYMOUTH_THEME}/cosmic-blue.plymouth"
grep -qx 'ModuleName=two-step' "${PLYMOUTH_THEME}/cosmic-blue.plymouth"
grep -qx 'UseFirmwareBackground=true' "${PLYMOUTH_THEME}/cosmic-blue.plymouth"
for image in watermark.png bgrt-fallback.png; do
    cmp "/ctx/system_files/cosmic-blue/usr/share/plymouth/themes/cosmic-blue/${image}" "${PLYMOUTH_THEME}/${image}"
done
compgen -G "${PLYMOUTH_THEME}/throbber-*.png" >/dev/null
test -e "${PLYMOUTH_THEME}/lock.png"
test -e "${PLYMOUTH_THEME}/entry.png"
# Clean graphical boot: systemd status lines hidden, GRUB menu hidden after a
# good boot (needs bootupd's static GRUB configuration and Fedora's
# boot-success flag)
grep -q '"systemd.show_status=error"' /usr/lib/bootc/kargs.d/10-cosmic-blue-quiet-boot.toml
grep -q '"rd.systemd.show_status=error"' /usr/lib/bootc/kargs.d/10-cosmic-blue-quiet-boot.toml
test -f /usr/lib/bootupd/grub2-static/grub-static-pre.cfg
test -f /usr/lib/bootupd/grub2-static/configs.d/12_cosmic-blue-menu-auto-hide.cfg
test -x /usr/sbin/grub2-set-bootflag
test -f /usr/lib/systemd/user/grub-boot-success.timer
systemctl --global is-enabled grub-boot-success.timer
KVER="$(rpm -q --qf '%{VERSION}-%{RELEASE}.%{ARCH}\n' kernel-core | head -n1)"
lsinitrd "/usr/lib/modules/${KVER}/initramfs.img" > /tmp/initramfs.txt
grep -q 'plymouth/themes/cosmic-blue/bgrt-fallback.png' /tmp/initramfs.txt
grep -q 'plymouth/themes/cosmic-blue/cosmic-blue.plymouth' /tmp/initramfs.txt
# TPM2 disk unlock (`ujust toggle-tpm2`): the recipe, our checks wrapping
# common's script, and TPM2 support in the initramfs (systemd-cryptsetup token)
grep -q '^toggle-tpm2:' /usr/share/ublue-os/just/shared.just
grep -q '/usr/bin/luks-tpm2-autounlock' /usr/share/ublue-os/just/shared.just
cmp /ctx/system_files/cosmic-blue/usr/bin/luks-tpm2-autounlock /usr/bin/luks-tpm2-autounlock
test -x /usr/libexec/ublue-luks-tpm2-autounlock
grep -q '^get_luks_uuid()' /usr/libexec/ublue-luks-tpm2-autounlock
grep -q 'BASH_SOURCE' /usr/libexec/ublue-luks-tpm2-autounlock
for cmd in systemd-cryptenroll cryptsetup gum mokutil; do command -v "${cmd}"; done
grep -q 'libcryptsetup-token-systemd-tpm2.so' /tmp/initramfs.txt
grep -q 'libtss2-esys' /tmp/initramfs.txt

# cosmic-blue signing key and policy, and the signed image reference
test -s /etc/pki/containers/cosmic-blue.pub
jq -e '.transports.docker."ghcr.io/cosmic-blue-os"[0].keyPath == "/etc/pki/containers/cosmic-blue.pub"' \
    /etc/containers/policy.json >/dev/null
grep -q 'use-sigstore-attachments: true' /etc/containers/registries.d/cosmic-blue.yaml
jq -e '."image-ref" | startswith("ostree-image-signed:")' /usr/share/ublue-os/image-info.json >/dev/null

# ujust and its recipes
for f in bin/ujust share/ublue-os/just/00-entry.just share/ublue-os/just/apps.just \
         share/ublue-os/just/default.just share/ublue-os/just/update.just; do
    test -e "/usr/${f}"
done
# cosmic-blue recipes (60-custom.just) parse and are listed by ujust
recipes="$(just --justfile /usr/share/ublue-os/just/00-entry.just --summary)"
for recipe in toggle-dark-mode reset-cosmic-theme reset-cosmic-config export-cosmic-config \
              install-microsoft-fonts install-cli-tools report-bug enroll-secure-boot-key \
              switch-channel install-gaming clean-system; do
    grep -qw -- "${recipe}" <<<"${recipes}"
done

# `ujust report-bug`: the system details script runs anywhere (here without
# deployments) and prints the image name
test -x /usr/libexec/cosmic-blue-system-info
/usr/libexec/cosmic-blue-system-info | grep "^Image: ${IMAGE_NAME} " >/dev/null

test ! -e /usr/share/flatpak/preinstall.d/bazaar.preinstall
# FlatPark (Bluefin's second Flatpak remote) is offered but off: not in
# remotes.d nor in the image's installation, its descriptor and filter (apps
# also on Flathub only come from Flathub) ready for `ujust toggle-flatpark`
test ! -e /usr/share/flatpak/remotes.d/flatpark.flatpakrepo
if flatpak remotes --system --columns=name | grep -qx flatpark; then echo "FlatPark is on"; exit 1; fi
FLATPARK=/usr/share/cosmic-blue/flatpak/flatpark.flatpakrepo
grep -q '^\[Flatpak Repo\]' "${FLATPARK}"
filter="$(sed -n 's/^Filter=//p' "${FLATPARK}")"
test "${filter}" = /usr/share/flatpak/flatpark.filter
grep -q '^deny ' "${filter}"
grep -qw toggle-flatpark <<<"${recipes}"
test ! -e /usr/lib/systemd/system/flatpark-filter-copy.path
if grep -q flatpark /usr/lib/systemd/system-preset/*.preset; then exit 1; fi
systemctl is-enabled --quiet cosmic-blue-flatpark-off.service
# Bluefin leftovers removed in 10-system-files.sh
for path in /etc/bazaar \
    /usr/lib/systemd/user/bazaar.service /usr/libexec/projectbluefin-countme \
    /usr/lib/systemd/system/projectbluefin-countme.timer /usr/share/ublue-os/agent \
    /usr/share/ublue-os/just/changelog.just /usr/share/ublue-os/homebrew/full-desktop.Brewfile; do
    test ! -e "${path}"
done
for recipe in contribute bluespeed bluefin-apps bazaar-preview toggle-testing changelogs \
              bluefin-cli; do
    # (`! cmd` never stops a set -e script: use if ... exit 1)
    if tr ' ' '\n' <<<"${recipes}" | grep -qx -- "${recipe}"; then echo "ujust ${recipe} left in place"; exit 1; fi
done
# Every listed command reads verb-object (build_files/10-system-files.sh
# renames Bluefin's that do not), and every install-<thing> has its
# remove-<thing>; the earlier names and Bluefin's own installers still run,
# unlisted
grep -q 'ublue-os/tap/antigravity-ide-linux' /usr/share/ublue-os/just/system.just
for recipe in $(tr ' ' '\n' <<<"${recipes}" | grep '^install-'); do
    tr ' ' '\n' <<<"${recipes}" | grep -qx -- "remove-${recipe#install-}" \
        || { echo "ujust ${recipe} has no remove-${recipe#install-}"; exit 1; }
done
odd="$(tr ' ' '\n' <<<"${recipes}" \
    | grep -vE '^(install|remove|toggle|reset|export|switch|report|enroll|check|show|run|reboot|clean|update)(-|$)' || true)"
if [[ -n "${odd}" ]]; then echo "ujust commands not named verb-object:" ${odd}; exit 1; fi
for recipe in install-dev-tools install-vms install-lima reset-default-apps install-cncf-tools \
              install-ai-tools install-asus install-jetbrains-toolbox install-opentabletdriver reboot-to-bios show-bios-info show-logs show-previous-logs run-benchmark \
              check-secure-boot reset-system report-bug; do
    tr ' ' '\n' <<<"${recipes}" | grep -qx -- "${recipe}"
done
for recipe in devmode toggle-devmode bug-report setup-vms setup-lima install-system-flatpaks cncf \
              bios bios-info logs-this-boot logs-last-boot benchmark check-sb-key powerwash \
              jetbrains-toolbox toggle-user-motd device-info toggle-vms _bluefin-dev-tools _bluefin-vms \
              _bluefin-lima _bluefin-cncf-tools _bluefin-ai-tools _bluefin-asus _bluefin-jetbrains-toolbox \
              _bluefin-opentabletdriver; do
    if tr ' ' '\n' <<<"${recipes}" | grep -qx -- "${recipe}"; then echo "ujust ${recipe} still listed"; exit 1; fi
    just --justfile /usr/share/ublue-os/just/00-entry.just --show "${recipe}" >/dev/null
done
if grep -q '^export LC_' /etc/zsh/zprofile; then exit 1; fi
# Kubernetes tools: Headlamp, not OpenLens (end-of-life on Flathub)
grep -q '^flatpak "io.kinvolk.Headlamp"' /usr/share/ublue-os/homebrew/cncf.Brewfile
if grep -q OpenLens /usr/share/ublue-os/homebrew/cncf.Brewfile; then exit 1; fi
# Default applications: ours, and each one names an app the system has: a
# launcher of the image, or a first-boot flatpak (whose launcher only exists
# after the first boot)
grep -q CosmicStore /etc/xdg/mimeapps.list
grep -qx 'application/vnd.appimage=it.mijorus.gearlever.desktop' /etc/xdg/mimeapps.list
while IFS='=' read -r type apps; do
    app="${apps%%;*}"
    if [[ ! -f "/usr/share/applications/${app}" ]] \
        && ! grep -qx "\[Flatpak Preinstall ${app%.desktop}\]" \
            /usr/share/flatpak/preinstall.d/cosmic-blue.preinstall; then
        echo "mimeapps.list: ${type} opens in ${app}, which is not installed"; exit 1
    fi
done < <(sed -n '/^\[Default Applications\]/,/^\[/{/^[a-z].*=/p}' /etc/xdg/mimeapps.list)
test ! -e /usr/share/applications/system-update.desktop
test -s /usr/share/icons/hicolor/scalable/apps/cosmic-blue-update.svg
test -s /usr/share/icons/hicolor/scalable/apps/cosmic-blue-tour-store.svg
test ! -e /usr/share/icons/hicolor/scalable/places/ublue-update.svg
test -s /usr/share/icons/cosmic-blue/layer/apps/system-software-update.svg
for icon in help-browser org.gnome.Terminal cs-tiling; do
    test -s "/usr/share/icons/cosmic-blue/layer/apps/${icon}.svg"
done
if grep -q Bazaar /usr/share/ublue-os/homebrew/system-flatpaks.Brewfile; then exit 1; fi
for remote in flathub cosmic; do
    grep -q '^\[Flatpak Repo\]' "/usr/share/flatpak/remotes.d/${remote}.flatpakrepo"
    flatpak remotes --system --columns=name | grep -qx "${remote}"
done
test -f /usr/share/flatpak/preinstall.d/cosmic-blue.preinstall
# First-boot flatpaks and `ujust reset-default-apps` install the same apps
diff <(sed -n 's/^\[Flatpak Preinstall \(.*\)\]$/\1/p' /usr/share/flatpak/preinstall.d/cosmic-blue.preinstall | sort) \
     <(sed -n 's/^flatpak "\(.*\)"$/\1/p' /usr/share/ublue-os/homebrew/system-flatpaks.Brewfile | sort)
# No flatpak that duplicates a COSMIC app or needs GNOME
for app in io.github.flattool.Ignition org.gnome.FileRoller org.gnome.Showtime \
           org.gnome.Decibels page.tesk.Refine org.gnome.Calendar org.gnome.Contacts; do
    if grep -q "${app}" /usr/share/flatpak/preinstall.d/cosmic-blue.preinstall; then exit 1; fi
done
test ! -e /usr/lib/systemd/system/flatpak-add-fedora-repos.service
# AppImages: Gear Lever on first boot, and the host can run both AppImage
# runtimes (older ones load libfuse.so.2, newer static ones need fusermount3)
grep -q '^\[Flatpak Preinstall it.mijorus.gearlever\]' /usr/share/flatpak/preinstall.d/cosmic-blue.preinstall
test -e /usr/lib64/libfuse.so.2
command -v fusermount >/dev/null
command -v fusermount3 >/dev/null
systemctl --global is-enabled cosmic-blue-appimage-update.timer
# No Bluefin GNOME dconf settings (the folder itself may come from Fedora)
if compgen -G '/etc/dconf/db/distro.d/*bluefin*' >/dev/null; then
    echo "Bluefin (GNOME) dconf settings present"; exit 1
fi

IMPORTANT_PACKAGES=(
    cosmic-session
    cosmic-greeter
    cosmic-comp
    cosmic-store
    cosmic-files
    cosmic-term
    distrobox
    fish
    flatpak
    pipewire
    systemd
    tailscale
    uupd
    wireplumber
    zsh
)
for package in "${IMPORTANT_PACKAGES[@]}"; do
    rpm -q "${package}" >/dev/null || { echo "Missing package: ${package}"; exit 1; }
done

# Everyday pieces: printing, scanning, codecs, fonts, disks, file previews
rpm -q cups cups-pk-helper foomatic-db-ppds system-config-printer hplip \
    gutenprint-cups \
    ptouch-driver printer-driver-brlaser foo2zjs nss-mdns \
    sane-backends sane-backends-drivers-scanners sane-airscan ipp-usb \
    fwupd upower \
    ffmpeg gstreamer1-plugin-libav gstreamer1-plugins-good \
    liberation-sans-fonts google-carlito-fonts google-crosextra-caladea-fonts \
    gnome-disk-utility papers-thumbnailer gnome-epub-thumbnailer cabextract
systemctl is-enabled cups.socket
systemctl is-enabled avahi-daemon.service
# Printers are added without the terminal: Print Settings
# (system-config-printer, through cups-pk-helper) is in the app list
grep -q '^Exec=system-config-printer' /usr/share/applications/system-config-printer.desktop
! grep -qE '^(NoDisplay=true|OnlyShowIn=)' /usr/share/applications/system-config-printer.desktop \
    || { echo "Print Settings is hidden from the app list"; exit 1; }
rpm -ql papers-thumbnailer | grep '^/usr/share/thumbnailers/.*\.thumbnailer$' >/dev/null
# Microsoft core fonts are never shipped in the image (their license forbids
# redistribution): users fetch them with `ujust install-microsoft-fonts`
test -z "$(find /usr/share/fonts -iname 'arial*.ttf' -o -iname 'verdana*.ttf')"

# No GNOME Shell package may end up in the image
for package in gnome-shell gdm mutter; do
    if rpm -q "${package}" >/dev/null; then
        echo "Unexpected GNOME package: ${package}"; exit 1
    fi
done

# COSMIC must come from Fedora packages (not a COPR)
for package in cosmic-session cosmic-comp; do
    vendor="$(rpm -q --qf '%{VENDOR}' "${package}")"
    [[ "${vendor}" == "Fedora Project" ]] || { echo "${package} does not come from Fedora (${vendor})"; exit 1; }
done

systemctl is-enabled cosmic-greeter.service
# docker CLI works through the rootless Podman socket (ujust install-dev-tools installs docker)
systemctl --global is-enabled podman.socket
test -x /usr/share/ublue-os/user-setup.hooks.d/30-docker-podman.sh
test "$(systemctl get-default)" = graphical.target

# Power profiles for laptops: tuned-ppd answers COSMIC's power profile
# requests, with battery detection; power-profiles-daemon conflicts with it
rpm -q tuned tuned-ppd thermald upower
if rpm -q power-profiles-daemon; then
    echo "power-profiles-daemon conflicts with tuned-ppd"; exit 1
fi
systemctl is-enabled tuned.service tuned-ppd.service thermald.service
test -f /usr/share/dbus-1/system-services/net.hadess.PowerProfiles.service
test -f /usr/share/dbus-1/system-services/org.freedesktop.UPower.PowerProfiles.service
grep -qx 'battery_detection=true' /etc/tuned/ppd.conf
grep -qx 'balanced=balanced-battery' /etc/tuned/ppd.conf
test -f /usr/lib/tuned/profiles/balanced-battery/tuned.conf ||
    test -f /usr/lib/tuned/balanced-battery/tuned.conf

# Installer keyboard layout and clock format copied to COSMIC user settings
test -x /usr/libexec/cosmic-blue-user-defaults
systemctl is-enabled cosmic-blue-user-defaults.service

# COSMIC's first-run assistant is skipped for new accounts (our tour opens
# instead, with its welcome illustration); VM agent
test -f /etc/skel/.config/cosmic-initial-setup-done
test -f /usr/share/cosmic-blue/tour-welcome.svg
test -f /usr/share/cosmic-blue/tour-banner.svg
rpm -q spice-vdagent

# Backups (Déjà Dup, build_files/30-packages.sh): the package starts its
# monitor at login in every desktop, which runs the automatic backups
rpm -q deja-dup
monitor_desktop=/etc/xdg/autostart/org.gnome.DejaDup.Monitor.desktop
grep -q '^Exec=.*deja-dup-monitor' "${monitor_desktop}"
if grep -qE '^(OnlyShowIn|NotShowIn|Hidden)=' "${monitor_desktop}"; then
    echo "${monitor_desktop} is limited to some desktops" >&2; exit 1
fi

# First steps tour: opened once for new accounts, at the first login;
# it runs and every text has its French translation
test -f /etc/skel/.config/cosmic-blue/first-steps-pending
test -x /usr/libexec/cosmic-blue-tour
test -x /usr/libexec/cosmic-blue-first-steps-autostart
grep -qx 'Exec=/usr/libexec/cosmic-blue-first-steps-autostart' \
    /etc/xdg/autostart/io.github.cosmic_blue_os.FirstSteps.desktop
test -f /usr/share/applications/io.github.cosmic_blue_os.FirstSteps.desktop
test -f /usr/share/icons/hicolor/scalable/apps/cosmic-blue-welcome.svg
# Color icons of the tour pages (build_files/first_steps/src/main.rs), found
# in the cosmic-blue icon theme (its layer, Tela) or hicolor
for icon in cosmic-blue-welcome cosmic-blue-tour-store cosmic-blue-tour-computer \
    cosmic-blue-tour-keyboard cosmic-blue-tour-bearings cosmic-blue-tour-try cosmic-blue-tour-tips \
    cosmic-blue-tour-files cosmic-blue-tour-accessibility org.gnome.DejaDup; do
    [[ -n "$(find /usr/share/icons/cosmic-blue /usr/share/icons/Tela-circle-4f8dff \
        /usr/share/icons/hicolor -name "${icon}.svg" -print -quit)" ]] \
        || { echo "Tour icon ${icon} not found" >&2; exit 1; }
done
# Wallpaper choice: each wallpaper has its small thumbnail (branding/tour/thumbnails.py)
for wallpaper in /usr/share/backgrounds/cosmic-blue/*.jpg; do
    test -s "/usr/share/cosmic-blue/wallpapers/${wallpaper##*/}" \
        || { echo "No tour thumbnail for ${wallpaper##*/}" >&2; exit 1; }
done
# Keyboard shortcuts sheet, and the dialog that explains Windows and other
# distributions' packages, opened by default for them
grep -qx 'Exec=/usr/libexec/cosmic-blue-tour --shortcuts' \
    /usr/share/applications/io.github.cosmic_blue_os.Shortcuts.desktop
grep -qx 'Exec=/usr/libexec/cosmic-blue-tour --open %f' \
    /usr/share/applications/io.github.cosmic_blue_os.OpenPackage.desktop
for type in application/x-ms-dos-executable application/x-msi application/vnd.debian.binary-package \
    application/x-rpm; do
    grep -qx "${type}=io.github.cosmic_blue_os.OpenPackage.desktop" /etc/xdg/mimeapps.list \
        || { echo "${type} does not open the package dialog" >&2; exit 1; }
done
# (libcosmic binary: its libraries resolve, and the check needs no display)
if ldd /usr/libexec/cosmic-blue-tour | grep 'not found'; then
    echo "cosmic-blue-tour: missing libraries"; exit 1
fi
/usr/libexec/cosmic-blue-tour --check

# cosmic-blue Tools: its launchers (the Updates one opens its Updates page;
# the earlier program name still does), its translations, the root helpers
# it runs with pkexec (polkit actions, limited to those programs), the
# scripts it shares with `ujust clean-system` and `ujust report-bug`, and
# uupd going through the bootc wrapper that keeps a version left behind from
# coming back
grep -qx 'Exec=/usr/libexec/cosmic-blue-tools' \
    /usr/share/applications/io.github.cosmic_blue_os.Tools.desktop
grep -qx 'Icon=cosmic-blue-tools' /usr/share/applications/io.github.cosmic_blue_os.Tools.desktop
test -s /usr/share/icons/hicolor/scalable/apps/cosmic-blue-tools.svg
grep -qx 'Exec=/usr/libexec/cosmic-blue-tools --updates' \
    /usr/share/applications/io.github.cosmic_blue_os.Updates.desktop
grep -qx 'Icon=cosmic-blue-update' /usr/share/applications/io.github.cosmic_blue_os.Updates.desktop
test "$(readlink /usr/libexec/cosmic-blue-updates)" = cosmic-blue-tools
if ldd /usr/libexec/cosmic-blue-tools | grep 'not found'; then
    echo "cosmic-blue-tools: missing libraries"; exit 1
fi
/usr/libexec/cosmic-blue-tools --check
for script in cosmic-blue-clean cosmic-blue-tools-helper cosmic-blue-report-bug; do
    test -x "/usr/libexec/${script}"
done
grep -q '<annotate key="org.freedesktop.policykit.exec.path">/usr/libexec/cosmic-blue-tools-helper</annotate>' \
    /usr/share/polkit-1/actions/io.github.cosmic_blue_os.tools.policy
if /usr/libexec/cosmic-blue-tools-helper clean everything 2>/dev/null; then
    echo "cosmic-blue-tools-helper accepted unknown arguments"; exit 1
fi
grep -q '/usr/libexec/cosmic-blue-clean' /usr/share/ublue-os/just/60-custom.just
grep -q '/usr/libexec/cosmic-blue-report-bug open' /usr/share/ublue-os/just/60-custom.just
# Only cosmic-blue's own `ujust clean-system` (Bluefin's is dropped)
test "$(grep -h '^clean-system' /usr/share/ublue-os/just/*.just | wc -l)" -eq 1
/usr/libexec/cosmic-blue-report-bug url | grep -q '^https://github.com/cosmic-blue-os/cosmic-blue/issues/new?template=bug_report.yml&system-info=Image'
test -x /usr/libexec/cosmic-blue-updates-helper
test -x /usr/libexec/cosmic-blue-bootc
grep -q '<annotate key="org.freedesktop.policykit.exec.path">/usr/libexec/cosmic-blue-updates-helper</annotate>' \
    /usr/share/polkit-1/actions/io.github.cosmic_blue_os.updates.policy
command -v pkexec >/dev/null
# "Check now" starts uupd-manual.service, which uupd's polkit rule lets any
# user start without a password
test -f /usr/lib/systemd/system/uupd-manual.service
grep -q 'uupd-manual.service' /usr/share/polkit-1/rules.d/uupd.rules
for unit in uupd.service uupd-manual.service; do
    systemctl cat "${unit}" | grep -qx 'Environment=UUPD_BOOTC_BINARY=/usr/libexec/cosmic-blue-bootc' \
        || { echo "${unit} does not run bootc through cosmic-blue-bootc"; exit 1; }
done
# Wrong arguments are refused before anything runs
if /usr/libexec/cosmic-blue-updates-helper channel dev 2>/dev/null; then
    echo "cosmic-blue-updates-helper accepted the dev channel"; exit 1
fi
if /usr/libexec/cosmic-blue-updates-helper edition amd 2>/dev/null; then
    echo "cosmic-blue-updates-helper accepted an unknown edition"; exit 1
fi

# Parental controls: the app (launcher, translations, warnings agent), its
# root helpers run with pkexec (polkit actions limited to those programs, more
# time asked every time), the service and its web filter, and the login check
# (PAM), which lets in accounts without rules
grep -qx 'Exec=/usr/libexec/cosmic-blue-parental-controls' \
    /usr/share/applications/io.github.cosmic_blue_os.ParentalControls.desktop
test -s /usr/share/icons/hicolor/scalable/apps/cosmic-blue-parental-controls.svg
grep -qx 'Exec=/usr/libexec/cosmic-blue-parental-controls --agent' \
    /etc/xdg/autostart/io.github.cosmic_blue_os.ParentalControlsAgent.desktop
for program in cosmic-blue-parental-controls cosmic-blue-parental-daemon; do
    if ldd "/usr/libexec/${program}" | grep 'not found'; then
        echo "${program}: missing libraries"; exit 1
    fi
done
/usr/libexec/cosmic-blue-parental-controls --check
for helper in cosmic-blue-parental-helper cosmic-blue-parental-extend; do
    test -x "/usr/libexec/${helper}"
    grep -q "<annotate key=\"org.freedesktop.policykit.exec.path\">/usr/libexec/${helper}</annotate>" \
        /usr/share/polkit-1/actions/io.github.cosmic_blue_os.parental.policy
done
grep -q '<allow_active>auth_admin</allow_active>' /usr/share/polkit-1/actions/io.github.cosmic_blue_os.parental.policy
systemctl is-enabled cosmic-blue-parental-controls.service
command -v dnsmasq nft setfacl notify-send >/dev/null
for service in cosmic-greeter greetd; do
    grep -qE '^account +required +pam_exec\.so stdout /usr/libexec/cosmic-blue-parental-daemon login-check$' \
        "/etc/pam.d/${service}"
done
PAM_USER=root /usr/libexec/cosmic-blue-parental-daemon login-check \
    || { echo "The parental controls login check refuses root"; exit 1; }
# Wrong arguments are refused before anything runs
if /usr/libexec/cosmic-blue-parental-helper set root </dev/null 2>/dev/null; then
    echo "cosmic-blue-parental-helper gave rules to root"; exit 1
fi
if /usr/libexec/cosmic-blue-parental-extend root 600 2>/dev/null; then
    echo "cosmic-blue-parental-extend accepted 600 minutes"; exit 1
fi
test ! -e /etc/cosmic-blue/parental-controls

# Login and lock screens: our rebuild, with the cosmic-blue wallpaper as the
# background shown before a user has a saved one
cmp /ctx/greeter/cosmic-greeter /usr/bin/cosmic-greeter

# COSMIC Store: our rebuild, with FlatPark as a recommended source (off) read
# from the descriptor that `ujust toggle-flatpark` uses too
cmp /ctx/store/cosmic-store /usr/bin/cosmic-store
grep -q /usr/share/cosmic-blue/flatpak/flatpark.flatpakrepo /usr/bin/cosmic-store

# COSMIC Files: our rebuild, which gives the user folders their own icons
# under /var/home (build_files/filemanager/patch.py)
for bin in /ctx/filemanager/bin/*; do
    cmp "${bin}" "/usr/bin/$(basename "${bin}")"
done

# The rebuilt programs carry no debug data (stripped in their helper stage),
# which would add about 0.1 GB each to the image and the ISO
for bin in /usr/bin/cosmic-greeter /usr/bin/cosmic-store /ctx/filemanager/bin/*; do
    sections="$(readelf -S --wide "${bin}")"
    # (.debug_gdb_scripts, Rust's debugger helpers, is loaded data and stays)
    if grep -q ' \.debug_info ' <<<"${sections}"; then
        echo "${bin} is not stripped"; exit 1
    fi
done

# Frosted glass on by default, in the theme apps read and in the builder
for theme in Dark Light; do
    grep -qx true "/usr/share/cosmic/com.system76.CosmicTheme.${theme}/v2/frosted_windows"
    grep -qx true "/usr/share/cosmic/com.system76.CosmicTheme.${theme}.Builder/v2/frosted_panel"
    # Thin frosted glass (Thickness Low) so the wallpaper shows through
    grep -qx Low "/usr/share/cosmic/com.system76.CosmicTheme.${theme}.Builder/v2/frosted"
    # COSMIC's own "Round" style (160 px radius_xl: pill-shaped panel and dock)
    for dir in "${theme}" "${theme}.Builder"; do
        grep -q '160' "/usr/share/cosmic/com.system76.CosmicTheme.${dir}/v2/corner_radii"
    done
done

# Icon theme: Tela circle dark (+ COSMIC symbolic icons it lacks), default for COSMIC and GTK
test -s /usr/share/icons/Tela-circle-4f8dff-dark/index.theme
test -s /usr/share/icons/Tela-circle-4f8dff/scalable/places/default-folder.svg
grep -q '4f8dff' /usr/share/icons/Tela-circle-4f8dff/scalable/places/default-folder.svg
test -s /usr/share/icons/cosmic-blue/index.theme
grep -qx 'Inherits=Tela-circle-4f8dff-dark,Cosmic,Pop,Adwaita,hicolor' /usr/share/icons/cosmic-blue/index.theme
grep -qx '"cosmic-blue"' /usr/share/cosmic/com.system76.CosmicTk/v1/icon_theme
test "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.desktop.interface icon-theme)" = "'cosmic-blue'"
test -n "$(find /usr/share/icons/cosmic-blue/tela/symbolic -name 'audio-volume-high-symbolic.svg' -print -quit)"
test -n "$(find /usr/share/icons/cosmic-blue/cosmic -name '*-symbolic.svg' -print -quit)"
test -n "$(find /usr/share/icons/cosmic-blue/tela -name 'com.system76.CosmicEdit.svg' -print -quit)"
# cosmic-blue layer (branding/icons/build.py) in front of Tela, with Tela's aliases
grep -q '^\[layer/places\]' /usr/share/icons/cosmic-blue/index.theme
grep -q '^\[layer/mimetypes\]' /usr/share/icons/cosmic-blue/index.theme
grep -q '^\[layer/apps\]' /usr/share/icons/cosmic-blue/index.theme
for icon in apps/com.system76.CosmicPanelAppButton apps/com.system76.CosmicPanelWorkspacesButton \
    apps/com.system76.CosmicPanelLauncherButton apps/com.system76.CosmicAppLibrary \
    apps/com.system76.CosmicStore apps/com.system76.CosmicSettings apps/com.system76.CosmicFiles \
    places/default-folder places/folder places/folder-download places/inode-directory \
    places/user-home places/user-trash mimetypes/text-plain mimetypes/image-jpeg mimetypes/application-zip; do
    test -s "/usr/share/icons/cosmic-blue/layer/${icon}.svg"
done
# Every link of the theme points to an existing icon
broken="$(find /usr/share/icons/cosmic-blue -xtype l)"
[[ -z "${broken}" ]] || { echo "Broken icon links:"; echo "${broken}"; exit 1; }
# Cursor theme: Bibata Modern in the cosmic-blue colors, the system default
# (COSMIC, login screen, X11 apps) and the GTK setting
CURSORS=/usr/share/icons/Bibata-Modern-cosmic-blue
grep -qx 'Name=Bibata Modern cosmic-blue' "${CURSORS}/index.theme"
for cursor in default pointer text wait progress grabbing not-allowed nwse-resize; do
    test -s "${CURSORS}/cursors/${cursor}"
done
test -z "$(find "${CURSORS}" -xtype l)"
grep -qx 'Inherits=Bibata-Modern-cosmic-blue' /usr/share/icons/default/index.theme
test "$(GSETTINGS_BACKEND=memory gsettings get org.gnome.desktop.interface cursor-theme)" = "'Bibata-Modern-cosmic-blue'"

# Theme colors built around the logo blue (see build_files/theme/src/main.rs).
# Prints the first color of a COSMIC config file as #rrggbb
# (colors are stored as hex strings, e.g. Some("#4F8DFFFF")).
first_color() {
    grep -oiE '#[0-9a-f]{6}' "$1" | head -n 1 | tr 'A-F' 'a-f'
}
check_color() {
    local file="$1" expected="$2" actual
    cat "${file}"
    actual="$(first_color "${file}")"
    [[ "${actual}" == "${expected}" ]] || { echo "${file}: ${actual}, expected ${expected}"; exit 1; }
}
builder=/usr/share/cosmic/com.system76.CosmicTheme
check_color "${builder}.Dark.Builder/v2/accent" '#4f8dff'
check_color "${builder}.Light.Builder/v2/accent" '#1d47c4'
check_color "${builder}.Dark.Builder/v2/bg_color" '#141c2a'
check_color "${builder}.Light.Builder/v2/bg_color" '#d3d8e0'
# The active window outline follows the accent
grep -qx None "${builder}.Dark.Builder/v2/window_hint"
grep -qx None "${builder}.Light.Builder/v2/window_hint"

# New image defaults reach existing accounts (cosmic-blue-migrate-defaults):
# the list of defaults covers COSMIC and GSettings, earlier defaults are known
# (Fedora's packaged ones, COSMIC's teal theme, COSMIC's icon theme), and the
# migration keeps the users' own choices
DEFAULTS_LIST=/usr/share/cosmic-blue/defaults.list
PREVIOUS_LIST=/usr/share/cosmic-blue/previous-defaults.list
MIGRATE=/usr/libexec/cosmic-blue-migrate-defaults
grep -q ' com.system76.CosmicTk/v1/icon_theme$' "${DEFAULTS_LIST}"
grep -q ' com.system76.CosmicTheme.Dark.Builder/v2/accent$' "${DEFAULTS_LIST}"
grep -q ' gsettings/org.gnome.desktop.interface/icon-theme$' "${DEFAULTS_LIST}"
if grep -q 'xkb_config$' "${DEFAULTS_LIST}"; then exit 1; fi
cmp <("${MIGRATE}" --manifest) "${DEFAULTS_LIST}"
for variant in /ctx/previous_defaults/theme/stock/cosmic /ctx/previous_defaults/theme/frosted/cosmic \
    /ctx/previous_defaults/theme/glass-0.8/cosmic /ctx/previous_defaults/theme/glass-0.65/cosmic \
    /ctx/build_files/previous_defaults/upstream; do
    "${MIGRATE}" --hash-tree "${variant}" | grep ' com.system76.CosmicT' >/dev/null
    missing="$(comm -23 <("${MIGRATE}" --hash-tree "${variant}" | sort) <(sort "${PREVIOUS_LIST}"))"
    [[ -z "${missing}" ]] || { echo "Not in ${PREVIOUS_LIST}:"; echo "${missing}"; exit 1; }
done
test -L /etc/systemd/system/multi-user.target.wants/cosmic-blue-migrate-defaults.service
/ctx/build_files/migrate-defaults-test.sh "${MIGRATE}"

# Secure Boot: the Universal Blue akmods key (enrolled with
# `ujust enroll-secure-boot-key`) is in every image, so it can be enrolled
# before switching to cosmic-blue-nvidia
AKMODS_CERT=/etc/pki/akmods/certs/akmods-ublue.der
test -s "${AKMODS_CERT}"
grep -q '^enroll-secure-boot-key:' /usr/share/ublue-os/just/60-custom.just
if grep -rqw universalblue /usr/share/ublue-os/just; then
    echo "ujust still uses the Universal Blue enrollment password"; exit 1
fi
command -v mokutil
# (PKCS#7 module signatures name the key by its certificate serial number)
akmods_key="$(openssl x509 -inform der -in "${AKMODS_CERT}" -noout -serial \
    | cut -d = -f 2 | tr -d ': \n' | tr 'a-f' 'A-F')"

# Secure Boot works with nothing to do on the standard image: Fedora's signed
# shim, GRUB and kernel, and every kernel module added to Fedora's (outside
# kernel/) signed by Fedora's key too. Only the NVIDIA image's driver is
# signed with the akmods key, enrolled by the person at the first boot.
fedora_module="$(find "/usr/lib/modules/${KVER}/kernel" -name '*.ko*' -print -quit)"
fedora_signer="$(modinfo -F signer "${fedora_module}")"
[[ -n "${fedora_signer}" ]] || { echo "${fedora_module} is not signed"; exit 1; }
while IFS= read -r -d '' module; do
    signer="$(modinfo -F signer "${module}")"
    echo "Added kernel module: ${module#"/usr/lib/modules/${KVER}/"} (signer: ${signer:-none})"
    [[ "${signer}" == "${fedora_signer}" ]] && continue
    if [[ "${IMAGE_NAME}" == *-nvidia && "${module##*/}" == nvidia* ]] \
        && [[ "$(modinfo -F sig_key "${module}" | tr -d ': \n' | tr 'a-f' 'A-F')" == "${akmods_key}" ]]; then
        continue
    fi
    echo "${module} is not signed by Fedora's key: it would not load with Secure Boot on"
    exit 1
done < <(find "/usr/lib/modules/${KVER}" -path "/usr/lib/modules/${KVER}/kernel" -prune -o -name '*.ko*' -print0)

if [[ "${IMAGE_NAME}" == *-nvidia ]]; then
    jq -e '."image-flavor" == "nvidia-open"' /usr/share/ublue-os/image-info.json >/dev/null
    # Driver and modules of the same version, for the image's kernel
    rpm -q kmod-nvidia nvidia-driver egl-wayland libva-nvidia-driver nvidia-container-toolkit
    test "$(rpm -q --qf '%{VERSION}' kmod-nvidia)" = "$(rpm -q --qf '%{VERSION}' nvidia-driver)"
    modinfo -k "${KVER}" nvidia nvidia-drm nvidia-modeset nvidia-uvm >/dev/null
    # Open modules, signed with the akmods key: Secure Boot accepts them once
    # that key is enrolled
    modinfo -k "${KVER}" -F license nvidia | grep -q 'Dual MIT/GPL'
    module_key="$(modinfo -k "${KVER}" -F sig_key nvidia | tr -d ': \n' | tr 'a-f' 'A-F')"
    [[ -n "${module_key}" && "${module_key}" == "${akmods_key}" ]] \
        || { echo "nvidia.ko is signed by ${module_key:-nothing}, not the akmods key ${akmods_key}"; exit 1; }
    # nvidia modules in the initramfs (black screen on boot otherwise), and
    # nouveau kept away
    grep -q 'nvidia-drm' /tmp/initramfs.txt
    grep -q 'nvidia-drm.modeset=1' /usr/lib/bootc/kargs.d/00-nvidia.toml
    grep -q 'modprobe.blacklist=nouveau' /usr/lib/bootc/kargs.d/00-nvidia.toml
    systemctl is-enabled cosmic-blue-nvidia-flatpak-gl.service
    # The NVIDIA repositories used for the install are disabled again
    if dnf5 repolist --enabled | grep -i nvidia; then
        echo "NVIDIA repositories left enabled"; exit 1
    fi
elif rpm -q kmod-nvidia; then
    echo "kmod-nvidia in a non-NVIDIA image"; exit 1
fi

# Only Fedora's repositories stay enabled (idea from Aurora): a third-party
# repository left on would feed `dnf`/`rpm-ostree install` on installed
# systems. fedora-multimedia (negativo17) is enabled by base-main itself for
# the codecs, as in every Universal Blue image.
allowed_repos='^(fedora|updates|updates-archive|fedora-multimedia)$'
enabled_repos="$(dnf5 repolist --enabled | awk 'NR > 1 { print $1 }')"
if unexpected="$(grep -vE "${allowed_repos}" <<<"${enabled_repos}")"; then
    echo "Unexpected enabled repositories:"; echo "${unexpected}"; exit 1
fi

# No ESP automount on /boot, which made ostree drop staged updates
# (50-services.sh)
[[ "$(readlink /etc/systemd/system-generators/systemd-gpt-auto-generator)" == /dev/null ]]

# Signature check turned on for systems switched to cosmic-blue without it
test -x /usr/libexec/cosmic-blue-signed-updates
systemctl is-enabled cosmic-blue-signed-updates.timer

# Repair of the bootc aleph of early installs (bootloader-update.service)
test -x /usr/libexec/cosmic-blue-fix-aleph
grep -q cosmic-blue-fix-aleph /usr/lib/systemd/system/bootloader-update.service.d/10-cosmic-blue-aleph.conf
# ISO installs: no systemd-remount-fs on the composefs root (fstab "/" line)
grep -q "ConditionPathExists=!/run/ostree-booted" /usr/lib/systemd/system/systemd-remount-fs.service.d/10-cosmic-blue-ostree.conf
# ...whose btrfs options (compression) are applied to the root btrfs
test -x /usr/libexec/cosmic-blue-root-mount-options
systemctl is-enabled cosmic-blue-root-mount-options.service

# System tuning shared with Bluefin and Aurora (most of it comes from Fedora,
# ublue-os/main and projectbluefin/common; tests/vm-lib.sh vm_check_tuning
# checks that it is active on a booted system)
# - weekly TRIM of the mounted filesystems, not of the fstab "/" line
systemctl is-enabled fstrim.timer
systemctl cat fstrim.service | grep -q -- '--listed-in /proc/self/mountinfo '
# - compressed RAM swap and the out-of-memory killer (Fedora defaults)
rpm -q zram-generator-defaults systemd-oomd-defaults
# - zram-aware swapping, a bounded write cache, more inotify instances: our
#   values must be the last ones systemd-sysctl applies
sysctl_config="$(/usr/lib/systemd/systemd-sysctl --cat-config)"
for setting in vm.swappiness=180 vm.page-cluster=0 vm.dirty_bytes=268435456 \
               vm.dirty_background_bytes=134217728 fs.inotify.max_user_instances=8192; do
    key="${setting%%=*}"
    last="$(sed -nE "s/^[[:space:]]*${key//./\\.}[[:space:]]*=[[:space:]]*//p" <<<"${sysctl_config}" | tail -n 1)"
    [[ "${key}=${last}" == "${setting}" ]] || { echo "sysctl ${key} is '${last}', expected ${setting#*=}"; exit 1; }
done
# - CA certificates and crypto policies regenerated after each update
#   (ublue-os/main), coredumps kept 5 days instead of 14
systemctl is-enabled bootc-update-ca-trust.service bootc-update-crypto-policies.service
grep -q '^d /var/lib/systemd/coredump .* 5d$' /usr/lib/tmpfiles.d/coredump.conf
# - zstd-compressed initramfs, with FIDO2 and PKCS#11 unlocking (passkeys,
#   smart cards) next to TPM2 (projectbluefin/common dracut config)
[[ "$(/usr/lib/dracut/skipcpio "/usr/lib/modules/${KVER}/initramfs.img" | head -c 4 | od -An -tx1 | tr -d ' \n')" == 28b52ffd ]]
grep -q 'libcryptsetup-token-systemd-fido2.so' /tmp/initramfs.txt
# - the screen reader and its speech
rpm -q orca
systemctl --global is-enabled speech-dispatcher.socket

# Homebrew sends no usage statistics
grep -qx 'HOMEBREW_NO_ANALYTICS=1' /etc/homebrew/brew.env
