#!/usr/bin/bash
# COSMIC keeps each program's settings in ~/.config/cosmic/<id>/v<N>/ and
# reads the system's defaults from /usr/share/cosmic/<id>/v<N>/, where N is
# the version of that program's settings. Celenn OS ships defaults there
# (system_files, build_files/theme), and its apps and ujust recipes read and
# write some of these settings, at the versions of the table below. A COSMIC
# that moves one of them to another version would silently ignore them.
#
#   tests/check-cosmic-config.sh
#       the repository against the table: every folder of defaults in
#       system_files is in it, at its version, and the apps' list of
#       settings at version 2 is the table's (tests/lint.sh)
#   tests/check-cosmic-config.sh <COSMIC version>      (e.g. 1.12.0)
#       the same, then the version each COSMIC program declares in its
#       source for that release: public files of github.com/pop-os at the
#       release's tag (epoch-<version>), libcosmic at the revision that
#       cosmic-settings locks for it. Run by check-cosmic-version.yml before
#       an update to a new COSMIC: a version that moved stops the update
#       until the apps and the table are adapted.
set -euo pipefail
cd "$(dirname "$0")/.."

# id | version Celenn OS uses | repository (libcosmic: the locked revision,
# others: the release's tag) | file | what declares the version there:
# struct:<name> (#[version = N] on that struct), const:<name> (a u64
# constant) or re:<Perl regular expression, its first group the version>
checks=(
    "com.system76.CosmicAppLibrary|1|cosmic-applibrary|src/app_group.rs|re:fn version\(\) -> u64 \{\s*(\d+)\s*\}"
    "com.system76.CosmicAppList|1|cosmic-applets|cosmic-app-list/cosmic-app-list-config/src/lib.rs|struct:AppListConfig"
    "com.system76.CosmicAppletTime|1|cosmic-applets|cosmic-applets-config/src/time.rs|struct:TimeAppletConfig"
    "com.system76.CosmicBackground|1|cosmic-bg|config/src/lib.rs|const:VERSION"
    "com.system76.CosmicBackground.Dark|1|cosmic-bg|config/src/lib.rs|const:VERSION"
    "com.system76.CosmicBackground.Light|1|cosmic-bg|config/src/lib.rs|const:VERSION"
    "com.system76.CosmicComp|1|cosmic-comp|cosmic-comp-config/src/lib.rs|struct:CosmicCompConfig"
    "com.system76.CosmicPanel|1|cosmic-panel|cosmic-panel-config/src/container_config.rs|const:VERSION"
    "com.system76.CosmicPanel.Dock|1|cosmic-panel|cosmic-panel-config/src/container_config.rs|const:VERSION"
    "com.system76.CosmicPanel.Panel|1|cosmic-panel|cosmic-panel-config/src/container_config.rs|const:VERSION"
    "com.system76.CosmicSettings.Shortcuts|1|cosmic-settings-daemon|config/src/shortcuts/mod.rs|struct:Config"
    "com.system76.CosmicTerm|1|cosmic-term|src/config.rs|const:CONFIG_VERSION"
    "com.system76.CosmicTheme.Dark|2|libcosmic|cosmic-theme/src/model/theme.rs|struct:Theme"
    "com.system76.CosmicTheme.Dark.Builder|2|libcosmic|cosmic-theme/src/model/theme.rs|struct:ThemeBuilder"
    "com.system76.CosmicTheme.Light|2|libcosmic|cosmic-theme/src/model/theme.rs|struct:Theme"
    "com.system76.CosmicTheme.Light.Builder|2|libcosmic|cosmic-theme/src/model/theme.rs|struct:ThemeBuilder"
    "com.system76.CosmicTheme.Mode|1|libcosmic|cosmic-theme/src/model/mode.rs|struct:ThemeMode"
    "com.system76.CosmicTk|1|libcosmic|src/config/mod.rs|struct:CosmicTk"
    # The constellations "Celenn Dark" and "Celenn Light" (build_files/theme,
    # build_files/10-system-files.sh, build_files/99-tests.sh)
    "com.system76.CosmicThemeBundle.celenn-*.Theme|2|libcosmic|cosmic-theme/src/model/theme.rs|struct:Theme"
    "com.system76.CosmicThemeBundle.celenn-*.Builder|2|libcosmic|cosmic-theme/src/model/theme.rs|struct:ThemeBuilder"
    "com.system76.CosmicThemeBundle.celenn-*.Background|1|cosmic-bg|config/src/lib.rs|const:VERSION"
    "com.system76.CosmicThemeBundle.celenn-*.Package|1|cosmic-settings|theme-bundle-config/src/lib.rs|struct:Package"
)

