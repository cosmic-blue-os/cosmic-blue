#!/usr/bin/bash
# Tests of the helpers that run as root through pkexec (system_files/usr/
# libexec: celenn-tools-helper, celenn-updates-helper, celenn-parental-helper,
# celenn-parental-extend). They are the privilege boundary: whatever the
# calling user passes, a helper must only do the few changes it documents.
#
# Each helper runs from a copy whose system paths point into a temporary
# folder, with the commands that change the system (bootc, mount,
# systemctl, useradd...) replaced by stubs that only log their arguments.
# The copy is refused if a system path is left in it, so nothing here
# touches the machine, even as root. Run by tests/lint.sh.
set -euo pipefail
cd "$(dirname "$0")/.."
LIBEXEC="${PWD}/system_files/usr/libexec"

T="$(mktemp -d)"
trap 'rm -rf "${T}"' EXIT
STUBS="${T}/stubs"
LOG="${T}/log"
mkdir -p "${STUBS}"
export LOG T

failures=0
pass=0
fail() {
    echo "FAIL: $*" >&2
    failures=$((failures + 1))
}
ok() { pass=$((pass + 1)); }

# --- Stubs ----------------------------------------------------------------
stub() { # name [body]
    printf '#!/usr/bin/bash\necho "%s $*" >> "${LOG}"\n%s\n' "$1" "${2:-exit 0}" > "${STUBS}/$1"
    chmod +x "${STUBS}/$1"
}
for name in flatpak rpm-ostree systemctl umount mount chown groupadd usermod \
    useradd chpasswd celenn-bootc; do
    stub "${name}"
done
stub busctl 'echo "o \"/org/freedesktop/Accounts/User1001\""'
stub systemd-escape 'echo "unit.mount"'
stub mountpoint 'exit "${MOUNTED:-1}"'
# Secure Boot off unless SECURE_BOOT=1; the driver key never enrolled
stub mokutil '[[ "$1" != --sb-state || "${SECURE_BOOT:-0}" != 1 ]] || echo "SecureBoot enabled"'
# bootc status prints $BOOTC_STATUS
stub bootc '[[ "$1" != status ]] || printf "%s\n" "${BOOTC_STATUS}"'
# fstab checks: the new file fails when VERIFY_NEW=1
stub findmnt '[[ "${VERIFY_NEW:-0}" == 1 && "$*" == *--verify*fstab.* ]] && exit 1; exit 0'
# Accounts: uid from UID_<name> (none: no such account), groups from
# GROUPS_<name>; numeric ids are their own group
stub id '
case "$1" in
    -u) var="UID_$2"; [[ -n "${!var:-}" ]] || exit 1; echo "${!var}" ;;
    -nG) var="GROUPS_$2"; echo "$2 ${!var:-}" ;;
    -g) echo "$2" ;;
esac'

# --- Copies of the helpers, with their paths in ${T} ----------------------
copy() { # helper sed-expression...
    local helper="$1" copy="${T}/$1"
    shift
    sed "$@" "${LIBEXEC}/${helper}" > "${copy}"
    chmod +x "${copy}"
    if grep -nE '(^|[ ="])/(etc|var|usr/libexec)/' "${copy}" | grep -v '^[0-9]*:#'; then
        echo "check-helpers.sh: ${helper} keeps a system path (above): add it to its copy" >&2
        exit 1
    fi
}
copy celenn-tools-helper \
    -e "s|^FSTAB=/etc/fstab$|FSTAB=${T}/fstab|" \
    -e "s|^MOUNT_ROOT=/var/mnt$|MOUNT_ROOT=${T}/mnt|" \
    -e "s|/var/lib/flatpak/overrides|${T}/overrides|g" \
    -e "s|/etc/pki/akmods/certs/akmods-ublue.der|${T}/key.der|"
copy celenn-updates-helper \
    -e "s|^HOLD=/var/lib/celenn/update-hold$|HOLD=${T}/hold/update-hold|" \
    -e "s|/usr/libexec/celenn-bootc|${STUBS}/celenn-bootc|g" \
    -e "s|/etc/pki/akmods/certs/akmods-ublue.der|${T}/key.der|"
copy celenn-parental-helper \
    -e "s|^CONFIG_DIR=/etc/celenn/parental-controls$|CONFIG_DIR=${T}/rules|"
copy celenn-parental-extend \
    -e "s|^STATE_DIR=/var/lib/celenn/parental-controls$|STATE_DIR=${T}/state|" \
    -e "s|/etc/celenn/parental-controls/|${T}/rules/|"
touch "${T}/key.der"

