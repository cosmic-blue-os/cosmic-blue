#!/usr/bin/bash
# Copy Bluefin's files (projectbluefin/common + ublue-os/brew), then ours,
# dropping everything that only matters for GNOME.
set -euox pipefail

# These base-main packages are replaced by projectbluefin/common files
# (same choice as Bluefin, see ublue-os/bluefin build_files/shared/build.sh).
readarray -t UBLUE_PKGS < <(rpm -qa --qf '%{NAME}\n' ublue-os-luks ublue-os-just ublue-os-udev-rules ublue-os-signing ublue-os-update-services)
if [[ "${#UBLUE_PKGS[@]}" -gt 0 ]]; then
    dnf5 -y remove "${UBLUE_PKGS[@]}"
fi

STAGE=/tmp/system_files
mkdir -p "${STAGE}"
cp -a /ctx/system_files/common/. "${STAGE}/"
cp -a /ctx/system_files/common-bluefin/. "${STAGE}/"
cp -a /ctx/system_files/brew/. "${STAGE}/"

# GNOME-specific files (or files for GNOME tools) and Bluefin branding we
# don't want in a COSMIC image.
GNOME_ONLY=(
    etc/dconf
    etc/gnome-initial-setup
    etc/environment                                    # GNOME_SHELL_SLOWDOWN_FACTOR
    etc/skel/.local/share/org.gnome.Ptyxis
    usr/lib/systemd/system/dconf-update.service
    usr/lib/systemd/user/bluefin-dynamic-wallpaper.service
    usr/lib/systemd/user/bluefin-dynamic-wallpaper.timer
    usr/libexec/bluefin-dynamic-wallpaper
    usr/lib/systemd/user/damask.service
    usr/lib/tmpfiles.d/damask-flatpak.conf
    usr/share/ublue-os/flatpak-overrides/app.drey.Damask
    usr/lib/systemd/user/compass.service
    usr/lib/systemd/user-preset/01-compass.preset
    usr/share/ublue-os/homebrew/preinstall.d/compass.Brewfile
    usr/share/glib-2.0/schemas/zz0-bluefin-modifications.gschema.override
    usr/share/xdg-terminal-exec/gnome-xdg-terminals.list
    usr/share/ublue-os/user-setup.hooks.d/10-theming.sh
    usr/share/ublue-os/user-setup.hooks.d/20-dynamic-wallpaper.sh
    usr/share/ublue-os/user-setup.hooks.d/25-damask-setup.sh
)
# Bluefin branding: wallpapers, the dinosaur boot (Plymouth) watermark and the
# Bluefin logos that replace Fedora's. Fedora's own files are kept instead.
GNOME_ONLY+=(
    usr/share/backgrounds/bluefin
    usr/share/gnome-background-properties
    usr/share/plymouth
    usr/share/pixmaps/fedora-gdm-logo.png
    usr/share/pixmaps/fedora-logo-icon.png
    usr/share/pixmaps/fedora-logo-small.png
    usr/share/pixmaps/fedora-logo-sprite.png
    usr/share/pixmaps/fedora-logo.png
    usr/share/pixmaps/fedora_logo_med.png
    usr/share/pixmaps/fedora_whitelogo_med.png
    usr/share/pixmaps/system-logo-white.png
)
# Bluefin's profile pictures (photos): Celenn OS ships its own avatars
# (system_files/usr/share/pixmaps/faces, made by branding/avatars).
GNOME_ONLY+=(
    usr/share/pixmaps/faces
)
# Bluefin's fastfetch logos (dinosaurs and a chicken), its global config and
# its Bluefin and Bazaar user counters: Celenn OS ships its own fastfetch
# (system_files/usr/bin/ublue-fastfetch and usr/share/ublue-os/fastfetch*).
GNOME_ONLY+=(
    etc/ublue-os/fastfetch.json
    usr/share/ublue-os/bluefin-logos
    usr/share/ublue-os/fastfetch-user-count
    usr/share/ublue-os/bazaar-install-count
)
# Bazaar, Bluefin's Flathub store: COSMIC Store already covers it (Flathub and
# the COSMIC repository), and one store is enough.
GNOME_ONLY+=(
    usr/share/flatpak/preinstall.d/bazaar.preinstall
    usr/share/ublue-os/bazaar
)
# Bluefin's Firefox prefs force hardware video decoding past Firefox's GPU
# blocklist: Firefox's own defaults are safer. Our Firefox defaults are
# policies (usr/lib/tmpfiles.d/celenn-firefox.conf).
GNOME_ONLY+=(
    usr/share/ublue-os/firefox-config
)
# More Bluefin leftovers that have no use on Celenn OS:
# - the rest of Bazaar (background service, config, flatpak override);
# - projectbluefin-countme, a usage counter for other Bluefin images that
#   exits at once on Celenn OS;
# - the AI agent notes about Bluefin's repositories;
# - get-geoclue-latitude, used only by Bluefin's GNOME dynamic wallpaper;
# - `ujust changelogs`, which shows Bluefin's release notes (Celenn OS's
#   are on its GitHub releases page);
# - opt-in Brewfiles for GNOME apps and Bluefin wallpapers, offered by
#   ChairLift only: Bluefin's "full desktop" GNOME Circle list, Hidamari and
#   Damask (video wallpaper and wallpaper slideshow, GNOME only).
GNOME_ONLY+=(
    etc/bazaar
    usr/lib/systemd/user/bazaar.service
    usr/lib/tmpfiles.d/bazaar-flatpak.conf
    usr/share/ublue-os/flatpak-overrides/io.github.kolunmi.Bazaar
    usr/libexec/projectbluefin-countme
    usr/lib/systemd/system/projectbluefin-countme.service
    usr/lib/systemd/system/projectbluefin-countme.timer
    usr/lib/systemd/system/timers.target.wants/projectbluefin-countme.timer
    usr/lib/systemd/system-preset/03-projectbluefin-countme.preset
    usr/share/ublue-os/agent
    usr/libexec/get-geoclue-latitude
    usr/share/ublue-os/just/changelog.just
    usr/share/ublue-os/homebrew/full-desktop.Brewfile
    usr/share/ublue-os/homebrew/video-wallpaper.Brewfile
    usr/share/ublue-os/homebrew/wallpaper-slideshow.Brewfile
    usr/share/ublue-os/homebrew/artwork.Brewfile
)
# Bluefin's community and help launchers, and its Report a Problem app, which
# files issues in Bluefin's repositories. Celenn OS's own Documentation
# launcher (system_files) opens the Celenn OS guide instead. Bluefin's
# icons of these launchers and of System Update go too: ours have their own.
GNOME_ONLY+=(
    usr/share/applications/discourse.desktop
    usr/share/applications/bluefin-help.desktop
    usr/share/applications/io.projectbluefin.Report.desktop
    usr/share/icons/hicolor/scalable/apps/io.projectbluefin.Report.svg
    usr/share/icons/hicolor/scalable/places/ublue-discourse.svg
    usr/share/icons/hicolor/scalable/places/ublue-docs.svg
    usr/share/icons/hicolor/scalable/places/ublue-update.svg
    usr/share/ublue-os/just/60-report.just
    usr/share/ublue-os/homebrew/preinstall.d/bluefin-report.Brewfile
)
# ChairLift ("Control Center"), Bluefin's settings and update app, with
# Bluefin's links and branding: its Homebrew cask (uninstalled from existing
# accounts by brew-preinstall once the Brewfile is gone), its config, launcher
# and icons, and its privileged helpers and their polkit actions. COSMIC
# Settings and ujust cover the same ground.
GNOME_ONLY+=(
    usr/share/ublue-os/homebrew/preinstall.d/chairlift.Brewfile
    usr/share/chairlift
    usr/share/applications/io.projectbluefin.chairlift.desktop
    usr/share/icons/hicolor/scalable/apps/io.projectbluefin.chairlift.svg
    usr/share/icons/hicolor/symbolic/apps/io.projectbluefin.chairlift-symbolic.svg
    usr/bin/chairlift-helper
    usr/libexec/bootc-update-stage
    usr/share/polkit-1/actions/io.projectbluefin.chairlift.bootc.policy
    usr/share/polkit-1/actions/io.projectbluefin.chairlift.ublue.policy
    usr/share/glib-2.0/schemas/io.projectbluefin.chairlift.livery.gschema.xml
    usr/share/glib-2.0/schemas/io.projectbluefin.chairlift.updates.gschema.xml
    usr/share/glib-2.0/schemas/io.projectbluefin.chairlift.firstrun.gschema.xml
    usr/share/glib-2.0/schemas/io.projectbluefin.chairlift.printers.gschema.xml
)
# FlatPark, the second Flatpak remote of projectbluefin/common: how far its
# apps can be trusted is not established yet, so Celenn OS offers it but
# leaves it off. Its descriptor moves out of remotes.d (which flatpak adds by
# itself) to where `ujust toggle-flatpark` finds it; its filter, which hides
# the apps that are also on Flathub, stays in /usr/share/flatpak. The filter
# copy service is dropped: it only serves Bazaar's sandbox, and `flatpak
# remote-add --filter` keeps its own copy of the filter in the repository.
install -d "${STAGE}/usr/share/celenn/flatpak"
mv "${STAGE}/usr/share/flatpak/remotes.d/flatpark.flatpakrepo" "${STAGE}/usr/share/celenn/flatpak/"
GNOME_ONLY+=(
    usr/lib/systemd/system/flatpark-filter-copy.path
    usr/lib/systemd/system/flatpark-filter-copy.service
)
sed -i '/flatpark-filter-copy/d' "${STAGE}/usr/lib/systemd/system-preset/02-flatpak-appstream-refresh.preset"
for path in "${GNOME_ONLY[@]}"; do
    rm -rf "${STAGE:?}/${path}"
