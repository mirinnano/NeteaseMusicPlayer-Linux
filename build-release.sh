#!/usr/bin/env bash
set -euo pipefail

# Build script for netease-cloud-music-gtk4
# Produces a relocatable tarball at dist/netease-cloud-music-gtk4-x86_64.tar.gz

cd "$(dirname "$0")"

BUILD_DIR="$(pwd)/build"
DIST_DIR="$(pwd)/dist"
APP_NAME="netease-cloud-music-gtk4"
DESTDIR="${DIST_DIR}/install"

rm -rf "${DIST_DIR}"
mkdir -p "${DESTDIR}" "${DIST_DIR}"

# 1. meson setup
meson setup "${BUILD_DIR}" \
    --buildtype=release \
    --prefix=/usr/local \
    -Dlibav=false

# 2. ninja build
ninja -C "${BUILD_DIR}"

# 3. install to DESTDIR
DESTDIR="${DESTDIR}" ninja -C "${BUILD_DIR}" install

# 4. strip binary
strip "${DESTDIR}/usr/local/bin/${APP_NAME}"

# 5. create tarball
cd "${DESTDIR}"
tar czf "${DIST_DIR}/${APP_NAME}-x86_64.tar.gz" \
    usr/local/bin/${APP_NAME} \
    usr/local/share/${APP_NAME} \
    usr/local/share/applications \
    usr/local/share/icons \
    usr/local/share/locale \
    usr/local/share/glib-2.0/schemas

echo ""
echo "✅ Build complete: ${DIST_DIR}/${APP_NAME}-x86_64.tar.gz"
tar tzf "${DIST_DIR}/${APP_NAME}-x86_64.tar.gz" | head -20
