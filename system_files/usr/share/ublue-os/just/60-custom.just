# vim: set ft=make :
# cosmic-blue commands, listed by `ujust` under "Desktop", "Apps" and "System".
# Names are verb-object: install-<thing> and remove-<thing>, toggle-<thing>
# (on/off), reset-, export-, switch-, report-, enroll-, and for Bluefin's
# (build_files/10-system-files.sh) check-, show-, run-, reboot-, clean-.
#
# COSMIC reads each setting from ~/.config/cosmic first and falls back to the
# image defaults in /usr/share/cosmic. Resetting a setting therefore means
# removing the user's copy, so later image updates of the defaults apply again.
#
# The reset recipes ask first; `yes` as their argument skips the question
# (e.g. `ujust reset-cosmic-theme yes` in a script).

# Switch between the dark and light themes
[group('Desktop')]
toggle-dark-mode mode="toggle":
    #!/usr/bin/bash
    set -euo pipefail
    key="com.system76.CosmicTheme.Mode/v1"
    user_dir="${XDG_CONFIG_HOME:-${HOME}/.config}/cosmic/${key}"
    read_setting() { # name default
        cat "${user_dir}/$1" 2>/dev/null || cat "/usr/share/cosmic/${key}/$1" 2>/dev/null || echo "$2"
    }
    write_setting() { # name value
        install -d "${user_dir}"
        printf '%s' "$2" >"${user_dir}/$1.tmp"
        mv -f "${user_dir}/$1.tmp" "${user_dir}/$1"
    }
    case "{{ mode }}" in
        dark) is_dark=true ;;
        light) is_dark=false ;;
        toggle)
            if [[ "$(read_setting is_dark true)" == true ]]; then is_dark=false; else is_dark=true; fi
            ;;
        *)
            echo "Usage: ujust toggle-dark-mode [toggle|dark|light]" >&2
            exit 1
            ;;
    esac
    # Automatic switching at sunrise and sunset would undo the choice
    if [[ "$(read_setting auto_switch false)" == true ]]; then
        write_setting auto_switch false
        echo "Automatic switching between light and dark turned off."
    fi
    write_setting is_dark "${is_dark}"
    if [[ "${is_dark}" == true ]]; then echo "Dark theme on."; else echo "Light theme on."; fi

# Bring back the cosmic-blue look
[group('Desktop')]
reset-cosmic-theme confirm="ask":
    #!/usr/bin/bash
    set -euo pipefail
    config="${XDG_CONFIG_HOME:-${HOME}/.config}/cosmic"
    # Accent, frosted glass and colors; dark/light mode; icon theme and fonts;
    # wallpaper
    settings=(
        com.system76.CosmicTheme.Dark
        com.system76.CosmicTheme.Dark.Builder
        com.system76.CosmicTheme.Light
        com.system76.CosmicTheme.Light.Builder
        com.system76.CosmicTheme.Mode
        com.system76.CosmicTk
        com.system76.CosmicBackground
    )
    echo "This restores the cosmic-blue colors, dark theme, icons, fonts and wallpaper."
    echo "Other COSMIC settings (panel, dock, keyboard, shortcuts...) are kept."
    if [[ "{{ confirm }}" != yes ]] && ! gum confirm "Reset the appearance?"; then
        exit 0
    fi
    # Each folder is moved aside before it is removed: the running session
    # can write a setting into it meanwhile, which makes a plain `rm -rf`
    # fail with "Directory not empty"
    trash="$(mktemp -d "${config:?}/.reset-cosmic-theme.XXXXXX")"
    for setting in "${settings[@]}"; do
        if [[ -e "${config}/${setting}" ]]; then
            mv "${config}/${setting}" "${trash}/"
        fi
    done
    rm -rf "${trash}"
    echo "Done. Log out and back in to see every change."

# Reset all COSMIC settings (a backup is kept)
[group('Desktop')]
reset-cosmic-config confirm="ask":
    #!/usr/bin/bash
    set -euo pipefail
    config="${XDG_CONFIG_HOME:-${HOME}/.config}/cosmic"
    if [[ ! -d "${config}" ]]; then
        echo "No COSMIC settings to reset."
        exit 0
    fi
    backup="${config}.backup-$(date +%Y%m%d-%H%M%S)"
    echo "This resets every COSMIC setting: appearance, panel, dock, shortcuts, applets..."
    echo "Your keyboard layout and clock format are kept."
    echo "Current settings will be moved to ${backup}"
    if [[ "{{ confirm }}" != yes ]] && ! gum confirm "Reset all COSMIC settings?"; then
        exit 0
    fi
    mv "${config}" "${backup}"
    # Keyboard layout and clock format, set from the installer choices
    keep=(
        com.system76.CosmicComp/v1/xkb_config
        com.system76.CosmicAppletTime/v1/military_time
        com.system76.CosmicAppletTime/v1/first_day_of_week
    )
    for setting in "${keep[@]}"; do
        if [[ -f "${backup}/${setting}" ]]; then
            install -D -m 0644 "${backup}/${setting}" "${config}/${setting}"
        fi
    done
    echo "Done. Log out and back in to finish the reset."
    echo "To undo it: rm -rf '${config}' && mv '${backup}' '${config}', then log out."

