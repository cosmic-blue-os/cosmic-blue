#!/usr/bin/bash
# Shared helpers of the virtual machine tests (vm-boot-test.sh,
# vm-update-test.sh, vm-iso-install-test.sh): install an image to a virtual disk with `bootc install`,
# boot it in QEMU (UEFI with Secure Boot, KVM), run commands over SSH, check the system,
# reboot, take screenshots and log in to the desktop.
#
# Source it after setting OUT (output directory); it sets WORK (scratch
# directory, removed on exit).
# Needs: podman, qemu-system-x86_64, OVMF, socat, openssh-client, jq, KVM.

TESTS_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]}")")"
WORK="$(mktemp -d -p "${RUNNER_TEMP:-/var/tmp}" celenn-vm.XXXXXX)"
SSH_PORT=2222
mkdir -p "${OUT}"

log() { echo "::notice::$*"; }
fail() { echo "::error::$*"; exit 1; }

cleanup() {
    if [[ -f "${WORK}/qemu.pid" ]]; then
        sudo kill "$(cat "${WORK}/qemu.pid")" 2>/dev/null || true
    fi
    sudo rm -rf "${WORK}"
}
trap cleanup EXIT

# Installs an image to a raw disk, as a user would with a bootc installer.
# Usage: vm_install <image in the root podman storage> <target image ref>
# console=ttyS0: boot messages go to the serial log.
# systemd.wants=sshd.service: SSH for the checks, only in these tests (bootc
# keeps the kernel arguments across updates).
vm_install() {
    local image="$1" target="$2"
    ssh-keygen -q -t ed25519 -N '' -f "${WORK}/id_ed25519"
    truncate -s 40G "${WORK}/disk.raw"
    log "Installing ${image} to a virtual disk"
    sudo podman run --rm --privileged --pid=host \
        --security-opt label=type:unconfined_t \
        -v /var/lib/containers:/var/lib/containers \
        -v /dev:/dev \
        -v "${WORK}:/output" \
        "${image}" \
        bootc install to-disk --via-loopback --generic-image --wipe \
            --target-imgref "${target}" \
            --root-ssh-authorized-keys /output/id_ed25519.pub \
            --karg console=tty0 --karg console=ttyS0,115200 \
            --karg systemd.wants=sshd.service \
            /output/disk.raw
    sudo chown "$(id -u):$(id -g)" "${WORK}/disk.raw"
}

# Boots the disk (UEFI, like most PCs and GNOME Boxes), with Secure Boot on
# and Microsoft's keys enrolled, as PCs come from the shop. The UEFI variables
# (boot entries) are kept from one start to the next, as on a real PC.
# Optional settings, as environment variables:
#   VM_MEMORY (MiB, default 4096: a modest PC), VM_CPUS (default: the
#   machine's processors, at most 4; CI runners have 4, so the tests run
#   faster without testing anything less)
#   VM_CDROM: an ISO in the CD drive
#   VM_SERIAL: serial log file name in OUT (default serial.log)
#   VM_EXTRA_ARGS: array of more QEMU arguments
vm_start() {
    local f ovmf_code="" ovmf_vars=""
    # Debian/Ubuntu (CI runners), then Fedora
    for f in /usr/share/OVMF/OVMF_CODE_4M.secboot.fd /usr/share/edk2/ovmf/OVMF_CODE.secboot.fd; do
        if [[ -f "${f}" ]]; then ovmf_code="${f}"; break; fi
    done
    for f in /usr/share/OVMF/OVMF_VARS_4M.ms.fd /usr/share/edk2/ovmf/OVMF_VARS.secboot.fd; do
        if [[ -f "${f}" ]]; then ovmf_vars="${f}"; break; fi
    done
    [[ -n "${ovmf_code}" && -n "${ovmf_vars}" ]] || fail "OVMF Secure Boot firmware not found"
    [[ -f "${WORK}/vars.fd" ]] || cp "${ovmf_vars}" "${WORK}/vars.fd"

    local extra=()
    if [[ -n "${VM_CDROM:-}" ]]; then
        # shellcheck disable=SC2054 # QEMU options are comma-separated
        extra+=(-drive file="${VM_CDROM}",media=cdrom,readonly=on)
    fi
    extra+=(${VM_EXTRA_ARGS[@]+"${VM_EXTRA_ARGS[@]}"})

    log "Booting the virtual machine"
    # smm=on and a secure flash: the firmware keeps the Secure Boot keys
    # out of reach of the system it starts
    qemu-system-x86_64 \
        -machine q35,smm=on,accel=kvm -cpu host -smp "${VM_CPUS:-$(( $(nproc) < 4 ? $(nproc) : 4 ))}" -m "${VM_MEMORY:-4096}" \
        -global driver=cfi.pflash01,property=secure,value=on \
        -global ICH9-LPC.disable_s3=1 \
        -drive if=pflash,format=raw,readonly=on,file="${ovmf_code}" \
        -drive if=pflash,format=raw,file="${WORK}/vars.fd" \
        -drive file="${WORK}/disk.raw",format=raw,if=virtio \
        -netdev user,id=net0,hostfwd=tcp:127.0.0.1:${SSH_PORT}-:22 \
        -device virtio-net-pci,netdev=net0 \
        -device virtio-vga,xres=1920,yres=1080 \
        -display none \
        -serial file:"${OUT}/${VM_SERIAL:-serial.log}" \
        -monitor unix:"${WORK}/monitor.sock",server,nowait \
        -pidfile "${WORK}/qemu.pid" -daemonize \
        "${extra[@]}"
}