# Runs a helper copy with the stubs: sets $status and $output, empties the log
run() {
    : > "${LOG}"
    status=0
    output="$(PATH="${STUBS}:${PATH}" "${T}/$1" "${@:2}" 2>&1 < "${INPUT:-/dev/null}")" || status=$?
}
logged() { grep -qxF -- "$1" "${LOG}"; }

# --- Arguments: anything else is refused before any change ----------------
refused() { # helper args...
    run "$@"
    if [[ "${status}" != 2 || -s "${LOG}" ]]; then
        fail "$* not refused (status ${status}, ran: $(tr '\n' ';' < "${LOG}"))"
    else
        ok
    fi
}
refused celenn-tools-helper
refused celenn-tools-helper clean now
refused celenn-tools-helper driver-key x
refused celenn-tools-helper disks
refused celenn-tools-helper disks 1234-ABCD
refused celenn-tools-helper disks 1234-ABCD=maybe
refused celenn-tools-helper disks "../../etc=on"
refused celenn-tools-helper disks "1234-ABCD=on;reboot"
refused celenn-tools-helper disks "1234-ABCD=on" "x=off"
refused celenn-tools-helper format 1234-ABCD
refused celenn-updates-helper
refused celenn-updates-helper rollback now
refused celenn-updates-helper channel dev
refused celenn-updates-helper channel "stable;reboot"
refused celenn-updates-helper channel stable testing
refused celenn-updates-helper pin stable-2026
refused celenn-updates-helper pin latest
refused celenn-updates-helper pin "stable-20261124 x"
refused celenn-updates-helper edition amd
refused celenn-parental-helper
refused celenn-parental-helper set
refused celenn-parental-helper set Bob
refused celenn-parental-helper set ../root
refused celenn-parental-helper set "bob;id"
refused celenn-parental-helper add-child bob
refused celenn-parental-helper remove bob
refused celenn-parental-extend
refused celenn-parental-extend bob
refused celenn-parental-extend bob 4
refused celenn-parental-extend bob 241
refused celenn-parental-extend bob 05
refused celenn-parental-extend bob 10 forever
refused celenn-parental-extend bob day late

# --- celenn-tools-helper ----------------------------------------------------
run celenn-tools-helper clean
{ [[ "${status}" == 0 ]] && logged "flatpak uninstall --system --unused --noninteractive -y" \
    && logged "rpm-ostree cleanup --base --repomd"; } && ok || fail "clean: ${output}"

# A disk that is not there
printf 'UUID=0000-1111 / ext4 defaults 0 1\n' > "${T}/fstab"
run celenn-tools-helper disks 1234-ABCD=on
[[ "${status}" == 1 && "${output}" == "1234-ABCD not-found" ]] && ok || fail "missing disk: ${output}"

# Turning off our line only: the other lines stay, a copy is kept
mkdir -p "${T}/mnt/Games" "${T}/overrides"
cat > "${T}/fstab" <<'EOF'
UUID=0000-1111 / ext4 defaults 0 1
UUID=AAAA-BBBB /data ext4 defaults 0 2
UUID=1234-ABCD /var/mnt/Games ext4 defaults,nosuid,nodev,nofail,x-gvfs-show,x-celenn-disk 0 2
EOF
sed -i "s|/var/mnt/Games|${T}/mnt/Games|" "${T}/fstab"
printf '[Context]\nfilesystems=%s;home;\n\n[Environment]\nA=1\n' "${T}/mnt/Games" \
    > "${T}/overrides/com.valvesoftware.Steam"
printf '[Context]\nfilesystems=%s;\n' "${T}/mnt/Games" > "${T}/overrides/net.lutris.Lutris"
cp "${T}/fstab" "${T}/fstab.before"
run celenn-tools-helper disks AAAA-BBBB=off 1234-ABCD=off
if [[ "${status}" != 0 ]] || grep -q 1234-ABCD "${T}/fstab" || ! grep -q AAAA-BBBB "${T}/fstab" \
    || ! cmp -s "${T}/fstab.before" "${T}/fstab.celenn.bak" || [[ -e "${T}/mnt/Games" ]]; then
    fail "disk off: ${output}"; cat "${T}/fstab" >&2
else
    ok
fi
# The game launchers lose the folder, and keep their other settings
steam="$(cat "${T}/overrides/com.valvesoftware.Steam")"
[[ "${steam}" == $'[Context]\nfilesystems=home;\n\n[Environment]\nA=1' ]] && ok || fail "Steam override: ${steam}"
[[ ! -e "${T}/overrides/net.lutris.Lutris" ]] && ok || fail "empty Lutris override kept"

