#!/usr/bin/bash
# Services enabled by Bluefin (build_files/base/17-cleanup.sh), minus dconf-update (GNOME).
set -euox pipefail

systemctl enable \
    brew-setup.service \
    celenn-flatpark-off.service \
    celenn-login-pictures.path \
    celenn-login-pictures.service \
    celenn-migrate-defaults.service \
    celenn-parental-controls.service \
    celenn-user-defaults.service \
    celenn-root-mount-options.service \
    celenn-signed-updates.timer \
    flatpak-nuke-fedora.service \
    flatpak-preinstall.service \
    input-remapper.service \
    rpm-ostree-countme.service \
    tailscaled.service \
    ublue-system-setup.service \
    uupd.timer

# Power management, already in base-main and enabled by Fedora's presets;
# enabled here too so a change in the base image cannot silently drop it.
# tuned-ppd is Fedora's power-profiles-daemon replacement: COSMIC's battery
# applet and Settings > Power use its D-Bus API to switch between power saver,
# balanced and performance, and on battery "balanced" uses tuned's
# balanced-battery profile (see /etc/tuned/ppd.conf). thermald (Intel CPUs
# only) keeps temperatures in check before the firmware throttles; it does not
# start in virtual machines.
systemctl enable \
    thermald.service \
    tuned.service \
    tuned-ppd.service

# Printing and scanning, already in base-main and enabled by Fedora's
# presets; enabled here too, like power management. Avahi finds the network
# printers and scanners (driverless IPP, eSCL) that CUPS and sane-airscan use.
systemctl enable \
    avahi-daemon.service \
    cups.socket

# podman.socket: rootless Docker-compatible API, used by the docker CLI through
# the "podman" context (user-setup hook 30-docker-podman.sh)
systemctl --global enable \
    podman-auto-update.timer \
    podman.socket \
    ublue-user-setup.service

# Speech for the screen reader (Orca): Fedora's presets do not enable
# speech-dispatcher's socket yet (rhbz#2284507, change pending in
# redhat-systemd-presets), so Orca can come up silent. Aurora enables it too.
systemctl --global enable speech-dispatcher.socket

# AppImages added with Gear Lever update by themselves once a day, like the
# system and flatpaks (and stop when `ujust toggle-updates` turns uupd off)
systemctl --global enable celenn-appimage-update.timer

# Files trashed more than 30 days ago go, and a notification warns when the
# disk is almost full (Celenn OS Tools turns both off)
systemctl --global enable celenn-housekeeping.timer

# LocalSend (a first-boot app; the tour's "Send without a cable") finds
# phones on the same network and receives their files on port 53317, which
# the default zone (public) closes. The port only answers while LocalSend is
# open, and LocalSend asks before it accepts files. Our service definition,
# unless firewalld ships its own.
if [[ ! -e /usr/lib/firewalld/services/localsend.xml ]]; then
    install -m 0644 /usr/share/celenn/firewalld/localsend.xml /usr/lib/firewalld/services/localsend.xml
fi
firewall-offline-cmd --zone=public --add-service=localsend

# Keep systemd-gpt-auto-generator from stacking an automount of the EFI
# system partition on /boot. GRUB tells systemd which disk it booted from
# (LoaderDevicePartUUID), so on installs where /boot is a directory of the
# root filesystem (bootc install to-disk, disk images) the generator adds
# boot.automount on top of ostree's /boot bind mount. Once that automount
# idles out (2 minutes), ostree cannot remount /boot read-write at shutdown
# ("Remounting /boot read-write: Invalid argument") and silently drops the
# staged update: the system reboots into the old image. Fixed upstream in
# ostree 84188e3 (2026-11-06, bootc-dev/bootc#2402).
# Nothing else of the generator is used: bootc sets root= on the kernel
# command line, bootupd mounts the ESP itself, and there is no swap, /home or
# /srv partition to discover. Installs from the ISO normally have a separate
# /boot partition, which this does not concern. Remove this once the base
# image ships an ostree with that fix.
mkdir -p /etc/systemd/system-generators
ln -sf /dev/null /etc/systemd/system-generators/systemd-gpt-auto-generator

# uupd replaces rpm-ostree automatic updates
systemctl disable rpm-ostreed-automatic.timer || true

# Flathub and the COSMIC flatpak repository (COSMIC applets and apps from
# System76) instead of the Fedora flatpak remote, both shown in COSMIC Store.
# Files in /usr/share/flatpak/remotes.d are added by flatpak itself the first
# time it opens the system installation, so systems installed before this
# change get them too; the remotes are also added now for new installs.
mkdir -p /usr/share/flatpak/remotes.d
curl --fail --retry 3 -sSL -o /usr/share/flatpak/remotes.d/flathub.flatpakrepo \
    https://dl.flathub.org/repo/flathub.flatpakrepo
curl --fail --retry 3 -sSL -o /usr/share/flatpak/remotes.d/cosmic.flatpakrepo \
    https://apt.pop-os.org/cosmic/cosmic.flatpakrepo
for remote in flathub cosmic; do
    flatpak remote-add --system --if-not-exists "${remote}" \
        "/usr/share/flatpak/remotes.d/${remote}.flatpakrepo"
done
systemctl disable flatpak-add-fedora-repos.service || true
rm -f /usr/lib/systemd/system/flatpak-add-fedora-repos.service

# Hide a few useless menu entries (like Bluefin)
for app in fish htop nvtop; do
    if [[ -f "/usr/share/applications/${app}.desktop" ]]; then
        sed -i 's@\[Desktop Entry\]@[Desktop Entry]\nHidden=true@g' "/usr/share/applications/${app}.desktop"
    fi
done