done

# Bluefin-only ujust recipes: `contribute` (runs a worker for Bluefin's
# build hive), `bluespeed` (Bluefin's AI stack, `install-ai-tools` remains),
# `bluefin-apps` (alias of install-system-flatpaks), `bazaar-preview`, and
# `toggle-testing`, Bluefin's way to its testing tag, which `ujust
# switch-channel testing` replaces (with stable and dev). Also `bluefin-cli`, which `ujust install-cli-tools` replaces,
# and Bluefin's developer mode (`toggle-devmode` and its alias `devmode`):
# Celenn OS's own `ujust install-dev-tools` offers the same list as the tour
# and Tools.
python3 -I /ctx/build_files/drop-just-recipes.py \
    "${STAGE}/usr/share/ublue-os/just/shared.just" contribute
python3 -I /ctx/build_files/drop-just-recipes.py \
    "${STAGE}/usr/share/ublue-os/just/system.just" \
    bluespeed bluefin-apps bazaar-preview toggle-testing bluefin-cli devmode toggle-devmode
# Every listed ujust command reads verb-object, like Celenn OS's own in
# 60-custom.just: install- (adds apps or tools) and remove-,
# toggle- (on/off), reset-, check-, show-, switch-, report-... Bluefin's names
# that do not are renamed, regrouped (Apps installs things, System looks
# after the computer) or hidden when another command covers them.
# Bluefin's installers have no `remove`: they become hidden _bluefin-*
# recipes, which 60-custom.just's install-* commands run, next to its own
# remove-* commands. 60-custom.just keeps every old name as a hidden alias.
# Fails if upstream renames a recipe.
python3 -I /ctx/build_files/rename-just-recipes.py "${STAGE}"/usr/share/ublue-os/just/{apps,default,shared,system,update}.just -- \
    "setup-vms=_bluefin-vms>install-vms" \
    "setup-lima=_bluefin-lima>install-lima" \
    "cncf=_bluefin-cncf-tools>install-cncf-tools" \
    "install-ai-tools=_bluefin-ai-tools>install-ai-tools" \
    "install-asus=_bluefin-asus>install-asus" \
    "install-jetbrains-toolbox=_bluefin-jetbrains-toolbox>install-jetbrains-toolbox" \
    "install-opentabletdriver=_bluefin-opentabletdriver>install-opentabletdriver" \
    "install-system-flatpaks=reset-default-apps@Apps|Reinstall the default apps" \
    "bios=reboot-to-bios@System|Restart into the firmware settings (BIOS/UEFI)" \
    "bios-info=show-bios-info@System|Show the motherboard and firmware version" \
    "logs-this-boot=show-logs@System|Show the system messages since startup" \
    "logs-last-boot=show-previous-logs@System|Show the system messages of the previous startup" \
    "benchmark=run-benchmark@System|Run a one minute processor benchmark" \
    "check-sb-key=check-secure-boot@System|Show the Secure Boot state and keys" \
    "powerwash=reset-system@System|Factory reset: erases everything (experimental)" \
    "check-local-overrides@System|List the system settings changed in /etc" \
    "check-idle-power-draw@System|Measure the power used while idle" \
    "update@System|Update the system, apps and Homebrew packages now" \
    "toggle-updates@System|Turn automatic updates on or off" \
    "toggle-tpm2|Turn on or off disk unlocking with the TPM2 chip (no password)" \
    -jetbrains-toolbox -toggle-user-motd -device-info -toggle-vms