# An fstab that would not be valid any more is not written
cp "${T}/fstab.before" "${T}/fstab"
VERIFY_NEW=1 run celenn-tools-helper disks 1234-ABCD=off
{ [[ "${status}" == 1 && "${output}" == "1234-ABCD not-closed" ]] && cmp -s "${T}/fstab.before" "${T}/fstab"; } \
    && ok || fail "invalid fstab written: ${output}"

# Folder names from labels: no way out of the disks' folder
# shellcheck source=/dev/null
source <(sed -n '/^mount_name() {/,/^}/p' "${T}/celenn-tools-helper")
# shellcheck disable=SC2034 # read by mount_name
FSTAB="${T}/fstab" MOUNT_ROOT="${T}/mnt"
mkdir -p "${T}/mnt/Photos" && touch "${T}/mnt/Photos/x"
for case in "My Disk|1234|My-Disk" "../../etc|1234|etc" "|12345678ABCD|Disk-12345678" \
    "Été 2025|1|Été-2025" ".hidden|1|hidden" 'a/b\c$(id)|1|abcid' "Photos|1|Photos-2"; do
    IFS='|' read -r label uuid want <<<"${case}"
    got="$(mount_name "${label}" "${uuid}")"
    [[ "${got}" == "${want}" ]] && ok || fail "mount_name '${label}': ${got}, not ${want}"
done

# --- celenn-updates-helper --------------------------------------------------
status_json() { # image [rollback digest] [queued]
    jq -n --arg image "$1" --arg rollback "${2:-}" --argjson queued "${3:-false}" '{
        spec: {image: {image: $image}},
        status: {
            booted: {image: {imageDigest: "sha256:booted", version: "45.20261127"}},
            rollback: (if $rollback == "" then null else {image: {imageDigest: $rollback}} end),
            rollbackQueued: $queued
        }}'
}
mkdir -p "${T}/hold" && touch "${T}/hold/update-hold"
BOOTC_STATUS="$(status_json ghcr.io/celenn-os/celenn:stable)" run celenn-updates-helper channel testing
{ [[ "${status}" == 0 ]] && logged "celenn-bootc switch --enforce-container-sigpolicy ghcr.io/celenn-os/celenn:testing" \
    && [[ ! -e "${T}/hold/update-hold" ]]; } && ok || fail "channel testing: ${output}"
BOOTC_STATUS="$(status_json ghcr.io/celenn-os/celenn-nvidia:testing)" run celenn-updates-helper pin stable-20261124
logged "celenn-bootc switch --enforce-container-sigpolicy ghcr.io/celenn-os/celenn-nvidia:stable-20261124" \
    && ok || fail "pin: ${output}"
for image in ghcr.io/celenn-os-evil/celenn:stable quay.io/other/image:stable \
    ghcr.io/celenn-os/celenn@sha256:0000 ghcr.io/celenn-os/celenn ""; do
    BOOTC_STATUS="$(status_json "${image}")" run celenn-updates-helper channel stable
    { [[ "${status}" == 1 ]] && ! grep -q celenn-bootc "${LOG}"; } && ok || fail "image ${image:-none} followed: ${output}"
done
BOOTC_STATUS="$(status_json ghcr.io/celenn-os/celenn:stable)" run celenn-updates-helper edition nvidia
{ logged "celenn-bootc switch --enforce-container-sigpolicy ghcr.io/celenn-os/celenn-nvidia:stable" \
    && ! grep -q "mokutil --import" "${LOG}"; } && ok || fail "edition nvidia: ${output}"
SECURE_BOOT=1 BOOTC_STATUS="$(status_json ghcr.io/celenn-os/celenn:testing)" run celenn-updates-helper edition nvidia
logged "mokutil --import ${T}/key.der" && ok || fail "driver key not queued with Secure Boot: ${output}"
BOOTC_STATUS="$(status_json ghcr.io/celenn-os/celenn-nvidia:stable)" run celenn-updates-helper edition nvidia
{ [[ "${status}" == 0 ]] && ! grep -q celenn-bootc "${LOG}"; } && ok || fail "nvidia again: ${output}"
BOOTC_STATUS="$(status_json ghcr.io/celenn-os/celenn:stable sha256:previous)" run celenn-updates-helper rollback
{ logged "bootc rollback" && [[ "$(cat "${T}/hold/update-hold")" == "sha256:booted 45.20261127" ]]; } \
    && ok || fail "rollback: ${output}"
