#!/usr/bin/bash
# ISO install test: boots the live ISO in a virtual machine with an empty
# disk, installs cosmic-blue with the live session's Anaconda installer
# (automated with a kickstart that keeps the ISO's own installer settings:
# embedded image, partitioning, post-install scripts), then boots the
# installed system and checks it the way a person would see it: clean boot,
# image followed for updates, btrfs layout, keyboard layout and clock format
# chosen in the installer, user created by the installer able to log in.
#
# Usage: tests/vm-iso-install-test.sh <iso> <image ref> <output dir>
#   <image ref>: the image installed systems should follow, e.g.
#   ghcr.io/cosmic-blue-os/cosmic-blue:stable
# Needs: qemu-system-x86_64, OVMF, xorriso, socat, openssh-client, jq, KVM.
set -euo pipefail
trap 'echo "::error::Command failed at line ${LINENO}: ${BASH_COMMAND}"' ERR

ISO="$(realpath "$1")"
IMAGE_REF="$2"
OUT="$(realpath -m "$3")"
# shellcheck source=tests/vm-lib.sh
source "$(dirname "$0")/vm-lib.sh"

# The installer's choices, checked on the installed system: the maintainer's
# Swiss French keyboard and a language with a 24-hour clock
KS_LANG=fr_CH.UTF-8
KS_XLAYOUT="ch (fr)"
INSTALL_TIMEOUT=7200
errors=0
error() { echo "::error::$*"; errors=1; }

ssh-keygen -q -t ed25519 -N '' -f "${WORK}/id_ed25519"
pubkey="$(cat "${WORK}/id_ed25519.pub")"

# 1. Boot the live ISO. Its kernel and initramfs are started directly, with
# the ISO's own boot arguments (boot/grub/grub.cfg) plus a serial console and
# SSH for the test. The SSH key reaches root through a systemd credential
# (QEMU SMBIOS), the live system has no password to log in with.
log "Reading the boot files of ${ISO##*/}"
mkdir -p "${WORK}/iso"
xorriso -osirrox on -indev "${ISO}" \
    -extract /boot/grub/grub.cfg "${WORK}/iso/grub.cfg" \
    -extract /boot/vmlinuz "${WORK}/iso/vmlinuz" \
    -extract /boot/initramfs.img "${WORK}/iso/initramfs.img" 2>/dev/null
live_args="$(awk '$1 == "linux" { $1 = ""; $2 = ""; print; exit }' "${WORK}/iso/grub.cfg")"
live_args="$(sed -E 's/(^| )(quiet|rhgb)( |$)/ /g; s/  +/ /g; s/^ //' <<< "${live_args}")"
[[ "${live_args}" == *root=live:* ]] || fail "No live boot entry in the ISO's grub.cfg"
echo "Live boot arguments: ${live_args}"

truncate -s 64G "${WORK}/disk.raw"
export VM_CDROM="${ISO}" VM_MEMORY=12288 VM_CPUS=4 VM_SERIAL=serial-live.log
export VM_KERNEL="${WORK}/iso/vmlinuz" VM_INITRD="${WORK}/iso/initramfs.img"
export VM_APPEND="${live_args} console=tty0 console=ttyS0,115200 systemd.wants=sshd.service"
VM_EXTRA_ARGS=(-smbios "type=11,value=io.systemd.credential:ssh.authorized_keys.root=${pubkey}")
vm_start
vm_wait_ssh no-ssh-live
vm 'cat /proc/cmdline; findmnt -n -o SOURCE,FSTYPE /run/rootfsbase || true'

# The live desktop, as it opens for a person trying cosmic-blue
mkdir -p "${OUT}/${SHOTS}"
if wait_for 300 'pgrep -x cosmic-panel'; then
    sleep 30
else
    echo "::warning::The live session did not start a COSMIC desktop"
fi
screenshot "${SHOTS}/00-live-session"

# 2. Automated install. The kickstart includes the installer settings of the
# live ISO (iso_files/configure_live_iso.sh), then gives the answers a person
# would give in the installer.
cat > "${WORK}/ks.cfg" <<EOF
%include /usr/share/anaconda/interactive-defaults.ks

