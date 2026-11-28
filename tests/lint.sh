#!/usr/bin/bash
# Static checks, run on pull requests (.github/workflows/lint.yml) and by
# `just lint`: shellcheck on every shell script of the repository and the
# image (warnings and errors only), rustfmt on the Rust programs, the apps'
# translatable texts (tests/check-translations.py), the developer tools
# lists (tests/check-dev-tools.py), the COSMIC settings versions
# (tests/check-cosmic-config.sh), the root helpers (tests/check-helpers.sh),
# actionlint on the workflows when it is installed (the workflow runs it
# separately).
set -euo pipefail
cd "$(dirname "$0")/.."

mapfile -t scripts < <(
    git ls-files '*.sh'
    # Scripts without the .sh extension, found by their #! line
    git ls-files -z system_files | while IFS= read -r -d '' file; do
        [[ -f "${file}" && ! -L "${file}" ]] || continue
        head -n 1 "${file}" | grep -qE '^#!.*\b(ba)?sh\b' && echo "${file}"
    done
)
echo "shellcheck: ${#scripts[@]} scripts"
shellcheck --severity=warning "${scripts[@]}"

# Rust: the crate roots (rustfmt follows their modules); no build needed
mapfile -t crates < <(git ls-files 'build_files/*/Cargo.toml')
roots=()
for manifest in "${crates[@]}"; do
    dir="${manifest%/Cargo.toml}"
    while IFS= read -r root; do
        roots+=("${dir}/${root}")
    done < <(sed -n 's/^path = "\(.*\.rs\)"$/\1/p' "${manifest}")
    if [[ -f "${dir}/src/main.rs" ]]; then
        roots+=("${dir}/src/main.rs")
    fi
done
mapfile -t roots < <(printf '%s\n' "${roots[@]}" | sort -u)
echo "rustfmt: ${#roots[@]} crate roots"
rustfmt --edition 2024 --check "${roots[@]}"

# Every tr("...") of the apps is in its program's translation template
tests/check-translations.py

# ujust install-dev-tools offers the same list as the tour and Tools
tests/check-dev-tools.py

# The COSMIC settings Celenn OS ships and changes are in the table of
# versions that check-cosmic-version.yml compares with each new COSMIC
tests/check-cosmic-config.sh

# The helpers that run as root through pkexec refuse anything but the
# changes they document (stubs, nothing touches the machine)
tests/check-helpers.sh

if command -v actionlint >/dev/null; then
    SHELLCHECK_OPTS="--severity=warning" actionlint
fi