# Save your COSMIC settings to a text file
[group('Desktop')]
export-cosmic-config:
    #!/usr/bin/bash
    set -euo pipefail
    config="${XDG_CONFIG_HOME:-${HOME}/.config}/cosmic"
    if [[ ! -d "${config}" ]]; then
        echo "No COSMIC settings to export: every setting is still the default."
        exit 0
    fi
    out="${HOME}/cosmic-config-$(date +%Y%m%d-%H%M%S).txt"
    # Left out on purpose:
    # - the computed themes (CosmicTheme.Dark/Light): COSMIC rebuilds them
    #   from the .Builder settings, which are exported
    # - the keyboard layout and clock format, set from the installer choices
    #   on every install
    # - any file naming the home folder or the user name (recent files,
    #   bookmarks, personal wallpapers...), binary files and big files
    skip_dirs=(
        com.system76.CosmicTheme.Dark/
        com.system76.CosmicTheme.Light/
    )
    skip_files=(
        com.system76.CosmicComp/v1/xkb_config
    )
    user_name="$(id -un)"
    exported=0
    skipped=()
    {
        echo "# cosmic-blue: COSMIC settings export"
        echo "# Date: $(date -u +%Y-%m-%dT%H:%MZ)"
        echo "# Image: $(jq -r '."image-ref" + " " + ."image-tag"' /usr/share/ublue-os/image-info.json 2>/dev/null || echo unknown)"
        echo "# COSMIC: $(rpm -q --qf '%{VERSION}' cosmic-settings 2>/dev/null || echo unknown)"
        echo "# Each file starts with a '===== <path> =====' line; paths are relative to ~/.config/cosmic."
    } >"${out}"
    while IFS= read -r -d '' file; do
        rel="${file#"${config}/"}"
        reason=""
        for dir in "${skip_dirs[@]}"; do
            [[ "${rel}" == "${dir}"* ]] && reason="rebuilt by COSMIC"
        done
        for name in "${skip_files[@]}"; do
            [[ "${rel}" == "${name}" ]] && reason="set by the installer"
        done
        if [[ -z "${reason}" ]]; then
            if [[ "$(stat -c %s "${file}")" -gt 65536 ]]; then
                reason="bigger than 64 KB"
            elif [[ -s "${file}" ]] && ! grep -qI . "${file}"; then
                reason="binary"
            elif grep -qF -e "${HOME}" -e "/home/" -e "/var/home/" "${file}" \
                || grep -qw -- "${user_name}" "${file}"; then
                reason="personal"
            fi
        fi
        if [[ -n "${reason}" ]]; then
            skipped+=("${rel} (${reason})")
            continue
        fi
        printf '\n===== %s =====\n' "${rel}" >>"${out}"
        cat "${file}" >>"${out}"
        printf '\n' >>"${out}"
        exported=$((exported + 1))
    done < <(find "${config}" -type f ! -name '*.tmp' -print0 | sort -z)
    {
        echo
        echo "# Left out (${#skipped[@]}):"
        for item in "${skipped[@]}"; do echo "#   ${item}"; done
    } >>"${out}"
    echo "Saved ${exported} settings to ${out}"
    if [[ "${#skipped[@]}" -gt 0 ]]; then
        echo "Left out ${#skipped[@]} files (listed at the end of the export):"
        printf '  %s\n' "${skipped[@]}"
    fi
    echo "It is plain text: read it before sharing it."

# Install Microsoft fonts: Arial, Times New Roman...
[group('Apps')]
install-microsoft-fonts confirm="ask": (_microsoft-fonts "install" confirm)

# Remove the Microsoft fonts
[group('Apps')]
remove-microsoft-fonts: (_microsoft-fonts "remove" "yes")

