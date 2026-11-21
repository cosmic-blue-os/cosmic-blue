#!/usr/bin/bash
# Rebuilds Fedora's cosmic-greeter package with the cosmic-blue wallpaper as
# its built-in background, from the same source package and build flags as
# Fedora. Runs in a Fedora container of the same release as the image; the
# binary is written to /out and installed by build_files/23-greeter.sh.
#
# Why: the login screen shows the wallpaper that cosmic-bg last saved for the
# selected user and for each screen (~/.local/state/cosmic/
# com.system76.CosmicBackground). Until a user has opened a session once, and
# on any screen that user never used, cosmic-greeter falls back to the image
# compiled into it (res/background.jpg up to COSMIC 1.11, res/background.png
# after it: the upstream COSMIC wallpaper). The
# lock screen is the same program and uses the same fallback.
#
# user-switch.patch also shows the accounts as a row of avatars under the
# password field when there are 2 to 6 of them (the users menu stays), and
# clears the error of a refused login when another user is selected. It is
# applied with no fuzz: when a new cosmic-greeter no longer takes it, this
# build fails and the COSMIC update pull request waits for the patch to be
# adapted.
# Usage: build.sh <wallpaper.jpg>
set -euox pipefail

WALLPAPER="$1"
TOPDIR=/rpmbuild
SPEC="${TOPDIR}/SPECS/cosmic-greeter.spec"

dnf5 -y install rpm-build dnf5-plugins python3 patch
mkdir -p /srpm "${TOPDIR}"
dnf5 -y download --srpm --destdir /srpm \
    --enablerepo=fedora-source --enablerepo=updates-source \
    cosmic-greeter
dnf5 -y builddep /srpm/cosmic-greeter-*.src.rpm
rpm -i --define "_topdir ${TOPDIR}" /srpm/cosmic-greeter-*.src.rpm

# Unpack the sources (vendored crates included) and apply Fedora's patches
rpmbuild --define "_topdir ${TOPDIR}" -bp "${SPEC}"

src=""
while IFS= read -r manifest; do
    if grep -q '^name = "cosmic-greeter"' "${manifest}"; then
        src="$(dirname "${manifest}")"
        break
    fi
done < <(find "${TOPDIR}/BUILD" -name Cargo.toml -not -path '*/vendor/*')
test -n "${src}"

# The fallback compiled in must be a single res/background.* file, or the
# replacement would do nothing. The JPEG is copied as is, even over a .png:
# the greeter (iced) reads the image format from the content, not the name.
fallback="$(grep -o 'include_bytes!("\.\./res/background\.[a-z]*")' "${src}/src/common.rs" \
    | sed 's|^include_bytes!("\.\./\(.*\)")$|\1|' | sort -u)"
test "$(printf '%s\n' "${fallback}" | grep -c .)" = 1
test -f "${src}/${fallback}"
install -m 0644 "${WALLPAPER}" "${src}/${fallback}"

patch --directory="${src}" --strip=1 --forward --fuzz=0 \
    < "$(dirname "$0")/user-switch.patch"
grep -q 'cosmic-blue: with 2 to 6 accounts' "${src}/src/greeter.rs"

# Compiled crates kept from the previous build in CARGO_CACHE_DIR
# (Containerfile.helpers). The package's own sources are touched, so they
# always compile again even when the unpacked files carry older dates.
if [ -n "${CARGO_CACHE_DIR:-}" ]; then
    mkdir -p "${CARGO_CACHE_DIR}/target"
    rm -rf "${src}/target"
    ln -s "${CARGO_CACHE_DIR}/target" "${src}/target"
    find "${src}" \( -path '*/vendor' -o -path "${src}/target" \) -prune -o -type f -exec touch {} +
fi

# Binaries older than this build (kept in CARGO_CACHE_DIR) are not used
touch /tmp/build-start
# Build only (no new %prep, which would undo the replacement)
rpmbuild --define "_topdir ${TOPDIR}" -bc --short-circuit "${SPEC}"

bin="$(find "${src}/target/" -type f -newer /tmp/build-start -name cosmic-greeter -perm -u+x | head -n1)"
test -n "${bin}"
# The wallpaper really is inside the binary
python3 - "${bin}" "${WALLPAPER}" <<'PY'
import sys
binary = open(sys.argv[1], "rb").read()
image = open(sys.argv[2], "rb").read()
if image not in binary:
    sys.exit("the cosmic-blue wallpaper is not embedded in cosmic-greeter")
PY
mkdir -p /out
# Without debug data: Fedora's Rust build flags keep it (-Cdebuginfo=2), and
# only its %install step, which is not run here, strips it (about 0.1 GB per
# program in the image and the ISO)
install -m 0755 -s "${bin}" /out/cosmic-greeter
# The version the binary was built from, checked against the image's package
rpm -qp --qf '%{VERSION}-%{RELEASE}\n' /srpm/cosmic-greeter-*.src.rpm > /out/version
cat /out/version
