#!/usr/bin/bash
# Update test: installs a published image (e.g. the current `stable`) to a
# virtual disk, boots it, logs a user in, then updates it to another
# published image the way an installed system does (pulled from GHCR inside
# the virtual machine, signature checked against the old image's
# /etc/containers/policy.json), reboots and checks that the updated system
# starts cleanly, keeps the user's files and settings in /etc, and shows the
# login screen and desktop. Finally rolls back and checks the old image boots
# again, as `bootc rollback` promises users.
#
# Usage: tests/vm-update-test.sh <from image> <to image> <output dir>
#   Both images are registry references, e.g.
#   ghcr.io/celenn-os/celenn:stable ghcr.io/celenn-os/celenn:20261106
# Needs: podman, skopeo, qemu-system-x86_64, OVMF, socat, openssh-client, jq, KVM.
set -euo pipefail
trap 'echo "::error::Command failed at line ${LINENO}: ${BASH_COMMAND}"' ERR

FROM="$1"
TO="$2"
OUT="$(realpath -m "$3")"
# shellcheck source=tests/vm-lib.sh
source "$(dirname "$0")/vm-lib.sh"

to_digest="$(skopeo inspect --format '{{.Digest}}' "docker://${TO}")"
log "Pulling ${FROM}"
sudo podman pull -q "${FROM}" >/dev/null
from_digest="$(sudo podman image inspect --format '{{.Digest}}' "${FROM}")"
echo "From: ${FROM} (${from_digest})"
echo "To:   ${TO} (${to_digest})"
if [[ "${from_digest}" == "${to_digest}" ]]; then
    log "Both images are the same (${to_digest}): nothing to update"
    exit 0
fi
errors=0

# 1. The old image, installed as from the ISO (which tracks the tag of FROM)
vm_install "${FROM}" "${FROM}"
sudo podman rmi -f "${FROM}" >/dev/null || true
vm_start
vm_wait_ssh
[[ "$(vm_booted_digest)" == "${from_digest}" ]] || fail "The installed system did not boot ${from_digest}"
# Problems of the old image are already known: warnings only
CHECK_LEVEL=warning vm_check_system before-update || true

# What is mounted on /boot: on an install where /boot is a directory of the
# root filesystem, an automount of the EFI system partition stacked there made
# ostree drop the staged update at shutdown (build_files/50-services.sh)
vm 'findmnt -R /boot; systemctl list-units --all --no-legend --plain "*.automount"' || true
# Images from before that fix: mask the generator as the fixed images do and
# boot again, so the rest of the update can still be tested. The update is
# finalized by the old system, so without this it would never apply.
gpt_auto=/etc/systemd/system-generators/systemd-gpt-auto-generator
if [[ "$(vm "readlink ${gpt_auto}" || true)" != /dev/null ]]; then
    echo "::warning::${FROM} predates the /boot automount fix: masking systemd-gpt-auto-generator before the update, as newer images do"
    vm "mkdir -p ${gpt_auto%/*} && ln -sf /dev/null ${gpt_auto}"
    vm_reboot
fi

# 2. Some user data and settings that the update must keep, and a logged in
# user, as on a real system
vm "echo kept > /etc/celenn-update-test"
if demo_user && demo_login 00-login-failed-before-update; then
    sleep 30
    vm "pkill -u ${DEMO_USER} -f cosmic-initial-setup" || true
    sleep 8
    mkdir -p "${OUT}/${SHOTS}"
    screenshot "${SHOTS}/00-before-update"
fi
vm "id ${DEMO_USER}" >/dev/null || fail "Could not create the test user"
vm "runuser -u ${DEMO_USER} -- sh -c 'echo kept > ~${DEMO_USER}/update-test'"
demo_journal before-update

# 3. The update, then a reboot into it
log "Updating to ${TO}"
# Through celenn-bootc when the old image records the download
# progress, as the Updates app does. The copy only exists while bootc runs:
# a watcher keeps the last one it saw
bootc_cmd=bootc
vm "grep -q update-progress /usr/libexec/celenn-bootc" 2>/dev/null \
    && bootc_cmd=/usr/libexec/celenn-bootc
vm "(while sleep 1; do cp /run/celenn/update-progress.jsonl /run/progress-seen 2>/dev/null; done) >/dev/null 2>&1 &
    watcher=\$!
    timeout 1800 ${bootc_cmd} switch --enforce-container-sigpolicy ${TO}; status=\$?
    kill \${watcher}
    exit \${status}" \
    || fail "bootc could not update to ${TO} (pull or signature check failed)"
if [[ "${bootc_cmd}" != bootc ]]; then
    seen="$(vm "cat /run/progress-seen" 2>/dev/null)" || seen=""
    echo "Download progress (last lines): $(tail -n 3 <<<"${seen}")"
    grep -q '"type":"ProgressBytes"' <<<"${seen}" \
        || { echo "::error::celenn-bootc did not record the download progress"; errors=1; }
    if vm "test -e /run/celenn/update-progress.jsonl"; then
        echo "::error::the download progress was not removed after the download"
        errors=1
    fi
fi
staged="$(vm 'bootc status --json' | jq -r '.status.staged.image.imageDigest // empty')"
[[ "${staged}" == "${to_digest}" ]] || fail "The staged update is '${staged}', not ${to_digest}"
# People reboot long after the update is staged, by which time idle
# automounts (2 minutes) have expired: wait as long before rebooting
sleep 180
vm_reboot

# 4. Checks of the updated system
if [[ "$(vm_booted_digest)" != "${to_digest}" ]]; then
    echo "::error::The system did not boot the update ${to_digest}"
    errors=1
    # The staged update is written to the boot loader at shutdown
    # (ostree-finalize-staged): its log from the previous boot
    vm 'journalctl -b -1 --no-pager -u ostree-finalize-staged.service -u ostree-finalize-staged-hold.service; journalctl -b -1 --no-pager -n 60' || true
fi
vm_check_system after-update || errors=1
# Older images turned FlatPark on by themselves: an update that brings
# celenn-flatpark-off turns it off (no app was installed from it)
if vm 'test -e /usr/libexec/celenn-flatpark-off' \
    && vm 'flatpak remotes --system --columns=name' | grep -qx flatpark; then
    echo "::error::FlatPark is still on after the update"
    errors=1
fi
vm "grep -qx kept /etc/celenn-update-test" \
    || { echo "::error::A change to /etc was lost in the update"; errors=1; }
vm "grep -qx kept ~${DEMO_USER}/update-test" \
    || { echo "::error::A file in the home folder was lost in the update"; errors=1; }
sleep 20
screenshot login-screen-after-update
# Screenshots of the updated system, for the user from the old image, who
# must still get a session that does not crash
desktop_tour || { echo "::error::The test user could not log in after the update"; errors=1; }
vm_check_crashes after-update || errors=1
demo_journal after-update

# 5. Rollback to the old image
log "Rolling back"
vm 'bootc rollback' || { echo "::error::bootc rollback failed"; errors=1; }
vm_reboot
[[ "$(vm_booted_digest)" == "${from_digest}" ]] \
    || { echo "::error::The rollback did not boot ${from_digest}"; errors=1; }
CHECK_LEVEL=warning vm_check_system after-rollback || true
vm "grep -qx kept ~${DEMO_USER}/update-test" \
    || { echo "::error::A file in the home folder was lost in the rollback"; errors=1; }

(( errors == 0 )) || fail "Update test failed (see the logs and screenshots in the artifacts)"
log "Update test passed: ${FROM} -> ${TO} -> rollback"
