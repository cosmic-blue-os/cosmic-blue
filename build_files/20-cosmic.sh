#!/usr/bin/bash
# COSMIC desktop, from the official Fedora repositories only.
set -euox pipefail

# For the build log: COSMIC groups available in this Fedora release
dnf5 group list --hidden | grep -i cosmic || true

# Comps group used by Fedora COSMIC (spin and Atomic). No COPR is enabled here,
# so COSMIC packages necessarily come from Fedora (checked in 99-tests.sh).
# @cosmic-desktop-apps is skipped: on Fedora it pulls KDE/GNOME apps
# (Okular, Rhythmbox, Nheko...) that we replace with flatpaks, like Bluefin.
# COSMIC Monitor is the system monitor (Ctrl + Shift + Esc, see the
# shortcuts in /usr/share/cosmic/com.system76.CosmicSettings.Shortcuts).
dnf5 -y install \
    @cosmic-desktop \
    cosmic-edit \
    cosmic-files \
    cosmic-monitor \
    cosmic-player \
    cosmic-store \
    cosmic-term

# The screen reader that COSMIC starts with Super + Alt + S (shown on the
# first page of our tour): named here so the image never ships without it
dnf5 -y install orca

# The COSMIC version Celenn OS is tested against (also checked by
# `just helpers-keys` before the helper stages): a new version comes through
# the pull request of check-cosmic-version.yml, which moves cosmic-version.
pinned="$(grep -v '^#' /ctx/build_files/cosmic-version | tr -d '[:space:]')"
installed="$(rpm -q --qf '%{VERSION}' cosmic-settings)"
if [[ "${installed}" != "${pinned}" ]]; then
    echo "COSMIC ${installed} installed, but Celenn OS is pinned to ${pinned} (build_files/cosmic-version)" >&2
    exit 1
fi

# (no power-profiles-daemon: base-main already ships tuned-ppd)
# Desktop pieces that Fedora's Atomic image brings and base-main may lack
dnf5 -y install --skip-unavailable \
    NetworkManager-wifi \
    NetworkManager-openvpn \
    NetworkManager-openconnect \
    bluez \
    gnome-keyring \
    gnome-keyring-pam \
    pipewire-pulseaudio \
    wireplumber \
    plymouth-system-theme \
    xdg-desktop-portal-gtk \
    xdg-user-dirs \
    adw-gtk3-theme \
    spice-vdagent

# spice-vdagent: in a VM (GNOME Boxes, virt-manager), clipboard sharing and
# following the window size. It only starts when a SPICE channel is present.

systemctl enable cosmic-greeter.service

# COSMIC's first-run assistant (cosmic-initial-setup) does not open: the
# installer has already set the language, keyboard layout, time zone and the
# account, and Celenn OS sets the appearance and panel layout. It exits at
# once when ~/.config/cosmic-initial-setup-done exists, which new accounts get
# from /etc/skel. Its accessibility page is replaced by the accessibility row
# on the first page of our tour below. (Its OEM mode, run by a
# "cosmic-initial-setup" system user whose home is not made from /etc/skel,
# is not used on Celenn OS: Anaconda creates the account.)
mkdir -p /etc/skel/.config
touch /etc/skel/.config/cosmic-initial-setup-done

# Celenn OS's own first steps tour (/usr/libexec/celenn-tour, built
# with libcosmic in Containerfile.helpers), opened once at the first login
# of each new account: the autostart entry only acts while
# ~/.config/celenn/first-steps-pending exists, which new accounts get
# from /etc/skel (see /usr/libexec/celenn-first-steps-autostart).
mkdir -p /etc/skel/.config/celenn
touch /etc/skel/.config/celenn/first-steps-pending

# The Updates launcher (Celenn OS Tools on its Updates page, built with the
# tour) replaces the "System Update" launcher of Bluefin's shared files, which
# ran `ujust update` in a terminal: its "Check now" runs the same update
# (uupd-manual.service), and `ujust update` stays for the terminal.
# /usr/libexec/celenn-updates, its earlier program (the tour, tests),
# opens the same page.
rm -f /usr/share/applications/system-update.desktop
ln -sf celenn-tools /usr/libexec/celenn-updates

systemctl set-default graphical.target

# Fedora's default wallpaper for this release (day/night), offered in COSMIC
# Settings next to the Celenn OS one. (Fedora 44 has no extras package.)
dnf5 -y install "f${FEDORA_MAJOR_VERSION}-backgrounds-base"