# `ujust install-cncf-tools` (projectbluefin/common cncf.Brewfile) installs
# OpenLens, which is end-of-life on Flathub (its open source version was
# retired): Headlamp, in the same list, is the Kubernetes UI it keeps.
# Fails if upstream changes it (tests/check-app-sources.sh warns about
# end-of-life apps every week).
python3 -I - "${STAGE}/usr/share/ublue-os/homebrew/cncf.Brewfile" <<'PY'
import pathlib, sys
path = pathlib.Path(sys.argv[1])
text = path.read_text()
old = '# OpenLens - Kubernetes IDE\nflatpak "dev.k8slens.OpenLens"\n'
if text.count(old) != 1 or 'flatpak "io.kinvolk.Headlamp"' not in text:
    sys.exit(f"{path}: OpenLens or Headlamp lines not found")
path.write_text(text.replace(old, "").replace("\n\n\n", "\n\n"))
PY

# `ujust enroll-secure-boot-key` (projectbluefin/common) queues the akmods key
# with Universal Blue's password: 60-custom.just has Celenn OS's own, with
# the password "celennos" that the ISO uses too. Its `ujust clean-system`
# asks question after question: 60-custom.just's runs the script of the
# "Clean up" button of Celenn OS Tools (/usr/libexec/celenn-clean).
python3 -I /ctx/build_files/drop-just-recipes.py \
    "${STAGE}/usr/share/ublue-os/just/default.just" enroll-secure-boot-key clean-system