BOOTC_STATUS="$(status_json ghcr.io/celenn-os/celenn:stable)" run celenn-updates-helper rollback
{ [[ "${status}" == 1 ]] && ! logged "bootc rollback"; } && ok || fail "rollback without a previous version: ${output}"
BOOTC_STATUS="$(status_json ghcr.io/celenn-os/celenn:stable sha256:previous true)" run celenn-updates-helper rollback
{ [[ "${status}" == 0 ]] && ! logged "bootc rollback"; } && ok || fail "rollback queued twice: ${output}"

# --- celenn-parental-helper -------------------------------------------------
export UID_bob=1001 UID_admin=1000 GROUPS_admin=wheel UID_daemon=2
rules='{"limit":{"enabled":true,"weekdays":5000,"weekend":-3.5},"bedtime":{"enabled":false,"start":1260,"end":420},"web_filter":true,"apps":{"enabled":true,"allowed":["org.mozilla.firefox","a;b","../x",3,"org.mozilla.firefox"]},"extra":"x"}'
printf '%s\n' "${rules}" > "${T}/input"
INPUT="${T}/input" run celenn-parental-helper set bob
want='{"limit":{"enabled":true,"weekdays":1440,"weekend":0},"bedtime":{"enabled":false,"start":1260,"end":420},"web_filter":true,"apps":{"enabled":true,"allowed":["org.mozilla.firefox"]}}'
{ [[ "${status}" == 0 && "$(cat "${T}/rules/bob.json")" == "${want}" ]] \
    && logged "usermod -aG celenn-children bob" && logged "systemctl start celenn-parental-controls.service"; } \
    && ok || fail "set bob: ${output} $(cat "${T}/rules/bob.json" 2>/dev/null)"
for user in admin daemon nobody; do
    INPUT="${T}/input" run celenn-parental-helper set "${user}"
    { [[ "${status}" == 1 && ! -e "${T}/rules/${user}.json" ]] && ! grep -q usermod "${LOG}"; } \
        && ok || fail "rules for ${user}: ${output}"
done
for bad in '{"limit":{"enabled":"yes","weekdays":60,"weekend":60}}' 'not json' ''; do
    printf '%s\n' "${bad}" > "${T}/input"
    cp "${T}/rules/bob.json" "${T}/bob.before"
    INPUT="${T}/input" run celenn-parental-helper set bob
    { [[ "${status}" == 1 ]] && cmp -s "${T}/rules/bob.json" "${T}/bob.before"; } \
        && ok || fail "rules '${bad}' taken: ${output}"
done
printf 'secret\n%s\n' "${rules}" > "${T}/input"
for args in "léa|Léa|/etc/shadow" "lea|Léa|/usr/share/pixmaps/faces/../../../etc/shadow.png" \
    "lea|Léa:0:0|" "lea||" "bob|Bob|"; do
    IFS='|' read -r user name picture <<<"${args}"
    INPUT="${T}/input" run celenn-parental-helper add-child "${user}" "${name}" ${picture:+"${picture}"}
    { [[ "${status}" != 0 ]] && ! grep -q useradd "${LOG}"; } && ok || fail "add-child ${args}: ${output}"
done
printf '\n%s\n' "${rules}" > "${T}/input"
INPUT="${T}/input" run celenn-parental-helper add-child lea Léa
{ [[ "${status}" == 1 ]] && ! grep -q useradd "${LOG}"; } && ok || fail "add-child without a password: ${output}"

# --- celenn-parental-extend -------------------------------------------------
mkdir -p "${T}/state"
now="$(date +%s)"
printf '%s 30 bedtime\n%s 0\n' "$((now - 3 * 86400))" "$((now - 3600))" > "${T}/state/bob.extra"
run celenn-parental-extend bob 30 late
mapfile -t lines < "${T}/state/bob.extra"
{ [[ "${status}" == 0 && "${#lines[@]}" == 2 && "${lines[0]}" == "$((now - 3600)) 0" \
    && "${lines[1]}" =~ ^[0-9]+\ 30\ late$ ]]; } && ok || fail "extend bob 30 late: ${output} ${lines[*]}"
run celenn-parental-extend bob day
[[ "$(tail -n 1 "${T}/state/bob.extra")" =~ ^[0-9]+\ 0$ ]] && ok || fail "extend bob day: ${output}"
for user in lea ../bob; do
    run celenn-parental-extend "${user}" 30
    { [[ "${status}" == 1 ]] && [[ ! -e "${T}/state/${user}.extra" ]]; } && ok || fail "extend ${user}: ${output}"
done

echo "helpers: ${pass} checks passed, ${failures} failed"
((failures == 0))
