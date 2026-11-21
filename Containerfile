# cosmic-blue: an independent Fedora Atomic image with the COSMIC desktop, built
# on Universal Blue base images (not affiliated with Universal Blue or Bluefin).

# Fedora release: the one of Bluefin's stable tag. check-cosmic-version.yml
# opens the pull request that moves BASE_TAG and BASE_IMAGE together when
# Bluefin stable moves to a new release (in 2026-11, Fedora 45 still broke the
# live ISO installer: systemd-tmpfiles exited with code 73).
# The base image is followed by tag: every daily build takes the latest
# base-main (Fedora package updates). The Bluefin common and brew images are
# pinned by digest: they bring scripts, services and ujust recipes that
# change without notice (a common change broke every build on 2026-11-16), so
# Renovate moves them once a week in a pull request that must build and pass
# the boot test (see .github/renovate.json5).
# BASE_TAG is the Fedora release of Containerfile.helpers (`just fedora-version`).
ARG BASE_TAG="45"
ARG BASE_IMAGE="ghcr.io/ublue-os/base-main:45"
ARG COMMON_IMAGE="ghcr.io/projectbluefin/common:latest@sha256:c2799fd6253575b9efb22bf798af335e822d8ad22f798f09c75fcdffcb37c3a8"
ARG BREW_IMAGE="ghcr.io/ublue-os/brew:latest@sha256:9e0492dbc471258657564c94dae18b58dd96ddb998a06f2b15e8209efb48042b"

# Bluefin shared files (ujust, uupd, setup, signing, udev...) and Homebrew
FROM ${COMMON_IMAGE} AS common
FROM ${BREW_IMAGE} AS brew

# Build context: scripts + system files, never copied as-is into the final image
FROM scratch AS ctx
COPY build_files /build_files
COPY --from=common /system_files/shared /system_files/common
COPY --from=common /system_files/bluefin /system_files/common-bluefin
COPY --from=brew /system_files /system_files/brew
# Our own files, applied last so they take precedence
COPY system_files /system_files/cosmic-blue
# Outputs of Containerfile.helpers (build context "helpers", see `just build`)
COPY --from=helpers /theme/cosmic /system_files/cosmic-blue/usr/share/cosmic
COPY --from=helpers /theme/previous /previous_defaults/theme
COPY --from=helpers /tour/cosmic-blue-tour /system_files/cosmic-blue/usr/libexec/cosmic-blue-tour
COPY --from=helpers /tour/cosmic-blue-tools /system_files/cosmic-blue/usr/libexec/cosmic-blue-tools
COPY --from=helpers /tour/cosmic-blue-parental-controls /system_files/cosmic-blue/usr/libexec/cosmic-blue-parental-controls
COPY --from=helpers /tour/cosmic-blue-parental-daemon /system_files/cosmic-blue/usr/libexec/cosmic-blue-parental-daemon
COPY --from=helpers /greeter /greeter
COPY --from=helpers /store /store
COPY --from=helpers /filemanager /filemanager
COPY --from=helpers /plymouth /system_files/cosmic-blue/usr/share/plymouth/themes/cosmic-blue
COPY --from=helpers /icons /icons
COPY --from=helpers /cursors /system_files/cosmic-blue/usr/share/icons
# Public key used to verify the image signature on update
COPY cosign.pub /system_files/cosmic-blue/etc/pki/containers/cosmic-blue.pub

FROM ${BASE_IMAGE}

ARG IMAGE_NAME="cosmic-blue"
ARG IMAGE_VENDOR="cosmic-blue-os"
ARG UBLUE_IMAGE_TAG="latest"
ARG VERSION=""

RUN --mount=type=bind,from=ctx,source=/,target=/ctx \
    --mount=type=cache,dst=/var/cache \
    --mount=type=cache,dst=/var/log \
    --mount=type=tmpfs,dst=/tmp \
    IMAGE_NAME="${IMAGE_NAME}" IMAGE_VENDOR="${IMAGE_VENDOR}" \
    UBLUE_IMAGE_TAG="${UBLUE_IMAGE_TAG}" VERSION="${VERSION}" \
    /ctx/build_files/build.sh

RUN bootc container lint