# Bluefin's zsh profile forces every LC_* variable (and LC_ALL) to en_US for
# zsh login shells, which overrides the language chosen at install time.
sed -i '/^export LC_[A-Z_]*=en_US/d' "${STAGE}/etc/zsh/zprofile"

# `ujust` with no recipe prints a link to Bluefin's documentation first:
# point it to the Celenn OS guide.
sed -i 's|https://docs.projectbluefin.io/[^"]*|https://celenn-os.github.io/celenn/docs/|' \
    "${STAGE}/usr/share/ublue-os/just/00-entry.just"

# `ujust toggle-tpm2` (projectbluefin/common) runs /usr/bin/luks-tpm2-autounlock:
# common's script moves aside, and ours (system_files/usr/bin) runs extra
# checks (no TPM2 chip, Secure Boot off) before calling it.
install -d "${STAGE}/usr/libexec"
mv "${STAGE}/usr/bin/luks-tpm2-autounlock" "${STAGE}/usr/libexec/ublue-luks-tpm2-autounlock"

cp -a "${STAGE}/." /
# Only Celenn OS's avatars in the profile picture folder
rm -rf /usr/share/pixmaps/faces
cp -a /ctx/system_files/celenn/. /
# The wallpaper of the Celenn Dark and Celenn Light constellations (theme
# bundles written by build_files/theme): our per-theme default
for mode in Dark Light; do
    install -D -m 0644 "/usr/share/cosmic/com.system76.CosmicBackground.${mode}/v1/all" \
        "/usr/share/cosmic/com.system76.CosmicThemeBundle.celenn-${mode,,}.Background/v1/base"
done

# No Homebrew usage statistics (brew sends them unless told not to), for
# every shell: brew reads /etc/homebrew/brew.env itself. Appended, as the
# brew image may ship its own settings in that file.
install -d /etc/homebrew
grep -qsx 'HOMEBREW_NO_ANALYTICS=1' /etc/homebrew/brew.env \
    || echo 'HOMEBREW_NO_ANALYTICS=1' >>/etc/homebrew/brew.env

# Only accept key-signed Celenn OS images (cosign.pub, signed by build.yml).
# The scope is the whole namespace, so that switching between celenn and
# celenn-nvidia is verified too.
IMAGE="ghcr.io/${IMAGE_VENDOR}"
jq --arg image "$IMAGE" 'del(.transports.docker[$image + "/celenn"])
    | .transports.docker[$image] = [{
    "type": "sigstoreSigned",
    "keyPath": "/etc/pki/containers/celenn.pub",
    "signedIdentity": {"type": "matchRepository"}
}]' /etc/containers/policy.json > /tmp/policy.json
mv /tmp/policy.json /etc/containers/policy.json
mkdir -p /etc/containers/registries.d
cat > /etc/containers/registries.d/celenn.yaml <<EOF
docker:
  ${IMAGE}:
    use-sigstore-attachments: true
EOF