[private]
_microsoft-fonts action confirm:
    #!/usr/bin/bash
    set -euo pipefail
    # The fonts are free to download and use but may not be redistributed, so
    # the image cannot ship them: they are fetched from the original
    # installers (the same files and checksums as Debian's
    # ttf-mscorefonts-installer) and extracted into the user's fonts folder,
    # which Flatpak apps also read.
    dest="${XDG_DATA_HOME:-${HOME}/.local/share}/fonts/microsoft-core-fonts"
    if [[ "{{ action }}" == remove ]]; then
        if [[ ! -d "${dest}" ]]; then
            echo "Microsoft core fonts are not installed."
            exit 0
        fi
        rm -rf "${dest}"
        fc-cache -f
        echo "Microsoft core fonts removed."
        exit 0
    elif [[ "{{ action }}" != install ]]; then
        echo "Usage: ujust install-microsoft-fonts or ujust remove-microsoft-fonts" >&2
        exit 1
    fi
    declare -A installers=(
        [andale32.exe]=0524fe42951adc3a7eb870e32f0920313c71f170c859b5f770d82b4ee111e970
        [arial32.exe]=85297a4d146e9c87ac6f74822734bdee5f4b2a722d7eaa584b7f2cbf76f478f6
        [arialb32.exe]=a425f0ffb6a1a5ede5b979ed6177f4f4f4fdef6ae7c302a7b7720ef332fec0a8
        [comic32.exe]=9c6df3feefde26d4e41d4a4fe5db2a89f9123a772594d7f59afd062625cd204e
        [courie32.exe]=bb511d861655dde879ae552eb86b134d6fae67cb58502e6ff73ec5d9151f3384
        [georgi32.exe]=2c2c7dcda6606ea5cf08918fb7cd3f3359e9e84338dc690013f20cd42e930301
        [impact32.exe]=6061ef3b7401d9642f5dfdb5f2b376aa14663f6275e60a51207ad4facf2fccfb
        [times32.exe]=db56595ec6ef5d3de5c24994f001f03b2a13e37cee27bc25c58f6f43e8f807ab
        [trebuc32.exe]=5a690d9bb8510be1b8b4fe49f1f2319651fe51bbe54775ddddd8ef0bd07fdac9
        [verdan32.exe]=c1cb61255e363166794e47664e2f21af8e3a26cb6346eb8d2ae2fa85dd5aad96
        [webdin32.exe]=64595b5abc1080fba8610c5c34fab5863408e806aafe84653ca8575bed17d75a
    )
    echo "This downloads Microsoft's core fonts for the web: Andale Mono, Arial,"
    echo "Arial Black, Comic Sans MS, Courier New, Georgia, Impact, Times New Roman,"
    echo "Trebuchet MS, Verdana and Webdings."
    echo "They are covered by Microsoft's End User License Agreement:"
    echo "  https://corefonts.sourceforge.net/eula.htm"
    echo "Most documents do not need them: cosmic-blue already has look-alike fonts"
    echo "with the same sizes (Liberation, Carlito, Caladea)."
    # `yes`: the license was accepted already ("Accept and install" in
    # cosmic-blue Tools, which shows the license link)
    if [[ "{{ confirm }}" != yes ]]; then
        read -r -p "Do you accept the license and want to install them? [y/N] " answer
        [[ "${answer}" =~ ^[YyOo] ]] || { echo "Nothing installed."; exit 0; }
    fi
    tmp="$(mktemp -d)"
    trap 'rm -rf "${tmp}"' EXIT
    for exe in "${!installers[@]}"; do
        echo "Downloading ${exe}..."
        curl -fsSL --retry 3 -o "${tmp}/${exe}" "https://downloads.sourceforge.net/corefonts/${exe}"
        echo "${installers[${exe}]}  ${tmp}/${exe}" | sha256sum --quiet -c -
        cabextract -q -L -d "${tmp}/fonts" "${tmp}/${exe}"
    done
    install -d "${dest}"
    install -m 0644 "${tmp}"/fonts/*.ttf "${dest}/"
    fc-cache -f "${dest}"
    echo "Installed $(find "${dest}" -name '*.ttf' | wc -l) font files in ${dest}."
    echo "Restart open apps to see them. Remove them with: ujust remove-microsoft-fonts"

# Install command line tools: bat, eza, fd, ripgrep, zoxide...
[group('Apps')]
install-cli-tools: (_cli-tools "install")

# Remove the command line tools
[group('Apps')]
remove-cli-tools: (_cli-tools "remove")

[private]
_cli-tools action:
    #!/usr/bin/bash
    set -euo pipefail
    # The same tools as Bluefin's `ujust bluefin-cli` (its cli.Brewfile), plus
    # its shell shortcuts ("bling"), added to the shell's startup file between
    # the same markers so either command can turn them off again.
    brewfile=/usr/share/ublue-os/homebrew/cli.Brewfile
    bling_dir=/usr/share/ublue-os/bling
    case "$(basename "${SHELL}")" in
        fish) script=bling.fish; rc="${XDG_CONFIG_HOME:-${HOME}/.config}/fish/config.fish" ;;
        zsh) script=bling.sh; rc="${ZDOTDIR:-${HOME}}/.zshrc" ;;
        *) script=bling.sh; rc="${HOME}/.bashrc" ;;
    esac
    # The tools this recipe installed (not those already there), so that
    # `remove` leaves the user's own Homebrew tools alone
    added="${XDG_STATE_HOME:-${HOME}/.local/state}/cosmic-blue/cli-tools.list"
    bling_on() { [[ -f "${rc}" ]] && grep -q "source ${bling_dir}/${script}" "${rc}"; }
    case "{{ action }}" in
        install)
            echo "This installs these command line tools for your account with Homebrew:"
            brew bundle list --file="${brewfile}" --formula | sed 's|.*/||' | paste -sd ' ' | fold -s -w 76 | sed 's/^/  /'
            echo
            mkdir -p "$(dirname "${added}")"
            for formula in $(brew bundle list --file="${brewfile}" --formula); do
                brew list --formula "${formula##*/}" >/dev/null 2>&1 || echo "${formula##*/}" >>"${added}"
            done
            brew bundle --file="${brewfile}"
            if ! bling_on; then
                echo
                echo "Shortcuts: ls lists files with eza, cat shows files with bat,"
                echo "z <folder> jumps to a folder you often visit (zoxide)."
                if gum confirm "Turn on these shortcuts in your terminal?"; then
                    mkdir -p "$(dirname "${rc}")"
                    {
                        echo "### ${script} source start"
                        echo "test -f ${bling_dir}/${script} && source ${bling_dir}/${script}"
                        echo "### ${script} source end"
                    } >>"${rc}"
                fi
            fi
            echo "Done. Open a new terminal to use them."
            echo "Remove them with: ujust remove-cli-tools"
            ;;
        remove)
            if bling_on; then
                sed -i "/### ${script} source start/,/### ${script} source end/d" "${rc}"
                echo "Shortcuts turned off."
            fi
            # Only the tools this recipe installed; one another Homebrew
            # package depends on is kept
            if [[ -f "${added}" ]]; then
                for formula in $(sort -u "${added}"); do
                    if brew list --formula "${formula}" >/dev/null 2>&1; then
                        brew uninstall --formula "${formula}" || true
                    fi
                done
                rm -f "${added}"
            else
                echo "No tools from ujust install-cli-tools are installed on this account."
            fi
            echo "Done. Open a new terminal."
            ;;
        *)
            echo "Usage: ujust install-cli-tools or ujust remove-cli-tools" >&2
            exit 1
            ;;
    esac

