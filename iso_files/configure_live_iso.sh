#!/usr/bin/env bash
# Titanoboa post-rootfs hook: turns the cosmic-blue image into a live session
# with the Anaconda installer. It runs inside the live root filesystem only;
# the installed system is the unmodified ghcr.io image embedded in the ISO.
# Adapted from projectbluefin/iso (iso_files/configure_iso_anaconda.sh).

set -eoux pipefail

IMAGE_INFO="$(cat /usr/share/ublue-os/image-info.json)"
IMAGE_TAG="$(jq -c -r '."image-tag"' <<<"$IMAGE_INFO")"
IMAGE_REF="$(jq -c -r '."image-ref"' <<<"$IMAGE_INFO")"
IMAGE_REF="${IMAGE_REF##*:docker://}"
IMAGE_REF="${IMAGE_REF##ostree-unverified-registry:}"
IMAGE_FLAVOR="$(jq -c -r '."image-flavor"' <<<"$IMAGE_INFO")"
IMAGE_NAME="$(jq -c -r '."image-name"' <<<"$IMAGE_INFO")"
# The tag the ISO was built from (stable, or latest for PR test builds) is the
# one embedded in the ISO and followed by installed systems. The workflow
# writes it into the titanoboa checkout, mounted at /app.
if [[ -s /app/cosmic-blue-image-tag ]]; then
    IMAGE_TAG="$(< /app/cosmic-blue-image-tag)"
fi

### Live session

# Services that make no sense in a live session (updates, first-boot setup,
# flatpak installs, VPN...).
for unit in \
    brew-setup.service \
    flatpak-nuke-fedora.service \
    flatpak-preinstall.service \
    rpm-ostree-countme.service \
    rpm-ostreed-automatic.timer \
    tailscaled.service \
    ublue-system-setup.service \
    uupd.timer; do
    systemctl disable "$unit" || true
done
for unit in podman-auto-update.timer ublue-user-setup.service; do
    systemctl --global disable "$unit" || true
done

### Anaconda

# Firefox: a browser in the live session (the installed system uses the flatpak).
# spice-vdagent: clipboard sharing when the live ISO runs in a VM (GNOME Boxes...).
dnf install -y \
    anaconda-live \
    firefox \
    spice-vdagent \
    libblockdev-btrfs \
    libblockdev-dm \
    libblockdev-lvm
# Same Firefox defaults as the installed system's flatpak
mkdir -p /etc/firefox/policies
ln -sf /usr/share/cosmic-blue/firefox/policies/policies.json /etc/firefox/policies/policies.json

# Profile matched on os-release (ID=fedora, VARIANT_ID=cosmic-blue or
# cosmic-blue-nvidia, the image name)
. /etc/os-release
# Images built before VARIANT_ID was set (such as stable until its next
# promotion) lack the line, and the installer then used Fedora's profile.
# Only the live system's copy changes: installed systems get the image as is.
if ! grep -q '^VARIANT_ID=' /usr/lib/os-release; then
    echo "VARIANT_ID=${IMAGE_NAME}" >> /usr/lib/os-release
fi
mkdir -p /etc/anaconda/profile.d
tee /etc/anaconda/profile.d/cosmic-blue.conf <<EOF
# Anaconda configuration file for cosmic-blue

[Profile]
profile_id = cosmic-blue

[Profile Detection]
os_id = fedora
variant_id = ${IMAGE_NAME}

[Network]
default_on_boot = FIRST_WIRED_WITH_LINK

[Bootloader]
efi_dir = fedora
menu_auto_hide = True

[Storage]
default_scheme = BTRFS
btrfs_compression = zstd:1
# Same layout as Bluefin and Aurora: one btrfs volume with /, /home and /var
default_partitioning =
    /     (min 1 GiB, max 70 GiB)
    /home (min 500 MiB, free 50 GiB)
    /var  (btrfs)

[User Interface]
hidden_spokes =
    NetworkSpoke

[Localization]
use_geolocation = False
EOF

echo "cosmic-blue release $VERSION_ID" >/etc/system-release
sed -i 's/ANACONDA_PRODUCTVERSION=.*/ANACONDA_PRODUCTVERSION=""/' /usr/{,s}bin/liveinst || true

# Installer shortcut: on the live user's desktop and first in the COSMIC dock.
# The installer is not started automatically.
sed -i 's/^Name=.*/Name=Install cosmic-blue/' /usr/share/applications/liveinst.desktop
# cosmic-blue icon (branding/icons/build.py) instead of Anaconda's
install -D -m 0644 /app/cosmic-blue-welcome/cosmic-blue-installer.svg \
    /usr/share/icons/hicolor/scalable/apps/cosmic-blue-installer.svg