lang ${KS_LANG}
keyboard --xlayouts='${KS_XLAYOUT}'
timezone Europe/Zurich --utc
ignoredisk --only-use=vda
zerombr
clearpart --all --initlabel --disklabel=gpt
# The profile's layout (btrfs with /, /home and /var)
autopart
user --name=${DEMO_USER} --password=${DEMO_PASSWORD} --plaintext --groups=wheel --gecos=cosmic-blue
# Only for the test: SSH for the checks, boot messages on the serial log
sshkey --username=root "${pubkey}"
services --enabled=sshd
bootloader --append="console=tty0 console=ttyS0,115200"
# A text-mode install would make the system boot to a text console; the
# graphical installer a person uses boots it to the login screen
xconfig --startxonboot
poweroff

# NVIDIA image: the virtual machine cannot answer the firmware's prompt to
# enroll the Secure Boot key (it has no Secure Boot), so withdraw the request.
%post --nochroot
mokutil --revoke-import 2>/dev/null || :
%end
EOF
# The installer shortcut runs liveinst, which restarts as root and starts
# Anaconda's graphical interface in the live user's session, with
# --liveinst. In that mode Anaconda only reads interactive-defaults.ks and
# never runs unattended, so here it runs in text mode without --liveinst,
# with the kickstart above: same installer settings, payload, storage and
# post-install scripts. Its own session bus (it inhibits the screensaver on
# one) keeps it away from the live desktop.
cat > "${WORK}/install.sh" <<'EOF'
#!/usr/bin/bash
unset WAYLAND_DISPLAY DISPLAY
anaconda-cleanup anaconda || true
exec dbus-run-session -- anaconda --cmdline --kickstart=/root/ks.cfg
EOF
vm 'cat > /root/ks.cfg' < "${WORK}/ks.cfg"
vm 'cat > /root/install.sh && chmod +x /root/install.sh' < "${WORK}/install.sh"

collect_live_logs() {
    mkdir -p "${OUT}/anaconda-live"
    vm 'tar -cz -C / root/install.log root/ks.cfg $(cd / && ls -d tmp/*.log tmp/anaconda-tb-* 2>/dev/null)' \
        | tar -xz -C "${OUT}/anaconda-live" 2>/dev/null || true
    vm 'journalctl -b --no-pager' > "${OUT}/journal-live.log" 2>/dev/null || true
    # Shown in the job log too, as artifacts are not always at hand
    vm 'for f in /root/install.log /tmp/anaconda.log /tmp/program.log /tmp/storage.log /tmp/packaging.log; do
            [ -f "$f" ] && { echo "== $f"; tail -n 40 "$f"; }; done; ls /tmp/anaconda-tb-* 2>/dev/null' || true
}

log "Installing from the live ISO"
started=${SECONDS}
vm 'systemd-run --unit=cosmic-blue-install --service-type=exec \
    -p StandardOutput=file:/root/install.log -p StandardError=file:/root/install.log \
    /root/install.sh'
# Anaconda powers the virtual machine off when the install succeeds
last_progress=""
last_stats=${SECONDS}
while vm_running; do
    if (( SECONDS - started > INSTALL_TIMEOUT )); then
        screenshot install-timeout
        collect_live_logs
        fail "The install did not finish within $((INSTALL_TIMEOUT / 60)) minutes"
    fi
    state="$(vm 'systemctl show -p ActiveState --value cosmic-blue-install' 2>/dev/null || true)"
    if [[ "${state}" == failed || "${state}" == inactive ]]; then
        sleep 30  # a successful install powers off right after Anaconda exits
        vm_running || break
        screenshot install-failed
        collect_live_logs
        fail "The installer stopped without powering off (unit ${state}): the install failed, see anaconda-live in the artifacts"
    fi
    progress="$(vm 'tail -n 1 /tmp/anaconda.log' 2>/dev/null | cut -c 1-200 || true)"
    if [[ -n "${progress}" && "${progress}" != "${last_progress}" ]]; then
        echo "[$(( (SECONDS - started) / 60 )) min] ${progress}"
        last_progress="${progress}"
    fi
    # Every 5 minutes: is the image deploy writing to the disk, or stuck?
    if (( SECONDS - last_stats >= 300 )); then
        last_stats=${SECONDS}
        echo "[$(( (SECONDS - started) / 60 )) min] $(vm 'echo "written: $(df -BM --output=used /mnt/sysimage 2>/dev/null | tail -n 1)"; \
            free -m | awk "/^Mem/ { print \"memory used/available MB: \" \$3 \"/\" \$7 }"; \
            ps -eo pcpu,stat,etime,args --sort=-pcpu | head -n 4 | cut -c 1-160; \
            dmesg | grep -iE "oom|hung task|blocked for" | tail -n 3' 2>/dev/null || true)"
    fi
    sleep 20
done
log "Install finished in $(( (SECONDS - started) / 60 )) minutes"

# 3. Boot the installed system as the firmware chooses (the boot entry made by
# the installer), with the ISO still in the drive
unset VM_KERNEL VM_INITRD VM_APPEND
VM_EXTRA_ARGS=()
export VM_MEMORY=4096 VM_CPUS=2 VM_SERIAL=serial.log
vm_start
vm_wait_ssh no-ssh-installed
vm 'cp -r /var/log/anaconda /tmp/anaconda-logs && chmod -R a+rX /tmp/anaconda-logs' || true
mkdir -p "${OUT}/anaconda"
vm 'tar -cz -C /tmp/anaconda-logs .' | tar -xz -C "${OUT}/anaconda" 2>/dev/null || true

# 4. Checks of the installed system
# The installer used the cosmic-blue profile (iso_files/configure_live_iso.sh:
# btrfs layout, no network spoke...), matched on VARIANT_ID in os-release
grep -q "The 'cosmic-blue' profile is detected" "${OUT}"/anaconda/anaconda.log 2>/dev/null \
    || error "The installer did not use the cosmic-blue profile: $(grep -ho "The '[^']*' profile is detected" "${OUT}"/anaconda/anaconda.log 2>/dev/null || echo 'anaconda.log missing')"