# Install Steam and Heroic
[group('Apps')]
install-gaming: (_gaming "install")

# Remove Steam and Heroic (games are kept)
[group('Apps')]
remove-gaming: (_gaming "remove")

[private]
_gaming action:
    #!/usr/bin/bash
    set -euo pipefail
    # Both are Flathub apps, installed system-wide like the first-boot apps.
    # Flatpak adds the graphics driver parts they need (NVIDIA included) by
    # itself. Removing them keeps the games and saves in ~/.var/app.
    apps=(com.valvesoftware.Steam com.heroicgameslauncher.hgl)
    case "{{ action }}" in
        install)
            echo "This installs from Flathub:"
            echo "  Steam   - Valve's game store and launcher (Proton runs Windows games)"
            echo "  Heroic  - games from Epic Games, GOG and Amazon Prime Gaming"
            echo "The download is about 1.5 GB, games not included."
            if [[ -t 0 ]] && ! gum confirm "Install them?"; then
                echo "Nothing installed."
                exit 0
            fi
            flatpak install --system --noninteractive flathub "${apps[@]}"
            echo "Done: Steam and Heroic are in the app list."
            echo "Remove them with: ujust remove-gaming"
            ;;
        remove)
            readarray -t installed < <(flatpak list --system --app --columns=application | grep -xF -f <(printf '%s\n' "${apps[@]}") || true)
            if [[ "${#installed[@]}" -eq 0 ]]; then
                echo "Steam and Heroic are not installed."
                exit 0
            fi
            flatpak uninstall --system --noninteractive "${installed[@]}"
            echo "Removed. Your games and saves stay in ~/.var/app; delete"
            echo "~/.var/app/com.valvesoftware.Steam or ~/.var/app/com.heroicgameslauncher.hgl to free the space."
            ;;
        *)
            echo "Usage: ujust install-gaming or ujust remove-gaming" >&2
            exit 1
            ;;
    esac

# Bluefin's installers (projectbluefin/common), renamed _bluefin-* at build
# time (build_files/10-system-files.sh): these commands run them to install,
# and add the removal they lack (remove-*). Removing never deletes your own files
# (projects, virtual machine disks are only removed after a question).

# Install developer tools from a menu: Docker, VS Code...
[group('Apps')]
install-dev-tools: (_dev-tools "install")

# Remove developer tools, picked from a menu
[group('Apps')]
remove-dev-tools: (_dev-tools "remove")

[private]
_dev-tools action:
    #!/usr/bin/bash
    set -euo pipefail
    case "{{ action }}" in
        install) exec ujust _bluefin-dev-tools ;;
        remove) ;;
        *) echo "Usage: ujust install-dev-tools or ujust remove-dev-tools" >&2; exit 1 ;;
    esac
    # What the menu installs: "Label|kind|names" (kind: formula, cask, flatpak, ujust)
    tools=(
        "devcontainer CLI|formula|devcontainer"
        "Docker|formula|docker docker-compose lazydocker dive"
        "Podman Desktop|flatpak|io.podman_desktop.PodmanDesktop"
        "Lima|ujust|lima"
        "VS Code|cask|visual-studio-code-linux"
        "VSCodium|cask|vscodium-linux"
        "Antigravity IDE|cask|antigravity-ide-linux"
        "Zed|cask|zed"
        "JetBrains Toolbox|ujust|jetbrains-toolbox"
        "Neovim|formula|neovim"
        "Helix|formula|helix"
        "vim|formula|vim"
        "micro|formula|micro"
    )
    present() { # kind names
        case "$1" in
            formula) brew list --formula "${2%% *}" >/dev/null 2>&1 ;;
            cask) brew list --cask "$2" >/dev/null 2>&1 ;;
            flatpak) flatpak info --system "$2" >/dev/null 2>&1 ;;
            ujust)
                case "$2" in
                    lima) brew list --formula lima >/dev/null 2>&1 ;;
                    jetbrains-toolbox) brew list --cask jetbrains-toolbox-linux >/dev/null 2>&1 ;;
                esac
                ;;
        esac
    }
    installed=()
    for tool in "${tools[@]}"; do
        IFS='|' read -r label kind names <<<"${tool}"
        if present "${kind}" "${names}"; then installed+=("${label}"); fi
    done
    if [[ "${#installed[@]}" -eq 0 ]]; then
        echo "No tools from ujust install-dev-tools are installed."
        exit 0
    fi
    chosen="$(gum choose --no-limit --header "Which tools do you want to remove? (space to select)" "${installed[@]}" || true)"
    [[ -n "${chosen}" ]] || { echo "Nothing removed."; exit 0; }
    for tool in "${tools[@]}"; do
        IFS='|' read -r label kind names <<<"${tool}"
        grep -qxF -- "${label}" <<<"${chosen}" || continue
        echo "Removing ${label}..."
        case "${kind}" in
            formula) read -ra formulas <<<"${names}"; brew uninstall --formula "${formulas[@]}" || true ;;
            cask) brew uninstall --cask "${names}" || true ;;
            flatpak) flatpak uninstall --system --noninteractive "${names}" || true ;;
            ujust) ujust "remove-${names}" ;;
        esac
    done
    if [[ "${#installed[@]}" -eq "$(grep -c . <<<"${chosen}")" ]]; then
        rm -f "${HOME}/.config/bluefin/devmode"
    fi
    echo "Done. Your projects and settings are kept."

# Install virt-manager, to run virtual machines
[group('Apps')]
install-vms: (_vms "install")

# Remove virt-manager (virtual machine disks are kept)
[group('Apps')]
remove-vms: (_vms "remove")