sed -i 's/^Icon=.*/Icon=cosmic-blue-installer/' /usr/share/applications/liveinst.desktop
grep -qx 'Icon=cosmic-blue-installer' /usr/share/applications/liveinst.desktop
mkdir -p /etc/skel/Desktop
install -m 0755 /usr/share/applications/liveinst.desktop /etc/skel/Desktop/liveinst.desktop
dock=()
for app in liveinst org.mozilla.firefox firefox com.system76.CosmicFiles \
    com.system76.CosmicTerm com.system76.CosmicSettings; do
    if [[ -f "/usr/share/applications/${app}.desktop" ]]; then dock+=("\"${app}\""); fi
done
mkdir -p /usr/share/cosmic/com.system76.CosmicAppList/v1
(IFS=,; echo "[${dock[*]}]") >/usr/share/cosmic/com.system76.CosmicAppList/v1/favorites
cat /usr/share/cosmic/com.system76.CosmicAppList/v1/favorites

# Install the image embedded in the ISO (no download needed), then point the
# deployment at the registry so that `bootc upgrade` pulls updates from GHCR.
mkdir -p /usr/share/anaconda/post-scripts
tee -a /usr/share/anaconda/interactive-defaults.ks <<EOF
ostreecontainer --url=$IMAGE_REF:$IMAGE_TAG --transport=containers-storage --no-signature-verification
%include /usr/share/anaconda/post-scripts/install-configure-upgrade.ks
%include /usr/share/anaconda/post-scripts/disable-fedora-flatpak.ks
EOF
if [[ "${IMAGE_FLAVOR}" == nvidia* ]]; then
    echo '%include /usr/share/anaconda/post-scripts/secureboot-enroll-key.ks' \
        >> /usr/share/anaconda/interactive-defaults.ks
fi

tee /usr/share/anaconda/post-scripts/install-configure-upgrade.ks <<EOF
%post --erroronfail
bootc switch --mutate-in-place --enforce-container-sigpolicy --transport registry $IMAGE_REF:$IMAGE_TAG
%end
EOF

tee /usr/share/anaconda/post-scripts/disable-fedora-flatpak.ks <<'EOF'
%post --erroronfail
systemctl disable flatpak-add-fedora-repos.service || true
%end
EOF

# NVIDIA image: queue the Universal Blue akmods key (it signs the NVIDIA
# modules) for enrollment, as Bluefin's ISO does. With Secure Boot on, the
# firmware's MOK manager asks to confirm it on the first reboot (password
# "universalblue"); without Secure Boot, mokutil fails and nothing happens.
tee /usr/share/anaconda/post-scripts/secureboot-enroll-key.ks <<'EOF'
%post --erroronfail --nochroot
set -eu
readonly ENROLLMENT_PASSWORD="universalblue"
readonly SECUREBOOT_KEY="/etc/pki/akmods/certs/akmods-ublue.der"
if [[ ! -d /sys/firmware/efi ]]; then
    echo "EFI mode not detected. Skipping key enrollment."
    exit 0
fi
if [[ ! -f "${SECUREBOOT_KEY}" ]]; then
    echo "Secure Boot key not found: ${SECUREBOOT_KEY}"
    exit 0
fi
mokutil --timeout -1 || :
echo -e "${ENROLLMENT_PASSWORD}\n${ENROLLMENT_PASSWORD}" | mokutil --import "${SECUREBOOT_KEY}" || :
%end
EOF

### Welcome tour

# A short tour (iso_files/welcome) that opens with the live session: what
# cosmic-blue is, trying it live, highlights, and a button for the installer.
# The workflow copies its files into the titanoboa checkout, mounted at /app.
dnf install -y gjs gtk4 libadwaita
install -D -m 0755 /app/cosmic-blue-welcome/cosmic-blue-welcome.js /usr/libexec/cosmic-blue-welcome
install -D -m 0644 /app/cosmic-blue-welcome/io.github.cosmic_blue_os.Welcome.desktop \
    /usr/share/applications/io.github.cosmic_blue_os.Welcome.desktop
install -D -m 0644 /app/cosmic-blue-welcome/io.github.cosmic_blue_os.Welcome.desktop \
    /etc/xdg/autostart/io.github.cosmic_blue_os.Welcome.desktop
install -D -m 0644 /app/cosmic-blue-welcome/cosmic-blue-welcome.svg \
    /usr/share/icons/hicolor/scalable/apps/cosmic-blue-welcome.svg
gtk-update-icon-cache --force /usr/share/icons/hicolor || :
# COSMIC's first-run assistant is for installed systems: skip it in the live
# session so that it does not open on top of the tour.
mkdir -p /etc/skel/.config
touch /etc/skel/.config/cosmic-initial-setup-done