# Checks that the system started with Secure Boot on, and that no kernel
# module was refused for its signature. Usage: vm_check_secure_boot <label>
#   Returns 1 if a check failed. NVIDIA image: its driver is signed with a key
#   the person enrolls at the first boot, which the virtual machine does not
#   do, so refused NVIDIA modules are expected there (it has no NVIDIA GPU).
vm_check_secure_boot() {
    local label="$1" level="${CHECK_LEVEL:-error}" state rejected
    state="$(vm 'mokutil --sb-state' 2>&1 || true)"
    echo "${label}: ${state}"
    if ! grep -q 'SecureBoot enabled' <<< "${state}"; then
        echo "::${level}::${label}: Secure Boot is not on (${state})"
        return 1
    fi
    rejected="$(vm 'journalctl -k -b --no-pager -o cat' \
        | grep -iE 'module verification failed|Key was rejected|unsigned module|Lockdown: .*module' || true)"
    [[ -z "${rejected}" ]] && return 0
    echo "${rejected}"
    if vm 'bootc status --json' | jq -e '.status.booted.image.image.image | test("-nvidia[:@]")' >/dev/null; then
        echo "::notice::${label}: kernel modules refused with Secure Boot on (expected for the NVIDIA driver until its key is enrolled)"
        return 0
    fi
    echo "::${level}::${label}: kernel modules were refused with Secure Boot on"
    return 1
}

# True while the virtual machine is running
vm_running() {
    [[ -f "${WORK}/qemu.pid" ]] && kill -0 "$(cat "${WORK}/qemu.pid")" 2>/dev/null
}

vm() {
    ssh -q -i "${WORK}/id_ed25519" -p "${SSH_PORT}" \
        -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
        -o ConnectTimeout=5 -o BatchMode=yes root@127.0.0.1 "$@"
}
monitor() {
    echo "$*" | socat - UNIX-CONNECT:"${WORK}/monitor.sock" >/dev/null
}
screenshot() {
    local name="$1"
    echo "screendump ${OUT}/${name}.png -f png" | socat - UNIX-CONNECT:"${WORK}/monitor.sock" >/dev/null || true
    sleep 2
    if [[ ! -s "${OUT}/${name}.png" ]]; then
        echo "screendump ${OUT}/${name}.ppm" | socat - UNIX-CONNECT:"${WORK}/monitor.sock" >/dev/null || true
    fi
}

# Waits for SSH (a boot can take a few minutes)
# Usage: vm_wait_ssh [name of the screenshot taken on timeout]
vm_wait_ssh() {
    local deadline=$((SECONDS + 600))
    until vm true 2>/dev/null; do
        if (( SECONDS > deadline )); then
            screenshot "${1:-no-ssh}"
            tail -n 50 "${OUT}/${VM_SERIAL:-serial.log}" || true
            fail "The virtual machine did not answer over SSH within 10 minutes"
        fi
        sleep 10
    done
    log "The virtual machine is up"
}

# Reboots and waits for the new boot to answer over SSH
vm_reboot() {
    local old new deadline=$((SECONDS + 600))
    old="$(vm cat /proc/sys/kernel/random/boot_id)"
    log "Rebooting the virtual machine"
    vm 'systemctl reboot' || true
    until new="$(vm cat /proc/sys/kernel/random/boot_id 2>/dev/null)" && [[ "${new}" != "${old}" ]]; do
        if (( SECONDS > deadline )); then
            screenshot "no-reboot"
            tail -n 50 "${OUT}/${VM_SERIAL:-serial.log}" || true
            fail "The virtual machine did not come back within 10 minutes of the reboot"
        fi
        sleep 10
    done
    log "The virtual machine is back"
}

# Digest of the image the system booted
vm_booted_digest() {
    vm 'bootc status --json' | jq -r '.status.booted.image.imageDigest'
}

# Checks that the system started cleanly and the login screen is up.
# Usage: vm_check_system <label>
#   Logs go to OUT with the label in their name. Returns 1 if a check failed;
#   prints the problems as GitHub errors, or as warnings when
#   CHECK_LEVEL=warning.
vm_check_system() {
    local label="$1" level="${CHECK_LEVEL:-error}" errors=0
    local state all_failed failed unit deadline

    state="$(vm 'timeout 300 systemctl is-system-running --wait' || true)"
    vm 'journalctl -b --no-pager' > "${OUT}/journal-${label}.log" || true
    vm 'bootc status' > "${OUT}/bootc-status-${label}.txt" || true
    cat "${OUT}/bootc-status-${label}.txt"

    # Units that fail because of the CI machine, not the image:
    # - mcelog: "AMD Processor family 26: mcelog does not support this processor"
    # - EXTRA_IGNORED_UNITS (e.g. "nvidia-persistenced|nvidia-cdi-refresh"):
    #   units that need hardware the virtual machine lacks, such as an NVIDIA
    #   GPU (nvidia-cdi-refresh.path fails with its service)
    local ignored="^(mcelog${EXTRA_IGNORED_UNITS:+|${EXTRA_IGNORED_UNITS}})\\.(service|path) "
    all_failed="$(vm 'systemctl --failed --no-legend --plain' || true)"
    failed="$(grep -vE "${ignored}" <<< "${all_failed}" | grep -v '^$' || true)"
    if [[ -n "${all_failed}" ]]; then
        echo "Failed units (including the ignored ones):"
        echo "${all_failed}"
    fi
    if [[ "${state}" != running && "${state}" != degraded ]] || [[ -n "${failed}" ]]; then
        echo "::${level}::${label}: system state is '${state}'. Failed units: $(awk '{print $1}' <<< "${failed}" | xargs)"
        for unit in $(awk '{print $1}' <<< "${failed}"); do
            vm "journalctl -b -u ${unit} --no-pager -n 30" || true
        done
        errors=1
    fi

    # NVIDIA image: bootc applied its kernel arguments (usr/lib/bootc/kargs.d)
    if vm 'bootc status --json' | jq -e '.status.booted.image.image.image | test("-nvidia[:@]")' >/dev/null; then
        vm 'grep -q nvidia-drm.modeset=1 /proc/cmdline && grep -q modprobe.blacklist=nouveau /proc/cmdline' \
            || { echo "::${level}::${label}: the NVIDIA kernel arguments are missing"; vm 'cat /proc/cmdline' || true; errors=1; }
    fi

    vm_check_secure_boot "${label}" || errors=1

    vm 'systemctl is-active cosmic-greeter.service' \
        || { echo "::${level}::${label}: cosmic-greeter.service is not active"; errors=1; }

    # The login screen starts through a login shell as its own user: anything
    # /etc/profile.d prints there shows on the screen before it appears
    # (a "brew: Permission denied" did on 2026-11-17). Checked on the text
    # console the login screen started on (what the user sees) and by running
    # that login shell again as the login screen's user.
    local profile_output greeter_output
    profile_output="$(vm 'stat -c "%A %U %C %n" /home/linuxbrew/.linuxbrew /home/linuxbrew/.linuxbrew/bin/brew 2>&1;
        journalctl -b _TRANSPORT=audit --no-pager 2>/dev/null | grep -i "avc.*brew" | tail -n 5;
        echo ---; { tr -s " " < /dev/vcs1; } 2>/dev/null | grep -o "[^ ]*profile\.d[^|]*" | cut -c1-300;
        runuser -u cosmic-greeter -- sh -l -c true 2>&1')" || true
    echo "Homebrew files, and login shell messages of the login screen user:"
    echo "${profile_output}"
    greeter_output="$(sed '1,/^---$/d' <<< "${profile_output}")"
    if [[ -n "${greeter_output}" ]]; then
        echo "::${level}::${label}: the login screen's login shell prints messages: ${greeter_output}"
        errors=1
    fi

    # The login screen runs its own compositor
    deadline=$((SECONDS + 120))
    until vm 'pgrep -x cosmic-comp' >/dev/null; do
        if (( SECONDS > deadline )); then
            echo "::${level}::${label}: the login screen compositor (cosmic-comp) is not running"
            vm 'journalctl -b -u cosmic-greeter.service --no-pager -n 80' || true
            errors=1
            break
        fi
        sleep 5
    done
    vm 'journalctl -b -p warning --no-pager' > "${OUT}/journal-warnings-${label}.log" || true
    return "${errors}"
}