[private]
_vms action:
    #!/usr/bin/bash
    set -euo pipefail
    case "{{ action }}" in
        install) exec ujust _bluefin-vms ;;
        remove)
            if ! flatpak info --system org.virt_manager.virt-manager >/dev/null 2>&1; then
                echo "Virtual machines (virt-manager) are not installed."
                exit 0
            fi
            echo "This removes virt-manager. Your virtual machines' disks are kept"
            echo "(in ~/.local/share/libvirt): installing it again finds them."
            if [[ -t 0 ]] && ! gum confirm "Remove virt-manager?"; then echo "Nothing removed."; exit 0; fi
            flatpak uninstall --system --noninteractive \
                org.virt_manager.virt-manager org.virt_manager.virt_manager.Extension.Qemu || true
            sed -i '/uri_default.*session/d' "${HOME}/.config/libvirt/libvirt.conf" 2>/dev/null || true
            echo "virt-manager removed."
            ;;
        *) echo "Usage: ujust install-vms or ujust remove-vms" >&2; exit 1 ;;
    esac

# Install Lima, a light Ubuntu virtual machine
[group('Apps')]
install-lima: (_lima "install")

# Remove Lima and its virtual machine
[group('Apps')]
remove-lima: (_lima "remove")

[private]
_lima action:
    #!/usr/bin/bash
    set -euo pipefail
    case "{{ action }}" in
        install) exec ujust _bluefin-lima ;;
        remove)
            if ! command -v limactl >/dev/null; then
                echo "Lima is not installed."
                exit 0
            fi
            if limactl ls -q 2>/dev/null | grep -qx ubuntu; then
                echo "This deletes the Lima \"ubuntu\" virtual machine and everything inside it,"
                echo "then removes Lima. Files shared from your home folder are kept."
                if [[ ! -t 0 ]] || ! gum confirm "Delete the ubuntu virtual machine and remove Lima?"; then
                    echo "Nothing removed."
                    exit 0
                fi
                limactl autostart disable ubuntu 2>/dev/null || true
                limactl stop -f ubuntu 2>/dev/null || true
                limactl delete -f ubuntu
            fi
            brew uninstall --formula lima || true
            sed -i '\|^Include ~/.lima/\*/ssh.config$|d' "${HOME}/.ssh/config" 2>/dev/null || true
            echo "Lima removed."
            ;;
        *) echo "Usage: ujust install-lima or ujust remove-lima" >&2; exit 1 ;;
    esac

# Install Kubernetes and cloud native tools
[group('Apps')]
install-cncf-tools: (_cncf-tools "install")

# Remove the cloud native tools
[group('Apps')]
remove-cncf-tools: (_cncf-tools "remove")

[private]
_cncf-tools action:
    #!/usr/bin/bash
    set -euo pipefail
    brewfile=/usr/share/ublue-os/homebrew/cncf.Brewfile
    case "{{ action }}" in
        install) exec ujust _bluefin-cncf-tools ;;
        remove)
            removed=0
            if command -v brew >/dev/null; then
                for formula in $(brew bundle list --file="${brewfile}" --formula); do
                    if brew list --formula "${formula##*/}" >/dev/null 2>&1; then
                        brew uninstall --formula "${formula##*/}" || true
                        removed=1
                    fi
                done
            fi
            if [[ "${removed}" -eq 0 ]]; then echo "No cloud native tools are installed."; exit 0; fi
            echo "Done. A tool that another Homebrew package needs is kept."
            ;;
        *) echo "Usage: ujust install-cncf-tools or ujust remove-cncf-tools" >&2; exit 1 ;;
    esac

# Install Goose, an AI assistant for the terminal
[group('Apps')]
install-ai-tools: (_ai-tools "install")

# Remove Goose
[group('Apps')]
remove-ai-tools: (_ai-tools "remove")

[private]
_ai-tools action:
    #!/usr/bin/bash
    set -euo pipefail
    brewfile=/usr/share/ublue-os/homebrew/ai-tools.Brewfile
    case "{{ action }}" in
        install) exec ujust _bluefin-ai-tools ;;
        remove)
            if ! command -v goose >/dev/null && [[ ! -f "${XDG_CONFIG_HOME:-${HOME}/.config}/goose/config.yaml" ]]; then
                echo "Goose is not installed."
                exit 0
            fi
            for formula in $(brew bundle list --file="${brewfile}" --formula); do
                if brew list --formula "${formula##*/}" >/dev/null 2>&1; then
                    brew uninstall --formula "${formula##*/}" || true
                fi
            done
            for cask in $(brew bundle list --file="${brewfile}" --cask); do
                if brew list --cask "${cask##*/}" >/dev/null 2>&1; then
                    brew uninstall --cask "${cask##*/}" || true
                fi
            done
            # The configuration this command wrote, unless you changed it
            config="${XDG_CONFIG_HOME:-${HOME}/.config}/goose/config.yaml"
            if [[ -f "${config}" ]] && cmp -s "${config}" /usr/share/ublue-os/goose/config.yaml; then
                rm -f "${config}"
            fi
            echo "Done."
            ;;
        *) echo "Usage: ujust install-ai-tools or ujust remove-ai-tools" >&2; exit 1 ;;
    esac

# Install the ASUS laptop tools (asusctl, ROG Control Center)
[group('Apps')]
install-asus: (_asus "install")

# Remove the ASUS laptop tools
[group('Apps')]
remove-asus: (_asus "remove")