# COSMIC Settings lists every image under /usr/share/backgrounds, following
# symlinks, so aliases (desktop-backgrounds-compat's default.png...) and
# copies show the same Fedora wallpaper many times. Keep one file per image.
echo "Wallpapers before removing duplicates:"
find /usr/share/backgrounds -printf '%y %p %l\n' | sort
while IFS= read -r -d '' link; do
    target="$(readlink -f "${link}" || true)"
    if [[ -z "${target}" || ! -e "${target}" || "${target}" == /usr/share/backgrounds/* ]]; then
        echo "Removing alias ${link} -> ${target}"
        rm -f "${link}"
    fi
done < <(find /usr/share/backgrounds -type l -print0)
declare -A seen_wallpapers=()
while IFS= read -r -d '' file; do
    hash="$(sha256sum "${file}" | cut -d' ' -f1)"
    if [[ -n "${seen_wallpapers[${hash}]:-}" ]]; then
        echo "Removing ${file}, same image as ${seen_wallpapers[${hash}]}"
        rm -f "${file}"
    else
        seen_wallpapers[${hash}]="${file}"
    fi
done < <(find /usr/share/backgrounds -type f -regextype egrep \
    -iregex '.*\.(png|jpe?g|jxl|webp|svg)' -print0 | sort -z)
find /usr/share/backgrounds -mindepth 1 -type d -empty -delete
# COSMIC Settings lists the wallpapers in folder order (alphabetical in the
# deployed image): move COSMIC's own wallpapers after ours ("cosmic" sorts after
# "celenn", but other folders may come between).
if [[ -d /usr/share/backgrounds/cosmic ]]; then
    mv /usr/share/backgrounds/cosmic /usr/share/backgrounds/cosmic-desktop
fi
echo "Wallpapers kept:"
find /usr/share/backgrounds -printf '%y %p %l\n' | sort

# For the build log: the dock favorites and shortcuts as packaged by Fedora
cat /usr/share/cosmic/com.system76.CosmicAppList/v1/favorites || true
ls /usr/share/cosmic/com.system76.CosmicSettings.Shortcuts/v1/ || true

# Defaults that accounts may still hold as a copy from an earlier image:
# Fedora's packaged COSMIC defaults (before ours replace them), COSMIC's
# earlier default themes and built-in defaults (build_files/previous_defaults).
# celenn-migrate-defaults resets such copies when a default changes.
PREVIOUS_DEFAULTS=/usr/share/celenn/previous-defaults.list
mkdir -p "$(dirname "${PREVIOUS_DEFAULTS}")"
{
    /usr/libexec/celenn-migrate-defaults --hash-tree /usr/share/cosmic
    for variant in /ctx/previous_defaults/theme/*/cosmic /ctx/build_files/previous_defaults/*/; do
        /usr/libexec/celenn-migrate-defaults --hash-tree "${variant}"
    done
} | sort -u >"${PREVIOUS_DEFAULTS}"
wc -l "${PREVIOUS_DEFAULTS}"

# Celenn OS defaults (wallpaper, theme, dock, extra shortcuts) and boot
# screen (Plymouth theme)
# were copied before these packages were installed; apply them again so they
# win over the packaged files.
cp -a /ctx/system_files/celenn/usr/share/cosmic/. /usr/share/cosmic/
cp -a /ctx/system_files/celenn/usr/share/plymouth/. /usr/share/plymouth/
# COSMIC's default wallpapers (CosmicBackground) point to the folder moved
# above until ours replace them: nothing else may point there.
if grep -rlF /usr/share/backgrounds/cosmic/ /usr/share/cosmic /etc 2>/dev/null; then
    echo "These files point to the moved COSMIC wallpapers" >&2
    exit 1
fi

# COSMIC's own theme bundles ("constellations": COSMIC, Dunes, Mirror Grove,
# Still Peaks, Universe in four colors, each dark and light), shown next to
# Celenn Dark and Celenn Light in Settings > Appearance. System76 ships them in
# cosmic-theme-bundles, which Fedora doesn't package: taken from upstream at a
# pinned commit. Their wallpapers (Git LFS, checked against the LFS pointers)
# go with COSMIC's in cosmic-desktop, their icon themes (Universe) in
# /usr/share/icons. Celenn's own bundles and defaults stay as they are.
# Updated by Renovate (latest commit of the master branch, see .github/renovate.json5)
THEME_BUNDLES_REPO="https://github.com/pop-os/cosmic-theme-bundles.git"
THEME_BUNDLES_COMMIT="792870f4adbe13eb5c178f530f7e5b7aedfc6bb6"
bundles_src=/tmp/cosmic-theme-bundles
bundles_base="${THEME_BUNDLES_REPO%.git}"
mkdir -p "${bundles_src}"
curl -fsSL --retry 3 "${bundles_base}/archive/${THEME_BUNDLES_COMMIT}.tar.gz" \
    | tar -xz -C "${bundles_src}" --strip-components=1
bundle_wallpapers=()
bundles_media="https://media.githubusercontent.com/media/${bundles_base#https://github.com/}/${THEME_BUNDLES_COMMIT}"
while IFS= read -r -d '' image; do
    name="${image#"${bundles_src}/resources/backgrounds/"}"
    if head -c 40 "${image}" | grep -q '^version https://git-lfs'; then
        oid="$(sed -n 's/^oid sha256:\([0-9a-f]\{64\}\)$/\1/p' "${image}")"
        test -n "${oid}"
        curl -fsSL --retry 3 -o "${image}" "${bundles_media}/resources/backgrounds/${name}"
        echo "${oid}  ${image}" | sha256sum -c -
    fi
    # The same image as one already there (cosmic-wallpapers): no duplicate
    # in the wallpaper list, the bundle points to that one
    hash="$(sha256sum "${image}" | cut -d' ' -f1)"
    if [[ -n "${seen_wallpapers[${hash}]:-}" ]]; then
        bundle_wallpapers+=("s|/usr/share/backgrounds/cosmic/${name}\"|${seen_wallpapers[${hash}]}\"|")
    else
        install -D -m 0644 "${image}" "/usr/share/backgrounds/cosmic-desktop/${name}"
        seen_wallpapers[${hash}]="/usr/share/backgrounds/cosmic-desktop/${name}"
    fi
done < <(find "${bundles_src}/resources/backgrounds" -type f -print0)
cp -a "${bundles_src}/resources/icons/." /usr/share/icons/
for bundle in "${bundles_src}"/resources/default_schema/com.system76.CosmicThemeBundle.*; do
    slug="${bundle##*CosmicThemeBundle.}"
    slug="${slug%%.*}"
    if [[ "${slug}" == celenn-* ]]; then
        echo "Upstream bundle ${slug} would replace ours" >&2
        exit 1
    fi
done
for rewrite in "${bundle_wallpapers[@]}"; do
    sed -i "${rewrite}" "${bundles_src}"/resources/default_schema/com.system76.CosmicThemeBundle.*.Background/v1/*
done
cp -a "${bundles_src}/resources/default_schema/." /usr/share/cosmic/
grep -rlZF /usr/share/backgrounds/cosmic/ /usr/share/cosmic \
    | xargs -0 -r sed -i 's|/usr/share/backgrounds/cosmic/|/usr/share/backgrounds/cosmic-desktop/|g'
for background in /usr/share/cosmic/com.system76.CosmicThemeBundle.*.Background/v1/base; do
    source="$(sed -n 's|^ *source: Path("\(.*\)"),$|\1|p' "${background}")"
    if [[ ! -f "${source}" ]]; then
        echo "${background}: missing wallpaper ${source}" >&2
        exit 1
    fi
done
rm -rf "${bundles_src}"

# Celenn OS boot screen: Fedora's BGRT theme (the PC maker's logo from the
# firmware, so the boot stays flicker-free) with our images. When the firmware
# has no logo (virtual machines, some PCs), the Celenn OS mark is shown in
# its place (bgrt-fallback.png). The spinner is tinted blue and the Celenn OS
# logo replaces Fedora's at the bottom (watermark.png). The other images
# (password prompt...) are Fedora's spinner ones. The background stays black
# to match the firmware screen. The initramfs is regenerated in 60-initramfs.sh.
PLYMOUTH_THEME=/usr/share/plymouth/themes/celenn
for image in /usr/share/plymouth/themes/spinner/*.png; do
    [[ -e "${PLYMOUTH_THEME}/$(basename "${image}")" ]] || cp -a "${image}" "${PLYMOUTH_THEME}/"
done
sed -e 's|^Name=.*|Name=Celenn OS|' \
    -e 's|^Description=.*|Description=Celenn OS boot screen, based on the BGRT theme|' \
    -e "s|^ImageDir=.*|ImageDir=${PLYMOUTH_THEME}|" \
    /usr/share/plymouth/themes/bgrt/bgrt.plymouth > "${PLYMOUTH_THEME}/celenn.plymouth"
cat "${PLYMOUTH_THEME}/celenn.plymouth"
plymouth-set-default-theme celenn
# The login screen takes over from the boot screen without a black screen
# (drop-in cosmic-greeter.service.d/10-celenn-boot-screen.conf): it must
# no longer wait for Plymouth to quit. A drop-in cannot remove an ordering,
# so it is taken out of the packaged unit.
GREETER_UNIT=/usr/lib/systemd/system/cosmic-greeter.service
grep '^After=' "${GREETER_UNIT}"
grep -q '^After=.*plymouth-quit-wait\.service' "${GREETER_UNIT}"
sed -i -E -e '/^After=/ s/[[:space:]]*plymouth-quit-wait\.service//' -e '/^After=[[:space:]]*$/d' \
    "${GREETER_UNIT}"
grep '^After=' "${GREETER_UNIT}"
# Dark GTK defaults (zz1-celenn.gschema.override)
glib-compile-schemas /usr/share/glib-2.0/schemas