# Usage: vm_check_tuning <label>
#   System tuning that only shows on a booted system (see the "System tuning"
#   checks of build_files/99-tests.sh for what is in the image). Returns 1 if
#   a check failed. Called once per way of installing (bootc install, ISO).
vm_check_tuning() {
    local label="$1" errors=0 unit
    local -a problems=()

    vm 'swapon --show=NAME --noheadings | grep -q zram' \
        || problems+=("no compressed RAM swap (zram)")
    for unit in systemd-oomd.service fstrim.timer tuned.service uupd.timer \
                bootc-update-ca-trust.service bootc-update-crypto-policies.service; do
        vm "systemctl is-active --quiet ${unit}" || problems+=("${unit} is not active")
    done
    # TRIM now rather than waiting a week: it must succeed and go through the
    # mount table, not the "/" line that the installer writes to fstab
    if vm 'systemctl start fstrim.service'; then
        vm 'journalctl -b -u fstrim.service --no-pager -o cat | tail -n 8' || true
    else
        problems+=("fstrim.service failed")
        vm 'journalctl -b -u fstrim.service --no-pager -n 20' || true
    fi
    # Kernel settings of /usr/lib/sysctl.d/60-celenn.conf, as applied
    local setting
    for setting in vm.swappiness=180 vm.page-cluster=0 vm.dirty_bytes=268435456 \
                   vm.dirty_background_bytes=134217728 fs.inotify.max_user_instances=8192; do
        vm "[ \"\$(sysctl -n ${setting%%=*})\" = ${setting#*=} ]" \
            || problems+=("sysctl ${setting%%=*} is not ${setting#*=}")
    done
    vm 'swapon --show; oomctl | head -n 20' || true

    for problem in "${problems[@]}"; do
        echo "::error::${label}: ${problem}"
        errors=1
    done
    return "${errors}"
}

# Desktop screenshots, to check the look without a PC: logs a test user in
# through the login screen, then opens a few views and apps. Only a login that
# does not give a COSMIC session is a failure (desktop_tour returns 1); a
# missing screenshot or app is a warning: it doesn't keep a working image from
# shipping.
# Files are numbered in the order they are taken (tests/screenshot-sheet.py).
DEMO_USER=demo
DEMO_PASSWORD=celennos  # letters only: typed as US QWERTY key codes below
SHOTS=screenshots