[private]
_asus action:
    #!/usr/bin/bash
    set -euo pipefail
    case "{{ action }}" in
        install) exec ujust _bluefin-asus ;;
        remove)
            if ! command -v brew >/dev/null || ! brew list --cask asusctl-linux >/dev/null 2>&1; then
                echo "The ASUS laptop tools are not installed."
                exit 0
            fi
            systemctl --user disable --now asusd-user.service 2>/dev/null || true
            sudo systemctl disable --now asusd.service asus-shutdown.service 2>/dev/null || true
            for cask in rog-control-center-linux asusctl-linux; do
                if brew list --cask "${cask}" >/dev/null 2>&1; then
                    brew uninstall --cask "${cask}" || true
                fi
            done
            echo "ASUS tools removed."
            ;;
        *) echo "Usage: ujust install-asus or ujust remove-asus" >&2; exit 1 ;;
    esac

# Install JetBrains Toolbox, to get JetBrains IDEs
[group('Apps')]
install-jetbrains-toolbox: (_jetbrains-toolbox "install")

# Remove JetBrains Toolbox
[group('Apps')]
remove-jetbrains-toolbox: (_jetbrains-toolbox "remove")

[private]
_jetbrains-toolbox action:
    #!/usr/bin/bash
    set -euo pipefail
    case "{{ action }}" in
        install) exec ujust _bluefin-jetbrains-toolbox ;;
        remove)
            if ! command -v brew >/dev/null || ! brew list --cask jetbrains-toolbox-linux >/dev/null 2>&1; then
                echo "JetBrains Toolbox is not installed."
                exit 0
            fi
            brew uninstall --cask jetbrains-toolbox-linux
            echo "JetBrains Toolbox removed. IDEs it installed stay in"
            echo "~/.local/share/JetBrains: delete that folder to remove them too."
            ;;
        *) echo "Usage: ujust install-jetbrains-toolbox or ujust remove-jetbrains-toolbox" >&2; exit 1 ;;
    esac

# Install OpenTabletDriver, for drawing tablets
[group('Apps')]
install-opentabletdriver: (_opentabletdriver "install")

# Remove OpenTabletDriver
[group('Apps')]
remove-opentabletdriver: (_opentabletdriver "remove")

[private]
_opentabletdriver action:
    #!/usr/bin/bash
    set -euo pipefail
    case "{{ action }}" in
        install) exec ujust _bluefin-opentabletdriver ;;
        remove)
            if ! flatpak info --system net.opentabletdriver.OpenTabletDriver >/dev/null 2>&1 \
                && [[ ! -e "${HOME}/.config/systemd/user/opentabletdriver.service" ]]; then
                echo "OpenTabletDriver is not installed."
                exit 0
            fi
            systemctl --user disable --now opentabletdriver.service 2>/dev/null || true
            rm -f "${HOME}/.config/systemd/user/opentabletdriver.service"
            systemctl --user daemon-reload
            sudo rm -f /etc/modprobe.d/blacklist-opentabletdriver.conf /etc/udev/rules.d/71-opentabletdriver.rules
            flatpak uninstall --system --noninteractive net.opentabletdriver.OpenTabletDriver 2>/dev/null || true
            echo "OpenTabletDriver removed. Restart to use the usual tablet drivers again."
            ;;
        *) echo "Usage: ujust install-opentabletdriver or ujust remove-opentabletdriver" >&2; exit 1 ;;
    esac

# Turn the FlatPark app source on or off (off by default)
[group('Apps')]
toggle-flatpark mode="toggle":
    #!/usr/bin/bash
    set -euo pipefail
    # FlatPark (https://flatpark.org) is a Flatpak hub with apps that are not
    # on Flathub. How far its apps can be trusted is not established yet, so
    # cosmic-blue leaves it off. When on, it has the lowest priority, and its
    # filter hides every app that is also on Flathub: those keep coming from
    # Flathub. It is added to your account's Flatpak installation, the one
    # COSMIC Store manages: FlatPark is listed there under Software
    # repositories (Recommended Flatpak sources) and can be added or removed
    # there too. Ask before every change, unless run without a terminal.
    repofile=/usr/share/cosmic-blue/flatpak/flatpark.flatpakrepo
    filter=/usr/share/flatpak/flatpark.filter
    confirm() {
        [[ -t 0 ]] || return 0
        read -r -p "$1 [y/N] " answer
        [[ "${answer}" =~ ^[YyOo] ]]
    }
    is_on() { flatpak remotes --user --columns=name | grep -qx flatpark; }
    mode="{{ mode }}"
    if [[ "${mode}" == toggle ]]; then
        if is_on; then mode=off; else mode=on; fi
    fi
    case "${mode}" in
        status)
            if is_on; then echo "FlatPark is on."; else echo "FlatPark is off."; fi
            ;;
        on)
            if is_on; then
                flatpak remote-modify --user --prio=0 --filter="${filter}" flatpark
                echo "FlatPark is already on."
                exit 0
            fi
            echo "FlatPark offers Flatpak apps that are not on Flathub. Its apps are not"
            echo "reviewed the way Flathub's are: install only apps whose authors you trust."
            echo "Apps that are also on Flathub or in the COSMIC repository keep coming from there."
            confirm "Turn FlatPark on?" || { echo "FlatPark stays off."; exit 0; }
            flatpak remote-add --user --if-not-exists --prio=0 --filter="${filter}" \
                flatpark "${repofile}"
            echo "FlatPark is on: its apps now show in COSMIC Store."
            echo "Turn it off with: ujust toggle-flatpark off"
            ;;
        off)
            if ! is_on; then
                echo "FlatPark is already off."
                exit 0
            fi
            readarray -t refs < <(flatpak list --user --columns=ref,origin | awk '$2 == "flatpark" {print $1}')
            if [[ "${#refs[@]}" -gt 0 ]]; then
                echo "Installed from FlatPark:"
                printf '  %s\n' "${refs[@]}"
                if [[ ! -t 0 ]]; then
                    echo "Uninstall them first, or run this in a terminal." >&2
                    exit 1
                fi
                confirm "Uninstall them and turn FlatPark off?" || { echo "FlatPark stays on."; exit 0; }
                flatpak uninstall --user --noninteractive "${refs[@]}"
            fi
            flatpak remote-delete --user flatpark
            echo "FlatPark is off."
            # Turned on system-wide by an older version of this command
            if flatpak remotes --system --columns=name | grep -qx flatpark \
                && ! flatpak list --system --columns=origin | grep -qx flatpark; then
                flatpak remote-delete --system flatpark || true
            fi
            ;;
        *)
            echo "Usage: ujust toggle-flatpark [toggle|on|off|status]" >&2
            exit 1
            ;;
    esac