errors=0
error() {
    echo "::error::$*"
    errors=1
}

declare -A versions
for check in "${checks[@]}"; do
    IFS='|' read -r id version _ <<<"${check}"
    versions["${id}"]="${version}"
done

# The defaults Celenn OS ships: each folder in the table, at its version
for dir in system_files/usr/share/cosmic/*/; do
    id="$(basename "${dir}")"
    version="${versions["${id}"]:-}"
    if [[ -z "${version}" ]]; then
        error "${dir}: not in the table of tests/check-cosmic-config.sh"
        continue
    fi
    for sub in "${dir}"*/; do
        [[ "$(basename "${sub}")" == "v${version}" ]] \
            || error "${sub}: COSMIC reads ${id} at v${version}"
    done
done

# The settings the "Change computers" tool reads and writes at version 2
mapfile -t listed < <(sed -n '/^const VERSION_2: /,/^\];/p' build_files/first_steps/src/transfer.rs \
    | grep -o '"com\.system76\.[^"]*"' | tr -d '"' | sort)
mapfile -t expected < <(for id in "${!versions[@]}"; do
    [[ "${versions["${id}"]}" == 2 && "${id}" != *ThemeBundle* ]] && echo "${id}"
done | sort)
[[ "${listed[*]}" == "${expected[*]}" ]] \
    || error "build_files/first_steps/src/transfer.rs: VERSION_2 lists '${listed[*]}', the table '${expected[*]}'"

if (($# == 0)); then
    ((errors == 0)) && echo "COSMIC settings: ${#checks[@]} ids, the repository matches the table"
    exit "${errors}"
fi

# The versions in COSMIC's source for the release given
tag="epoch-${1//\~/-}"
sources="$(mktemp -d)"
trap 'rm -rf "${sources}"' EXIT
fetch() {
    curl -fsSL --retry 3 "https://raw.githubusercontent.com/pop-os/$1/$2/$3"
}
libcosmic="$(fetch cosmic-settings "${tag}" Cargo.lock \
    | sed -n 's|^source = "git+https://github.com/pop-os/libcosmic[^#]*#\([0-9a-f]\{40\}\)"$|\1|p' \
    | sort -u)"
if [[ "$(grep -c . <<<"${libcosmic}")" != 1 ]]; then
    echo "::error::No single libcosmic revision in cosmic-settings ${tag} Cargo.lock"
    exit 1
fi

for check in "${checks[@]}"; do
    IFS='|' read -r id version repository file declaration <<<"${check}"
    ref="${tag}"
    [[ "${repository}" == libcosmic ]] && ref="${libcosmic}"
    copy="${sources}/${repository}/${file}"
    if [[ ! -f "${copy}" ]]; then
        mkdir -p "$(dirname "${copy}")"
        if ! fetch "${repository}" "${ref}" "${file}" >"${copy}"; then
            rm -f "${copy}"
            error "${id}: pop-os/${repository} ${ref} has no ${file}: find where it now declares its version"
            continue
        fi
    fi
    case "${declaration}" in
        struct:*) pattern="#\[version = (\d+)\](?:\s*#\[[^\]]*\])*\s*pub struct ${declaration#struct:}\b" ;;
        const:*) pattern="const ${declaration#const:}: u64 = (\d+);" ;;
        re:*) pattern="${declaration#re:}" ;;
        *) error "${id}: unknown declaration ${declaration}"; continue ;;
    esac
    found="$(PATTERN="${pattern}" perl -0777 -ne 'print $1 if /$ENV{PATTERN}/' "${copy}")"
    if [[ -z "${found}" ]]; then
        error "${id}: no version found in pop-os/${repository} ${ref} ${file} (${declaration})"
    elif [[ "${found}" != "${version}" ]]; then
        error "${id}: COSMIC ${1} moves it to v${found} (Celenn OS: v${version}); adapt system_files, the apps, the ujust recipes and this table"
    else
        echo "${id}: v${found}"
    fi
done
exit "${errors}"