type_text() {
    local i
    for (( i = 0; i < ${#1}; i++ )); do
        monitor "sendkey ${1:i:1}"
        sleep 0.15
    done
}
wait_for() {
    local seconds="$1"; shift
    local deadline=$((SECONDS + seconds))
    until vm "$@" >/dev/null 2>&1; do
        (( SECONDS > deadline )) && return 1
        sleep 3
    done
}
warn() { echo "::warning::Desktop screenshots: $*"; }
# A problem the boot test fails on but the update test only warns about (its
# account was made by the previous image): what a new account gets at its
# first login (COSMIC's first-run assistant, Celenn OS's tour) and the
# Firefox policies. Kept in DESKTOP_PROBLEMS for the boot test.
DESKTOP_PROBLEMS=()
desktop_problem() { warn "$*"; DESKTOP_PROBLEMS+=("$*"); }

# Creates the test user if needed and shows it on the login screen (the
# login screen reads the accounts when it starts)
demo_user() {
    if ! vm "id ${DEMO_USER}" >/dev/null 2>&1; then
        vm "useradd -m -c Celenn ${DEMO_USER} && echo ${DEMO_USER}:${DEMO_PASSWORD} | chpasswd" \
            || { warn "could not create the test user"; return 1; }
    fi
    # A fixed computer name, shown in the terminal screenshot: without it,
    # the runner's network can name the VM after its address
    # (ip-10-0-2-15), which changes from one run to the next
    vm 'hostnamectl hostname fedora' || warn "could not set the computer name"
    vm 'systemctl restart cosmic-greeter.service' \
        || { warn "could not restart the login screen"; return 1; }
    wait_for 120 'pgrep -x cosmic-greeter' || { warn "the login screen did not come back"; return 1; }
    sleep 20
}

# Logs the test user in with the keyboard, as a person would, and sets
# DEMO_ENV to the environment of its session. The password is typed a second
# time if the first try gives no session (a key lost by the virtual keyboard
# is not a broken image).
# Usage: demo_login <name of the screenshot taken if it fails>
demo_login() {
    mkdir -p "${OUT}/${SHOTS}"
    local try
    for try in 1 2; do
        if (( try == 2 )); then
            screenshot "${SHOTS}/$1"
            warn "no COSMIC session after the first try, typing the password again"
            monitor "sendkey ctrl-a"
            monitor "sendkey backspace"
            sleep 1
        fi
        type_text "${DEMO_PASSWORD}"
        monitor "sendkey ret"
        wait_for 120 "pgrep -u ${DEMO_USER} -x cosmic-panel" && break
        if (( try == 2 )); then
            screenshot "${SHOTS}/$1"
            warn "the test user did not get a COSMIC session"
            vm 'journalctl -b -u cosmic-greeter.service --no-pager -n 60' || true
            return 1
        fi
    done
    local uid run_dir wayland
    uid="$(vm "id -u ${DEMO_USER}")"
    run_dir="/run/user/${uid}"
    wayland="$(vm "ls ${run_dir} | grep -E '^wayland-[0-9]+\$' | head -n 1")"
    [[ -n "${wayland}" ]] || { warn "no Wayland socket in ${run_dir}"; return 1; }
    DEMO_ENV="XDG_RUNTIME_DIR=${run_dir} WAYLAND_DISPLAY=${wayland} DBUS_SESSION_BUS_ADDRESS=unix:path=${run_dir}/bus"
}

# The whole tour: login screen, first login, desktop, App Library, a few apps
# and the lock screen
desktop_tour() {
    mkdir -p "${OUT}/${SHOTS}"
    demo_user || return 1
    # New accounts get the first steps tour (not the account kept from the
    # previous image in the update test)
    local first_steps=false
    vm "test -e ~${DEMO_USER}/.config/celenn/first-steps-pending" && first_steps=true
    screenshot "${SHOTS}/01-login-screen"
    demo_login 02-login-failed || return 1

    # First login: COSMIC's first-run assistant (cosmic-initial-setup) must
    # not open (new accounts skip it, build_files/20-cosmic.sh); our tour
    # opens instead. (The update test's account comes from the stable
    # image, see vm-update-test.sh.)
    sleep 30
    if vm "pgrep -u ${DEMO_USER} -f cosmic-initial-setup" >/dev/null; then
        screenshot "${SHOTS}/02-first-run"
        desktop_problem "COSMIC's first-run assistant opened at the first login"
        vm "pkill -u ${DEMO_USER} -f cosmic-initial-setup" || true
        sleep 8
    fi
    if [[ "${first_steps}" == true ]]; then
        first_steps_shots || true
    fi
    # Backups (Déjà Dup): its package starts the monitor that runs the
    # automatic backups at login (a Flatpak could not, see 30-packages.sh)
    wait_for 30 "pgrep -u ${DEMO_USER} -f deja-dup-monitor" \
        || desktop_problem "the Déjà Dup backup monitor did not start at login"

    screenshot "${SHOTS}/03-desktop"

    # App Library (Super+A): the application icons
    monitor "sendkey meta_l-a"
    sleep 5
    screenshot "${SHOTS}/04-app-library"
    monitor "sendkey esc"
    sleep 3

    # Files (folder icons), Settings (accent, theme), COSMIC Store (our
    # rebuild, which no other step starts: a crash fails vm_check_crashes),
    # Celenn OS Tools (its home, tools, a failure card, the developer tools list)
    # and Keyboard shortcuts (Celenn OS's own apps; images before them skip
    # them). A command may start with VARIABLE=value settings.
    local entry name cmd program settings
    for entry in "05-files|cosmic-files" "06-settings|cosmic-settings appearance" "06-store|cosmic-store" \
        "06-tools-overview|/usr/libexec/celenn-tools" \
        "06-updates|/usr/libexec/celenn-updates" \
        "06-tools-space|/usr/libexec/celenn-tools --tool space" \
        "06-tools-inside|/usr/libexec/celenn-tools --tool inside" \
        "06-tools-software|/usr/libexec/celenn-tools --tool apps" \
        "06-tools-look|/usr/libexec/celenn-tools --tool look" \
        "06-tools-repair|/usr/libexec/celenn-tools --tool repair" \
        "06-tools-advanced|/usr/libexec/celenn-tools --tool advanced" \
        "06-tools-help|/usr/libexec/celenn-tools --tool help" \
        "06-tools-failure|CELENN_TOOLS_PREVIEW=failure /usr/libexec/celenn-tools --tool space" \
        "06-tools-developer|CELENN_TOOLS_PREVIEW=developer-tools /usr/libexec/celenn-tools --tool apps" \
        "06-tools-disks|/usr/libexec/celenn-tools --tool disks" \
        "06-tools-web-apps|/usr/libexec/celenn-tools --tool web-apps" \
        "06-quick-menu|/usr/libexec/celenn-tools --quick" \
        "06-shortcuts|/usr/libexec/celenn-tour --shortcuts"; do
        name="${entry%%|*}"
        cmd="${entry#*|}"
        settings=""
        while [[ "${cmd%% *}" == *=* ]]; do
            settings+=" ${cmd%% *}"
            cmd="${cmd#* }"
        done
        program="${cmd%% *}"
        if [[ "${program}" == /* ]] && ! vm "test -x ${program}"; then
            continue
        fi
        if ! vm "runuser -u ${DEMO_USER} -- env ${DEMO_ENV}${settings} \
                setsid -f ${cmd} >/dev/null 2>&1 </dev/null"; then
            warn "could not start ${cmd}"
            continue
        fi
        sleep 12
        # (Web apps: its sites' icons come from the internet)
        [[ "${name}" == 06-tools-web-apps ]] && sleep 18
        screenshot "${SHOTS}/${name}"
        vm "pkill -u ${DEMO_USER} -f '^(/usr/bin/)?${program}'" || true
        sleep 3
    done

    # Updates once more with the light theme: its symbols and accent must
    # read on both (COSMIC apps follow the setting as it changes)
    local mode_dir="~${DEMO_USER}/.config/cosmic/com.system76.CosmicTheme.Mode/v1"
    if vm "test -x /usr/libexec/celenn-updates" \
        && vm "runuser -u ${DEMO_USER} -- sh -c 'mkdir -p ${mode_dir} && printf false > ${mode_dir}/is_dark'"; then
        sleep 5
        if vm "runuser -u ${DEMO_USER} -- env ${DEMO_ENV} \
                setsid -f /usr/libexec/celenn-updates >/dev/null 2>&1 </dev/null"; then
            sleep 12
            screenshot "${SHOTS}/06-updates-light"
            vm "pkill -u ${DEMO_USER} -f '^/usr/libexec/celenn-updates'" || true
            sleep 3
        fi
        vm "runuser -u ${DEMO_USER} -- sh -c 'printf true > ${mode_dir}/is_dark'" \
            || warn "could not switch back to the dark theme"
        sleep 5
    fi

    # Updates while a new version downloads: its progress bar, from a
    # progress file like the one bootc's download writes (update_progress.rs)
    local progress=/run/celenn/update-progress.jsonl
    if vm "test -x /usr/libexec/celenn-updates" \
        && vm "mkdir -p /run/celenn && printf '%s\\n' \
            '{\"type\":\"ProgressBytes\",\"task\":\"pulling\",\"bytes\":1288490188,\"bytesTotal\":2147483648}' \
            > ${progress}"; then
        if vm "runuser -u ${DEMO_USER} -- env ${DEMO_ENV} \
                setsid -f /usr/libexec/celenn-updates >/dev/null 2>&1 </dev/null"; then
            sleep 12
            screenshot "${SHOTS}/06-updates-progress"
            vm "pkill -u ${DEMO_USER} -f '^/usr/libexec/celenn-updates'" || true
            sleep 3
        fi
        vm "rm -f ${progress}" || true
    fi

    # Firefox, a first-boot flatpak in the dock, with the Celenn OS
    # policies it reads from its systemconfig extension (mounted at
    # /app/etc/firefox in its sandbox, see celenn-firefox.conf)
    firefox_shot

    # Terminal: prompt and fastfetch
    terminal_shot

    # Lock screen
    if vm 'loginctl lock-sessions'; then
        sleep 8
        screenshot "${SHOTS}/08-lock-screen"
    else
        warn "could not lock the session"
    fi
}

# Usage: vm_check_crashes <label>
#   Programs that crashed since the system started (systemd-coredump). A crash
#   of a COSMIC or Celenn OS program is an error (returns 1), any other one
#   a warning. Run after desktop_tour, which starts the session and some apps.
vm_check_crashes() {
    local label="$1" errors=0 crashes line name
    # One "Process <pid> (<name>) of user <uid> dumped core." line per crash,
    # followed by its stack trace
    crashes="$(vm 'journalctl -b --no-pager -o cat MESSAGE_ID=fc2e22bc6ee647b6b90729ab34a250b1' || true)"
    while IFS= read -r line; do
        [[ "${line}" =~ ^Process\ [0-9]+\ \((.*)\)\ of\ user ]] || continue
        name="${BASH_REMATCH[1]}"
        # (names are cut to 15 characters: celenn-tou)
        if [[ "${name}" == cosmic* ]]; then
            echo "::error::${label}: ${name} crashed (${line})"
            errors=1
        else
            echo "::warning::${label}: ${name} crashed (${line})"
        fi
    done <<< "${crashes}"
    [[ -z "${crashes}" ]] || printf '%s\n' "${crashes}" > "${OUT}/crashes-${label}.log"
    return "${errors}"
}

# Celenn OS's first steps tour, which opens at the first login of a new
# account: one screenshot per page (Alt+Right turns the page), then closed.
# Nobody picks where they come from, so the pages are those of the general path.
FIRST_STEPS_PAGES=5

first_steps_shots() {
    local page
    if ! wait_for 60 "pgrep -u ${DEMO_USER} -f celenn-tour\$"; then
        desktop_problem "the first steps tour did not open at the first login"
        return 1
    fi
    sleep 8
    for (( page = 1; page <= FIRST_STEPS_PAGES; page++ )); do
        screenshot "${SHOTS}/02-first-steps-${page}"
        monitor "sendkey alt-right"
        sleep 3
    done
    vm "pkill -u ${DEMO_USER} -f celenn-tour\$" || true
    sleep 3
    vm "test ! -e ~${DEMO_USER}/.config/celenn/first-steps-pending" \
        || desktop_problem "the first steps tour would open again at the next login"
}

# The terminal as the test user sees it: the Celenn OS prompt (Starship) and
# fastfetch. Text checks first, as the user's login shell runs them,
# then COSMIC Terminal opened in a small git project, so the prompt shows its
# path and git segments, with `fastfetch` typed in it.
# The screenshot sheet adds a full-size crop of the window
# (tests/screenshot-sheet.py).
TERMINAL_PROJECT=projects/hello

terminal_checks() {
    local result line
    result="$(vm "runuser -u ${DEMO_USER} -- bash -l -s -- ${TERMINAL_PROJECT}" \
        < "${TESTS_DIR}/vm-terminal-checks.sh" 2>&1)" \
        || result="FAIL could not run the checks as ${DEMO_USER}: ${result}"
    # Terminals opened from the desktop inherit the session's environment:
    # the welcome banner's guard must not be in it, or no terminal greets
    if vm "tr '\\0' '\\n' < /proc/\$(pgrep -u ${DEMO_USER} -x cosmic-panel | head -n 1)/environ" \
            | grep -q '^UWELCOME_SHOWN='; then
        result+=$'\nFAIL the desktop session exports UWELCOME_SHOWN, so terminals show no welcome banner'
    fi
    while IFS= read -r line; do
        [[ "${line}" == "FAIL "* ]] && warn "terminal: ${line#FAIL }"
    done <<< "${result}"
    [[ "${result}" != *"FAIL "* ]]
}

terminal_shot() {
    terminal_checks || true
    if ! vm "runuser -u ${DEMO_USER} -- env ${DEMO_ENV} \
            sh -c 'cd ~${DEMO_USER}/${TERMINAL_PROJECT} || cd; exec setsid -f cosmic-term' \
            >/dev/null 2>&1 </dev/null"; then
        warn "could not start cosmic-term"
        return 1
    fi
    sleep 12
    type_text fastfetch
    monitor "sendkey ret"
    sleep 6
    screenshot "${SHOTS}/07-terminal"
    vm "pkill -u ${DEMO_USER} -f '^(/usr/bin/)?cosmic-term'" || true
    sleep 3
}

FIREFOX=org.mozilla.firefox
firefox_shot() {
    local policies
    policies="$(vm "runuser -u ${DEMO_USER} -- env ${DEMO_ENV} flatpak run --command=cat ${FIREFOX} \
        /app/etc/firefox/policies/policies.json" 2>&1)" || policies=""
    jq -e '.policies.DisableTelemetry' <<<"${policies}" >/dev/null 2>&1 \
        || desktop_problem "Firefox does not see the Celenn OS policies: $(head -c 300 <<<"${policies}")"
    if ! vm "runuser -u ${DEMO_USER} -- env ${DEMO_ENV} \
            setsid -f flatpak run ${FIREFOX} >/dev/null 2>&1 </dev/null"; then
        warn "could not start Firefox"
        return 1
    fi
    sleep 20
    screenshot "${SHOTS}/06-web-browser"
    vm "runuser -u ${DEMO_USER} -- env ${DEMO_ENV} flatpak kill ${FIREFOX}" || true
    sleep 3
}

# Saves the journal of the test user's session
demo_journal() {
    vm "journalctl -b _UID=\$(id -u ${DEMO_USER}) --no-pager" > "${OUT}/journal-${DEMO_USER}${1:+-$1}.log" 2>/dev/null || true
}

# Parental controls, after the desktop tour (the test user's session is
# open, locked by the tour's last step): a child account created with the
# app's helper (rules, service, web filter), screenshots of the app, then the
# test user itself as a child: the warning before its time is up, the more
# time window, more time given, its session closed when the time is up, and
# the login refused. Returns 1 when a rule is not applied; a missing
# screenshot or an unreachable family filter is a warning.
# Other disks (Tools, Other disks): a second, empty virtual disk (vdb, added by
# vm-boot-test.sh) gets an ext4 partition labeled "Mes Jeux" before the
# desktop tour (Tools then lists it), then the helper opens it at startup and
# stops doing so, as Apply does. Returns 1 when something is wrong.
DATA_DISK_LABEL="Mes Jeux"
disks_prepare() {
    vm "printf 'label: gpt\n,,0FC63DAF-8483-4772-8E79-3D69D8477DE4\n' | sfdisk -q /dev/vdb \
        && udevadm settle && mkfs.ext4 -q -L '${DATA_DISK_LABEL}' /dev/vdb1 && udevadm settle" \
        || { echo "::error::Other disks: could not prepare the test disk"; return 1; }
}
disks_checks() {
    local failed=0 uuid uid mp=/var/mnt/Mes-Jeux output system
    problem() { echo "::error::Other disks: $*"; failed=1; }
    uuid="$(vm 'blkid -s UUID -o value /dev/vdb1')" || { problem "no test disk"; return 1; }
    uid="$(vm "id -u ${DEMO_USER}")"
    output="$(vm "PKEXEC_UID=${uid} /usr/libexec/celenn-tools-helper disks ${uuid}=on" 2>&1)" \
        || problem "the helper did not open the disk: ${output}"
    vm "findmnt -no SOURCE ${mp}" | grep -q '^/dev/vdb1$' || problem "the disk is not open at ${mp}"
    vm "grep -qE '^UUID=${uuid} ${mp} ext4 [^ ]*nofail[^ ]*x-celenn-disk 0 2\$' /etc/fstab" \
        || problem "no fstab line for it: $(vm 'cat /etc/fstab')"
    [[ "$(vm "stat -c %u ${mp}")" == "${uid}" ]] || problem "the new, empty disk is not the user's"
    vm "runuser -u ${DEMO_USER} -- touch /mnt/Mes-Jeux/steam-library-test" \
        || problem "the user cannot write to it at /mnt/Mes-Jeux"
    vm "grep -q '^filesystems=.*${mp};' /var/lib/flatpak/overrides/com.valvesoftware.Steam" \
        || problem "Steam (Flatpak) was not given ${mp}"
    # Again: nothing changes, no second line
    vm "PKEXEC_UID=${uid} /usr/libexec/celenn-tools-helper disks ${uuid}=on" >/dev/null 2>&1 \
        || problem "turning it on twice failed"
    [[ "$(vm "grep -c 'UUID=${uuid}' /etc/fstab")" == 1 ]] || problem "two fstab lines for it"
    # Off: closed, line and folder gone, its files kept
    output="$(vm "/usr/libexec/celenn-tools-helper disks ${uuid}=off" 2>&1)" \
        || problem "the helper did not close the disk: ${output}"
    vm "mountpoint -q ${mp}" && problem "still open after off"
    vm "grep -q 'UUID=${uuid}' /etc/fstab" && problem "its fstab line is still there"
    vm "test -e ${mp}" && problem "its folder is still there"
    vm "grep -q '${mp}' /var/lib/flatpak/overrides/com.valvesoftware.Steam" \
        && problem "Steam (Flatpak) still has ${mp}"
    vm 'd="$(mktemp -d)" && mount /dev/vdb1 "$d" && { test -e "$d/steam-library-test"; s=$?; umount "$d"; rmdir "$d"; exit "$s"; }' \
        || problem "its files were not kept"
    # The system's own partition is refused (in use, or in fstab)
    system="$(vm 'findmnt -nvo UUID /sysroot')"
    output="$(vm "/usr/libexec/celenn-tools-helper disks ${system}=on" 2>&1)" \
        && problem "the system partition was accepted"
    [[ "${output}" == "${system} system" || "${output}" == "${system} in-fstab" ]] \
        || problem "the system partition: ${output}"
    return "${failed}"
}

PARENTAL_CHILD=lea
parental_checks() {
    local parental_failed=0 rules state day json
    problem() { echo "::error::Parental controls: $*"; parental_failed=1; }
    rules='{"limit":{"enabled":true,"weekdays":120,"weekend":180},"bedtime":{"enabled":true,"start":1230,"end":420},"web_filter":true,"apps":{"enabled":true,"allowed":["org.mozilla.firefox","org.mozilla.thunderbird"]}}'
    vm "printf '%s\n%s\n' ${DEMO_PASSWORD} '${rules}' \
            | /usr/libexec/celenn-parental-helper add-child ${PARENTAL_CHILD} 'Léa' /usr/share/pixmaps/faces/rocket.png" \
        || { problem "celenn-parental-helper could not create a child account"; return 1; }
    vm "id -nG ${PARENTAL_CHILD} | grep -qw wheel" && problem "the child account is an administrator"
    # Its avatar, one of the system's own, is copied where the login screen
    # reads it (celenn-login-pictures.path)
    wait_for 15 "cmp -s /usr/share/pixmaps/faces/rocket.png /var/lib/AccountsService/icons/${PARENTAL_CHILD}" \
        || problem "the child's avatar is not where the login screen reads it"
    wait_for 30 'systemctl is-active celenn-parental-controls.service' \
        || problem "celenn-parental-controls.service did not start"
    # A week of screen time for the chart
    json='{"today":{"date":"'"$(vm 'date +%F')"'","weekday":'"$(vm 'date +%u')"',"seconds":4800},"history":['
    for day in 6 5 4 3 2 1; do
        json+='{"date":"'"$(vm "date -d -${day}day +%F")"'","weekday":'"$(vm "date -d -${day}day +%u")"',"seconds":'$(( (day * 1931) % 7200 + 1500 ))'}'
        (( day > 1 )) && json+=','
    done
    json+=']}'
    vm "echo '${json}' > /var/lib/celenn/parental-controls/${PARENTAL_CHILD}.json"

    # Web filter: the child's DNS queries go to the family filter
    wait_for 30 'nft list table inet celenn_parental' \
        || problem "no nftables rules for the web filter"
    wait_for 30 'systemctl is-active celenn-family-dns.service' \
        || problem "celenn-family-dns.service did not start"
    # The filter answers this name itself (Firefox's DNS-over-HTTPS canary):
    # "no such name" for the child shows that its queries reach the filter,
    # without needing the family DNS servers on the internet
    local child_answer root_answer canary
    canary="$(vm "runuser -u ${PARENTAL_CHILD} -- python3 -c 'import socket
try:
    socket.getaddrinfo(\"use-application-dns.net\", 443)
    print(\"resolved\")
except socket.gaierror as error:
    print(\"noname\" if error.errno == socket.EAI_NONAME else error)'" 2>&1)" || true
    echo "DNS-over-HTTPS canary for the child: ${canary}"
    [[ "${canary}" == noname ]] \
        || problem "the child's DNS queries do not reach the family filter (${canary})"
    child_answer="$(vm "runuser -u ${PARENTAL_CHILD} -- getent ahostsv4 example.com" 2>&1 | head -n 1)" || true
    if [[ -z "${child_answer}" ]]; then
        echo "::warning::Parental controls: the child account resolves nothing (family filter unreachable?)"
    else
        child_answer="$(vm "runuser -u ${PARENTAL_CHILD} -- getent ahostsv4 pornhub.com" 2>&1 | head -n 1)" || true
        root_answer="$(vm 'getent ahostsv4 pornhub.com' 2>&1 | head -n 1)" || true
        echo "Adult site for the child: ${child_answer:-none}, for root: ${root_answer:-none}"
        [[ "${child_answer}" != "${root_answer}" ]] \
            || problem "the web filter does not change what the child account resolves"
        echo "Google for the child: $(vm "runuser -u ${PARENTAL_CHILD} -- getent ahostsv4 www.google.com" 2>&1 | head -n 1)"
    fi

    # Applications validated by a parent: a program the child downloaded or
    # copied is stopped at once
    vm "install -o ${PARENTAL_CHILD} -m 0755 /usr/bin/sleep /home/${PARENTAL_CHILD}/game \
        && runuser -u ${PARENTAL_CHILD} -- setsid -f /home/${PARENTAL_CHILD}/game 300 >/dev/null 2>&1 </dev/null" \
        || problem "could not start a program as the child"
    sleep 6
    if vm "pgrep -u ${PARENTAL_CHILD} -x game" >/dev/null; then
        problem "a downloaded program still runs on the child's account"
        vm "pkill -u ${PARENTAL_CHILD} -x game" || true
    fi
    vm "journalctl -b --no-pager -u celenn-parental-controls.service -g 'stopped .*home/${PARENTAL_CHILD}/game'" >/dev/null \
        || echo "::warning::Parental controls: the service did not log the downloaded program it stopped"

    # What would go around the filter is refused to the child: DNS to another
    # server, DNS over HTTPS to a public resolver, a VPN port, and
    # systemd-resolved's D-Bus lookups. A refusal answers at once (nftables
    # reject), unlike a network that is down.
    local probe target host port
    for target in "8.8.8.8 53 DNS to another server" "1.1.1.1 443 DNS over HTTPS" "9.9.9.9 1194 OpenVPN"; do
        read -r host port _ <<<"${target}"
        probe="$(vm "runuser -u ${PARENTAL_CHILD} -- python3 -c 'import socket
s = socket.socket()
s.settimeout(10)
try:
    s.connect((\"${host}\", ${port}))
    print(\"open\")
except ConnectionRefusedError:
    print(\"refused\")
except OSError as error:
    print(error)'" 2>&1)" || true
        echo "${target#* * } for the child: ${probe}"
        [[ "${probe}" == refused ]] || problem "${target#* * } is not refused to the child (${probe})"
    done
    vm "id -nG ${PARENTAL_CHILD}" | grep -qw celenn-children \
        || problem "the child account is not in the celenn-children group"
    probe="$(vm "runuser -u ${PARENTAL_CHILD} -- busctl call org.freedesktop.resolve1 /org/freedesktop/resolve1 \
        org.freedesktop.resolve1.Manager ResolveHostname isit 0 example.com 0 0" 2>&1)" && probe="answered: ${probe}"
    echo "systemd-resolved over D-Bus for the child: ${probe}"
    grep -qi 'access denied' <<<"${probe}" \
        || problem "systemd-resolved answers the child over D-Bus (${probe})"
    if vm "journalctl -b --no-pager -u polkit -g 'celenn-parental'" | grep -qi error; then
        problem "polkit could not load 10-celenn-parental.rules"
    fi

    # Unlock the test user's session (locked by the tour)
    vm 'loginctl unlock-sessions' || true
    sleep 5
    if vm "loginctl show-user ${DEMO_USER} -p Sessions --value | xargs -n1 loginctl show-session -p LockedHint --value | grep -qx yes"; then
        type_text "${DEMO_PASSWORD}"
        monitor "sendkey ret"
        sleep 8
    fi

    # The app: home, and the child's page
    local entry
    for entry in "09-parental-controls|" "09-parental-controls-child|--user ${PARENTAL_CHILD}"; do
        if vm "runuser -u ${DEMO_USER} -- env ${DEMO_ENV} \
                setsid -f /usr/libexec/celenn-parental-controls ${entry#*|} >/dev/null 2>&1 </dev/null"; then
            sleep 12
            screenshot "${SHOTS}/${entry%%|*}"
            vm "pkill -u ${DEMO_USER} -f '^/usr/libexec/celenn-parental-controls( --user|$)'" || true
            sleep 3
        else
            warn "could not open Parental controls"
        fi
    done

    # The test user becomes a child with 10 minutes a day, 4 min 40 s left:
    # the agent of its session warns
    vm "echo '{\"limit\":{\"enabled\":true,\"weekdays\":10,\"weekend\":10},\"bedtime\":{\"enabled\":false,\"start\":0,\"end\":0},\"web_filter\":false}' \
            | /usr/libexec/celenn-parental-helper set ${DEMO_USER}" \
        || problem "celenn-parental-helper set failed"
    set_used() {
        vm "echo '{\"today\":{\"date\":\"'\$(date +%F)'\",\"weekday\":'\$(date +%u)',\"seconds\":$1}}' \
            > /var/lib/celenn/parental-controls/${DEMO_USER}.json"
    }
    set_used 320
    vm "pgrep -u ${DEMO_USER} -f -- 'celenn-parental-controls --agent'" >/dev/null \
        || problem "the warnings agent does not run in the session"
    sleep 15
    screenshot "${SHOTS}/09-parental-warning"
    state="$(vm '/usr/libexec/celenn-parental-daemon status' 2>&1)" || true
    echo "${state}"
    grep -q "^${DEMO_USER}: .*blocked no" <<<"${state}" || problem "the test user is blocked too early"

    # The more time window, and more time given (as a parent would)
    if vm "runuser -u ${DEMO_USER} -- env ${DEMO_ENV} \
            setsid -f /usr/libexec/celenn-parental-controls --more-time >/dev/null 2>&1 </dev/null"; then
        sleep 10
        screenshot "${SHOTS}/09-parental-more-time"
        vm "pkill -u ${DEMO_USER} -f -- 'celenn-parental-controls --more-time'" || true
    fi
    vm "/usr/libexec/celenn-parental-extend ${DEMO_USER} 15" || problem "celenn-parental-extend failed"
    state="$(vm '/usr/libexec/celenn-parental-daemon status' 2>&1)" || true
    grep -q "^${DEMO_USER}: .*allowed 1500s" <<<"${state}" || problem "more time was not added: ${state}"
    vm "/usr/libexec/celenn-parental-extend ${DEMO_USER} 600" 2>/dev/null \
        && problem "celenn-parental-extend accepted 600 minutes"

    # Six more accounts, so that the login screen shows the most avatars it
    # can (8, two rows of 4); removed once the login has been refused
    local extra extra_users=(ana ben chloe david emma felix)
    for extra in "${extra_users[@]}"; do
        vm "useradd -m -c '${extra^}' ${extra}" || problem "could not add the account ${extra}"
    done
    # Time up: the service closes the session, the login screen refuses it
    set_used 1600
    if wait_for 60 "! pgrep -u ${DEMO_USER} -x cosmic-panel"; then
        log "Parental controls closed the session when its time was up"
    else
        problem "the session was not closed when its time was up"
        vm 'loginctl list-sessions; /usr/libexec/celenn-parental-daemon status' || true
    fi
    sleep 20
    # Eight accounts now (the test user, the child and the six above): the
    # login screen shows them as two rows of avatars, and the refusal below in
    # a light box (build_files/greeter/user-switch.patch)
    screenshot "${SHOTS}/09-login-users"
    type_text "${DEMO_PASSWORD}"
    monitor "sendkey ret"
    sleep 10
    screenshot "${SHOTS}/09-parental-refused"
    sleep 20
    vm "pgrep -u ${DEMO_USER} -x cosmic-panel" >/dev/null \
        && problem "the login screen let the test user in after its time was up"
    vm "env -i PAM_USER=${DEMO_USER} /usr/libexec/celenn-parental-daemon login-check" \
        && problem "the login check lets the test user in after its time was up"
    if ! vm "journalctl -b --no-pager -g 'pam_celenn_parental\\(cosmic-greeter:account\\): login refused for ${DEMO_USER}'" >/dev/null; then
        problem "the login screen did not run the login check"
        vm "grep -rs . /etc/greetd/ | grep -v '^#'; ls /etc/pam.d /usr/lib/pam.d | grep -i greet; \
            journalctl -b --no-pager -g 'pam_celenn_parental|greetd' | tail -n 10; \
            ausearch -m avc -ts boot 2>/dev/null | grep -i parental | tail -n 10" || true
    fi
    vm "journalctl -b --no-pager -u celenn-parental-controls.service" | tail -n 20 || true
    vm "journalctl -b --no-pager -g pam_celenn_parental" | tail -n 5 || true
    vm "rm -f /etc/celenn/parental-controls/${DEMO_USER}.json"
    for extra in "${extra_users[@]}"; do
        vm "userdel -r ${extra}" || true
    done
    return "${parental_failed}"
}