if vm 'findmnt -n -o SOURCE /run/rootfsbase' >/dev/null 2>&1; then
    fail "The live ISO booted again instead of the installed system"
fi
booted="$(vm 'bootc status --json' | jq -r '.status.booted.image.image | "\(.transport) \(.image)"')"
[[ "${booted}" == "registry ${IMAGE_REF}" ]] \
    || error "The installed system follows '${booted}', not 'registry ${IMAGE_REF}': updates would not come from GHCR"
vm_check_system installed || errors=1

fstype="$(vm 'findmnt -n -o FSTYPE /sysroot' || true)"
[[ "${fstype}" == btrfs ]] || error "The system is on '${fstype}', not btrfs"
vm 'findmnt -n -o OPTIONS /sysroot | grep -q compress=zstd:1' \
    || echo "::warning::btrfs is not mounted with compress=zstd:1"
vm 'findmnt -n -o FSTYPE /var/home | grep -qx btrfs' || error "/var/home is not on btrfs"
vm 'cat /etc/fstab; findmnt -o TARGET,SOURCE,FSTYPE,OPTIONS -R /sysroot; findmnt /; findmnt /var; findmnt /var/home; lsblk -f' || true

# Keyboard layout and clock format from the installer, given to COSMIC by
# cosmic-blue-user-defaults before the login screen
vm 'localectl status' || true
xkb="$(vm "cat ~${DEMO_USER}/.config/cosmic/com.system76.CosmicComp/v1/xkb_config" || true)"
echo "xkb_config of ${DEMO_USER}: ${xkb}"
grep -q 'layout: "ch"' <<< "${xkb}" && grep -q 'variant: "fr"' <<< "${xkb}" \
    || error "COSMIC did not get the keyboard layout chosen in the installer (${KS_XLAYOUT})"
[[ "$(vm "cat ~${DEMO_USER}/.config/cosmic/com.system76.CosmicAppletTime/v1/military_time" || true)" == true ]] \
    || error "COSMIC did not get the 24-hour clock of ${KS_LANG}"
vm "id -nG ${DEMO_USER} | grep -qw wheel" || error "The user made in the installer is not an administrator"

vm 'timeout 300 bootc upgrade --check' \
    || echo "::warning::bootc upgrade --check failed on the installed system"

# The user made in the installer can log in. Only one screenshot: the full
# desktop tour already runs in the boot test of every build (vm-boot-test.sh).
if demo_user && demo_login 01-login-failed; then
    sleep 30
    screenshot "${SHOTS}/01-installed-desktop"
else
    error "The user made in the installer could not log in"
fi
demo_journal

(( errors == 0 )) || fail "ISO install test failed (see the logs and screenshots in the artifacts)"
log "ISO install test passed: ${ISO##*/} installed and booted"
