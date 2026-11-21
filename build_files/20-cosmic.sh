#!/usr/bin/bash
# COSMIC desktop, from the official Fedora repositories only.
set -euox pipefail

# For the build log: COSMIC groups available in this Fedora release
dnf5 group list --hidden | grep -i cosmic || true

# Comps group used by Fedora COSMIC (spin and Atomic). No COPR is enabled here,
# so COSMIC packages necessarily come from Fedora (checked in 99-tests.sh).
# @cosmic-desktop-apps is skipped: on Fedora it pulls KDE/GNOME apps
# (Okular, Rhythmbox, Nheko...) that we replace with flatpaks, like Bluefin.
dnf5 -y install \
    @cosmic-desktop \
    cosmic-edit \
    cosmic-files \
    cosmic-player \
    cosmic-store \
    cosmic-term

# The screen reader that COSMIC starts with Super + Alt + S (shown on the
# first page of our tour): named here so the image never ships without it
dnf5 -y install orca

# The COSMIC version cosmic-blue is tested against (also checked by
# `just helpers-keys` before the helper stages): a new version comes through
# the pull request of check-cosmic-version.yml, which moves cosmic-version.
pinned="$(grep -v '^#' /ctx/build_files/cosmic-version | tr -d '[:space:]')"
installed="$(rpm -q --qf '%{VERSION}' cosmic-settings)"
if [[ "${installed}" != "${pinned}" ]]; then
    echo "COSMIC ${installed} installed, but cosmic-blue is pinned to ${pinned} (build_files/cosmic-version)" >&2
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
# account, and cosmic-blue sets the appearance and panel layout. It exits at
# once when ~/.config/cosmic-initial-setup-done exists, which new accounts get
# from /etc/skel. Its accessibility page is replaced by the accessibility row
# on the first page of our tour below. (Its OEM mode, run by a
# "cosmic-initial-setup" system user whose home is not made from /etc/skel,
# is not used on cosmic-blue: Anaconda creates the account.)
mkdir -p /etc/skel/.config
touch /etc/skel/.config/cosmic-initial-setup-done

# cosmic-blue's own first steps tour (/usr/libexec/cosmic-blue-tour, built
# with libcosmic in Containerfile.helpers), opened once at the first login
# of each new account: the autostart entry only acts while
# ~/.config/cosmic-blue/first-steps-pending exists, which new accounts get
# from /etc/skel (see /usr/libexec/cosmic-blue-first-steps-autostart).
mkdir -p /etc/skel/.config/cosmic-blue
touch /etc/skel/.config/cosmic-blue/first-steps-pending

# The Updates launcher (cosmic-blue Tools on its Updates page, built with the
# tour) replaces the "System Update" launcher of Bluefin's shared files, which
# ran `ujust update` in a terminal: its "Check now" runs the same update
# (uupd-manual.service), and `ujust update` stays for the terminal.
# /usr/libexec/cosmic-blue-updates, its earlier program (the tour, tests),
# opens the same page.
rm -f /usr/share/applications/system-update.desktop
ln -sf cosmic-blue-tools /usr/libexec/cosmic-blue-updates

systemctl set-default graphical.target

# Fedora's default wallpaper for this release (day/night), offered in COSMIC
# Settings next to the cosmic-blue one. (Fedora 44 has no extras package.)
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
# deployed image): move COSMIC's own wallpapers after cosmic-blue's so that
# ours come first ("cosmic" sorts before "cosmic-blue").
if [[ -d /usr/share/backgrounds/cosmic ]]; then
    mv /usr/share/backgrounds/cosmic /usr/share/backgrounds/cosmic-desktop
    if grep -rlF /usr/share/backgrounds/cosmic/ /usr/share/cosmic /etc 2>/dev/null; then
        echo "These files point to the moved COSMIC wallpapers" >&2
        exit 1
    fi
fi
echo "Wallpapers kept:"
find /usr/share/backgrounds -printf '%y %p %l\n' | sort

# For the build log: the dock favorites and shortcuts as packaged by Fedora
cat /usr/share/cosmic/com.system76.CosmicAppList/v1/favorites || true
ls /usr/share/cosmic/com.system76.CosmicSettings.Shortcuts/v1/ || true

# Defaults that accounts may still hold as a copy from an earlier image:
# Fedora's packaged COSMIC defaults (before ours replace them), COSMIC's
# earlier default themes and built-in defaults (build_files/previous_defaults).
# cosmic-blue-migrate-defaults resets such copies when a default changes.
PREVIOUS_DEFAULTS=/usr/share/cosmic-blue/previous-defaults.list
mkdir -p "$(dirname "${PREVIOUS_DEFAULTS}")"
{
    /usr/libexec/cosmic-blue-migrate-defaults --hash-tree /usr/share/cosmic
    for variant in /ctx/previous_defaults/theme/*/cosmic /ctx/build_files/previous_defaults/*/; do
        /usr/libexec/cosmic-blue-migrate-defaults --hash-tree "${variant}"
    done
} | sort -u >"${PREVIOUS_DEFAULTS}"
wc -l "${PREVIOUS_DEFAULTS}"

# cosmic-blue defaults (wallpaper, theme, dock, extra shortcuts) and boot
# screen (Plymouth theme)
# were copied before these packages were installed; apply them again so they
# win over the packaged files.
cp -a /ctx/system_files/cosmic-blue/usr/share/cosmic/. /usr/share/cosmic/
cp -a /ctx/system_files/cosmic-blue/usr/share/plymouth/. /usr/share/plymouth/

# cosmic-blue boot screen: Fedora's BGRT theme (the PC maker's logo from the
# firmware, so the boot stays flicker-free) with our images. When the firmware
# has no logo (virtual machines, some PCs), the cosmic-blue mark is shown in
# its place (bgrt-fallback.png). The spinner is tinted blue and the cosmic-blue
# logo replaces Fedora's at the bottom (watermark.png). The other images
# (password prompt...) are Fedora's spinner ones. The background stays black
# to match the firmware screen. The initramfs is regenerated in 60-initramfs.sh.
PLYMOUTH_THEME=/usr/share/plymouth/themes/cosmic-blue
for image in /usr/share/plymouth/themes/spinner/*.png; do
    [[ -e "${PLYMOUTH_THEME}/$(basename "${image}")" ]] || cp -a "${image}" "${PLYMOUTH_THEME}/"
done
sed -e 's|^Name=.*|Name=cosmic-blue|' \
    -e 's|^Description=.*|Description=cosmic-blue boot screen, based on the BGRT theme|' \
    -e "s|^ImageDir=.*|ImageDir=${PLYMOUTH_THEME}|" \
    /usr/share/plymouth/themes/bgrt/bgrt.plymouth > "${PLYMOUTH_THEME}/cosmic-blue.plymouth"
cat "${PLYMOUTH_THEME}/cosmic-blue.plymouth"
plymouth-set-default-theme cosmic-blue
# Dark GTK defaults (zz1-cosmic-blue.gschema.override)
glib-compile-schemas /usr/share/glib-2.0/schemas
