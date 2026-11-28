#!/usr/bin/bash
# Image identity: image-info.json (used by ujust, uupd...) and os-release.
set -euox pipefail

IMAGE_PRETTY_NAME="Celenn OS"
# One repository for all the images (celenn-nvidia included)
HOME_URL="https://github.com/${IMAGE_VENDOR}/celenn"
VERSION="${VERSION:-${FEDORA_MAJOR_VERSION}.$(date +%Y%m%d)}"

# The image is verified with cosign.pub (see 10-system-files.sh); without it,
# bootc cannot verify the signature and the image is referenced as "unverified".
if [[ -f /etc/pki/containers/celenn.pub ]]; then
    IMAGE_REF="ostree-image-signed:docker://ghcr.io/${IMAGE_VENDOR}/${IMAGE_NAME}"
else
    IMAGE_REF="ostree-unverified-registry:ghcr.io/${IMAGE_VENDOR}/${IMAGE_NAME}"
fi

# "nvidia-open" for the NVIDIA image, as in Bluefin (read by ujust recipes)
IMAGE_FLAVOR="main"
if [[ "${IMAGE_NAME}" == *-nvidia ]]; then
    IMAGE_FLAVOR="nvidia-open"
fi

mkdir -p /usr/share/ublue-os
cat > /usr/share/ublue-os/image-info.json <<JSON
{
  "image-name": "${IMAGE_NAME}",
  "image-flavor": "${IMAGE_FLAVOR}",
  "image-vendor": "${IMAGE_VENDOR}",
  "image-ref": "${IMAGE_REF}",
  "image-tag": "${UBLUE_IMAGE_TAG}",
  "base-image-name": "base",
  "fedora-version": "${FEDORA_MAJOR_VERSION}"
}
JSON

# Keep ID=fedora (avoids EFI/GRUB path issues); only change the displayed name.
sed -i "s|^NAME=.*|NAME=\"${IMAGE_PRETTY_NAME}\"|" /usr/lib/os-release
sed -i "s|^PRETTY_NAME=.*|PRETTY_NAME=\"${IMAGE_PRETTY_NAME} ${VERSION}\"|" /usr/lib/os-release
# Fedora's base os-release has no VARIANT_ID line to replace: add it. The
# live ISO's installer finds its Celenn OS profile with it.
sed -i "/^VARIANT_ID=/d" /usr/lib/os-release
echo "VARIANT_ID=${IMAGE_NAME}" >> /usr/lib/os-release
sed -i "s|^HOME_URL=.*|HOME_URL=\"${HOME_URL}\"|" /usr/lib/os-release
sed -i "s|^SUPPORT_URL=.*|SUPPORT_URL=\"${HOME_URL}/issues\"|" /usr/lib/os-release
sed -i "s|^BUG_REPORT_URL=.*|BUG_REPORT_URL=\"${HOME_URL}/issues\"|" /usr/lib/os-release
# Logo shown by COSMIC Settings (About) and fetch tools: an icon name
sed -i "/^LOGO=/d" /usr/lib/os-release
echo "LOGO=celenn-logo" >> /usr/lib/os-release
sed -i "/^IMAGE_ID=/d; /^IMAGE_VERSION=/d" /usr/lib/os-release
echo "IMAGE_ID=\"${IMAGE_NAME}\"" >> /usr/lib/os-release
echo "IMAGE_VERSION=\"${VERSION}\"" >> /usr/lib/os-release