# Switch between the stable (weekly), testing (daily) and dev images
[group('System')]
switch-channel channel="status":
    #!/usr/bin/bash
    set -euo pipefail
    # The image the next boot starts (the first deployment), as rpm-ostree
    # records it: "ostree-image-signed:docker://<image>" when signatures are
    # checked, "ostree-unverified-registry:<image>" otherwise. Readable
    # without root.
    ref="$(rpm-ostree status --json | jq -r '.deployments[0]."container-image-reference" // empty')"
    image="${ref#*docker://}"
    image="${image#ostree-unverified-registry:}"
    repo="${image%:*}"
    tag="${image##*:}"
    if [[ "${repo}" != ghcr.io/cosmic-blue-os/* || "${repo}" == "${image}" ]]; then
        echo "This system does not follow a cosmic-blue image (${ref:-none}): nothing to switch." >&2
        exit 1
    fi
    channel="{{ channel }}"
    case "${channel}" in
        status)
            case "${tag}" in
                stable) echo "Channel: stable (updated once a week, after more tests)." ;;
                testing) echo "Channel: testing (updated every day)." ;;
                latest) echo "Channel: testing (updated every day), under its old name latest." ;;
                dev) echo "Channel: dev (every change, as soon as it is merged and tested; for testers)." ;;
                *) echo "Pinned to ${image}: no automatic updates to newer images." ;;
            esac
            echo "Switch with: ujust switch-channel stable, testing or dev"
            exit 0
            ;;
        # The old name of testing
        latest) channel=testing ;;
        stable|testing|dev) ;;
        *)
            echo "Usage: ujust switch-channel [status|stable|testing|dev]" >&2
            exit 1
            ;;
    esac
    target="${repo}:${channel}"
    if [[ "${tag}" == "${channel}" ]]; then
        echo "Already on the ${channel} channel (${target})."
        exit 0
    fi
    case "${channel}" in
        stable)
            echo "stable gets a new image once a week, after the update and install tests."
            echo "It can be a few days older than the image you have now: that is fine,"
            echo "your files and settings are kept."
            ;;
        testing)
            echo "testing gets a new image every day, as soon as it passes the build tests."
            ;;
        dev)
            echo "dev gets every change as soon as it is merged and passes the build tests,"
            echo "several times a day: for testing what is coming. Something can break;"
            echo "the previous image stays one reboot away (sudo bootc rollback)."
            ;;
    esac
    echo "The ${target} image is downloaded now, its signature is checked,"
    echo "and it starts at the next reboot."
    if [[ -t 0 ]] && ! gum confirm "Switch to ${target}?"; then
        echo "Nothing changed."
        exit 0
    fi
    sudo bootc switch --enforce-container-sigpolicy "${target}"
    echo "Done. Reboot to start the ${channel} image."
    case "${tag}" in
        stable|testing|dev) echo "To go back: ujust switch-channel ${tag}" ;;
        latest) echo "To go back: ujust switch-channel testing" ;;
    esac

# Install firmware updates (BIOS/UEFI, SSD, dock...)
[group('System')]
update-firmware:
    #!/usr/bin/bash
    set -euo pipefail
    # fwupd lists the devices that can be updated; the updates come from the
    # LVFS (fwupd.org), signed by the device makers. Nothing installs on its
    # own: the Firmware app shows the same updates.
    echo "Downloading the list of firmware updates..."
    if ! fwupdmgr refresh --force >/dev/null; then
        echo "Could not download the list of firmware updates (no network?)." >&2
        exit 1
    fi
    # Exit code 2: nothing to update
    status=0
    fwupdmgr update --no-unreported-check || status=$?
    case "${status}" in
        0) ;;
        2) echo "No firmware updates for this computer." ;;
        *) exit "${status}" ;;
    esac

# Stop charging the laptop at 80% so the battery lasts longer
[group('System')]
toggle-battery-limit mode="toggle":
    #!/usr/bin/bash
    set -euo pipefail
    # UPower (the battery service behind COSMIC's battery applet) caps the
    # charge, on laptops whose driver allows it (ThinkPad, ASUS, Dell,
    # Framework, Huawei, LG, Samsung, MSI...): charging stops at 80% and
    # starts again below 75%, unless the laptop maker set other values in
    # UPower's hardware database. UPower keeps the setting across reboots.
    property() { # device name
        busctl get-property org.freedesktop.UPower "$1" org.freedesktop.UPower.Device "$2" 2>/dev/null \
            | awk '{print $2}'
    }
    # The laptop's own battery, not a mouse's or a headset's
    battery=""
    for device in $(upower --enumerate | grep '/battery_'); do
        if [[ "$(property "${device}" PowerSupply)" == true ]]; then
            battery="${device}"
            break
        fi
    done
    if [[ -z "${battery}" ]]; then
        echo "No laptop battery found: nothing to do."
        exit 0
    fi
    if [[ "$(property "${battery}" ChargeThresholdSupported)" != true ]]; then
        echo "This laptop does not let the system limit its charge."
        echo "Some laptops offer it in their firmware settings: ujust reboot-to-bios"
        exit 0
    fi
    enabled="$(property "${battery}" ChargeThresholdEnabled)"
    case "{{ mode }}" in
        on) wanted=true ;;
        off) wanted=false ;;
        toggle) if [[ "${enabled}" == true ]]; then wanted=false; else wanted=true; fi ;;
        status) wanted="${enabled}" ;;
        *)
            echo "Usage: ujust toggle-battery-limit [toggle|on|off|status]" >&2
            exit 1
            ;;
    esac
    if [[ "${wanted}" != "${enabled}" ]]; then
        busctl call org.freedesktop.UPower "${battery}" org.freedesktop.UPower.Device \
            EnableChargeThreshold b "${wanted}"
    fi
    if [[ "${wanted}" == true ]]; then
        start="$(property "${battery}" ChargeStartThreshold)"
        end="$(property "${battery}" ChargeEndThreshold)"
        echo "Battery limit on: charging stops at ${end:-80}% and starts again below ${start:-75}%."
    else
        echo "Battery limit off: the battery charges to 100%."
    fi

# Earlier names (cosmic-blue's and Bluefin's, renamed in build_files/10-system-files.sh),
# still accepted but not listed
[private]
alias bug-report := report-bug

[private]
alias devmode := install-dev-tools

[private]
alias toggle-devmode := install-dev-tools

[private]
alias setup-vms := install-vms

[private]
alias setup-lima := install-lima

[private]
alias install-system-flatpaks := reset-default-apps

[private]
alias cncf := install-cncf-tools

[private]
alias bios := reboot-to-bios

[private]
alias bios-info := show-bios-info

[private]
alias logs-this-boot := show-logs

[private]
alias logs-last-boot := show-previous-logs

[private]
alias benchmark := run-benchmark

[private]
alias check-sb-key := check-secure-boot

[private]
alias powerwash := reset-system

# Open a bug report with your system details filled in
[group('System')]
report-bug mode="open":
    #!/usr/bin/bash
    set -euo pipefail
    # The details (cosmic-blue-system-info) hold no personal data. They go
    # into the "System details" field of the bug report form through its
    # URL (cosmic-blue-report-bug, also run by "Report a problem" in
    # cosmic-blue Tools): nothing is sent until the report is submitted.
    form="https://github.com/cosmic-blue-os/cosmic-blue/issues/new?template=bug_report.yml"
    info="$(/usr/libexec/cosmic-blue-system-info)"
    case "{{ mode }}" in
        open) ;;
        print)
            printf '%s\n' "${info}"
            exit 0
            ;;
        *)
            echo "Usage: ujust report-bug [open|print]" >&2
            exit 1
            ;;
    esac
    echo "System details for the bug report (no personal data):"
    echo
    printf '%s\n' "${info}"
    echo
    if [[ -n "${WAYLAND_DISPLAY:-}" ]] && printf '%s\n' "${info}" | wl-copy 2>/dev/null; then
        echo "Copied to the clipboard."
    fi
    if [[ -z "${WAYLAND_DISPLAY:-}${DISPLAY:-}" ]]; then
        echo "Report the bug at ${form}"
        echo "and paste the details above into the \"System details\" field."
        exit 0
    fi
    if gum confirm "Open the bug report form in the browser?"; then
        /usr/libexec/cosmic-blue-report-bug open
        echo "Describe the problem in the form, then submit it (a GitHub account is needed)."
    else
        echo "Report the bug later at ${form}"
    fi

# Free disk space: what nothing uses any more (your files stay)
[group('System')]
clean-system confirm="ask":
    #!/usr/bin/bash
    set -euo pipefail
    # Same script as "Clean up" in cosmic-blue Tools (Maintenance page).
    # Unused Flatpak runtimes, dangling Podman/Docker images, old Homebrew
    # downloads, update caches. Named images, containers and volumes stay:
    # `podman system prune` removes those when you are sure.
    if [[ "{{ confirm }}" != yes ]]; then
        gum confirm "Remove what nothing uses any more (your files stay)?" || exit 0
    fi
    /usr/libexec/cosmic-blue-clean

# NVIDIA with Secure Boot: enroll the driver key (password "cosmicblue")
[group('System')]
enroll-secure-boot-key:
    #!/usr/bin/bash
    set -euo pipefail
    # The password is not a secret: the firmware's MOK manager asks for it
    # once, at the machine itself, to confirm that the key was queued on purpose.
    password="cosmicblue"
    key=/etc/pki/akmods/certs/akmods-ublue.der
    sudo mokutil --timeout -1
    printf '%s\n%s\n' "${password}" "${password}" | sudo mokutil --import "${key}"
    echo "At the next reboot, a blue screen (MOK manager, QWERTY keyboard layout) shows up:"
    echo "choose \"Enroll MOK\", then \"Continue\", and type the password \"${password}\"."
