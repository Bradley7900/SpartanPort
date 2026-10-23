#!/usr/bin/env bash
# Starts the AppImage's launcher on a virtual display and checks it is still running after a few
# seconds: a missing glibc version or library stops it at start (#291). Run as root in a fresh
# container of the distro to check; it installs Xvfb and a font, as a desktop has them
# (.github/workflows/linux-appimage.yml).
# Usage: smoke-test.sh path/to/spartanport-x86_64.AppImage
set -euo pipefail
appimage=$(readlink -f -- "$1")
if command -v apt-get >/dev/null; then
    apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq xvfb fonts-dejavu-core >/dev/null
elif command -v dnf >/dev/null; then
    dnf install -y -q xorg-x11-server-Xvfb dejavu-sans-fonts >/dev/null
elif command -v zypper >/dev/null; then
    zypper -nq install xorg-x11-server-Xvfb dejavu-fonts >/dev/null
elif command -v pacman >/dev/null; then
    pacman -Sy --noconfirm --needed xorg-server-xvfb ttf-dejavu >/dev/null
fi
grep -m1 PRETTY_NAME /etc/os-release
ldd --version | sed -n 1p  # head would end ldd with SIGPIPE (pipefail)
work=$(mktemp -d)
cd "$work"
"$appimage" --appimage-extract >/dev/null
if [[ -n ${SPARTAN_RELEASE_TAG:-} ]]; then
    if ! grep -q '"version"' squashfs-root/release.json 2>/dev/null; then
        echo "FAIL: release.json is missing or invalid in the AppImage (tag $SPARTAN_RELEASE_TAG)" >&2
        exit 1
    fi
    echo "release.json present (tag $SPARTAN_RELEASE_TAG)"
fi
Xvfb :99 -screen 0 1280x800x24 >/dev/null 2>&1 &
export DISPLAY=:99 HOME=$work
sleep 2
version=$(squashfs-root/AppRun --version)
echo "$version"
grep -qE '^SpartanPort [0-9]+\.[0-9]+\.[0-9]+$' <<<"$version" || { echo 'FAIL: --version printed no version line' >&2; exit 1; }
status=0
timeout 15 squashfs-root/AppRun > launcher.log 2>&1 || status=$?
cat launcher.log
if [[ $status != 124 ]] || grep -qE 'panicked|GLIBC_|error while loading shared libraries' launcher.log; then
    echo "FAIL: the launcher exited ($status) instead of staying open" >&2
    exit 1
fi
echo "OK: the launcher stayed open"
