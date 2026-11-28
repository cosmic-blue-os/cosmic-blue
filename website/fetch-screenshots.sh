#!/usr/bin/env bash
# Downloads the desktop screenshots of the stable image for the website.
# The boot test of build.yml (tests/vm-boot-test.sh) logs a test user in and
# takes them; each published testing build keeps them for 90 days in the
# artifact site-screenshots-<version>. stable is one of these builds,
# promoted later (promote-stable.yml), so its version names its screenshots.
#
# Fails when the screenshots of the stable image cannot be downloaded, so
# the site is never built with the screenshots of another image.
#
# Usage: website/fetch-screenshots.sh <output directory>
# Writes the screenshots (PNG) and info.env (date, version, commit, run URL).
# Needs gh with GH_TOKEN (actions: read), GITHUB_REPOSITORY and skopeo.
set -uo pipefail

out="$1"
repo="${GITHUB_REPOSITORY:-celenn-os/celenn}"
image="docker://ghcr.io/${repo,,}:stable"
tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT
mkdir -p "${out}"

fail() { echo "::error file=website/fetch-screenshots.sh::Screenshots: $*"; exit 1; }

labels="$(skopeo inspect --retry-times 3 "${image}")" || fail "could not read ${image}"
# The boot test artifact is named after the image (boot-test-celenn)
image_name="${image##*/}"
image_name="${image_name%%:*}"
version="$(jq -r '.Labels["org.opencontainers.image.version"] // empty' <<< "${labels}")"
[[ -n "${version}" ]] || fail "${image} has no version label"
echo "stable is ${version}"

artifact="site-screenshots-${version}"
found="$(gh api "repos/${repo}/actions/artifacts?name=${artifact}&per_page=10" \
    --jq '[.artifacts[] | select(.expired | not)] | sort_by(.created_at) | last
          | select(. != null) | "\(.workflow_run.id) \(.created_at)"')" \
    || fail "could not list the ${artifact} artifacts"
read -r run created <<< "${found}"
if [[ -n "${run}" ]]; then
    gh run download "${run}" -R "${repo}" -n "${artifact}" -D "${tmp}" >/dev/null \
        || fail "could not download ${artifact} from run ${run}"
    shots="${tmp}"
else
    # A stable built before the builds kept site-screenshots-<version>: the
    # boot test results (7 days) of a published build of the same commit
    revision="$(jq -r '.Labels["org.opencontainers.image.revision"] // empty' <<< "${labels}")"
    [[ -n "${revision}" ]] || fail "no ${artifact} artifact, and ${image} has no revision label"
    echo "::warning::No ${artifact} artifact: using the boot test of a build of ${revision:0:7}"
    while read -r id when; do
        [[ -n "${id}" ]] || continue
        pushed="$(gh api "repos/${repo}/actions/runs/${id}/jobs?per_page=100" \
            --jq '.jobs[] | select(.name == "Build and push image") | .steps[] | select(.name == "Push the version tag") | .conclusion')"
        [[ "${pushed}" == success ]] || continue
        gh run download "${id}" -R "${repo}" -n "boot-test-${image_name}" -D "${tmp}" >/dev/null 2>&1 || continue
        run="${id}" created="${when}"
        break
    done < <(gh api "repos/${repo}/actions/workflows/build.yml/runs?head_sha=${revision}&status=completed&per_page=20" \
        --jq '.workflow_runs[] | select(.event != "pull_request") | "\(.id) \(.created_at)"')
    [[ -n "${run}" ]] || fail "no screenshots of stable (${version}, ${revision:0:7}): promote a newer testing build"
    shots="${tmp}/screenshots"
fi
sha="$(gh api "repos/${repo}/actions/runs/${run}" --jq .head_sha)" || fail "could not read run ${run}"
compgen -G "${shots}/*-desktop.png" >/dev/null || fail "the screenshots of ${version} have no desktop one"
cp "${shots}"/*.png "${out}/"
printf 'SHOTS_DATE=%s\nSHOTS_VERSION=%s\nSHOTS_COMMIT=%s\nSHOTS_RUN=%s\n' \
    "${created%%T*}" "${version}" "${sha:0:7}" "https://github.com/${repo}/actions/runs/${run}" > "${out}/info.env"
echo "Screenshots of ${version} (${sha:0:7}) from run ${run}:"
ls "${out}"
