#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
PACKAGING_DIR="$ROOT_DIR/packaging/arch"
BUILD_SCRIPT="$PACKAGING_DIR/PKGBUILD.local"

if [[ ! -f "$BUILD_SCRIPT" ]]; then
  echo "PKGBUILD.local not found under packaging/arch." >&2
  exit 1
fi

metadata="$({ cd "$PACKAGING_DIR" && bash -c 'source "$1"; printf "%s\n%s\n" "$pkgname" "$pkgver"' bash "$BUILD_SCRIPT"; })"
pkgname="$(printf '%s\n' "$metadata" | sed -n '1p')"
pkgver="$(printf '%s\n' "$metadata" | sed -n '2p')"
archive="$PACKAGING_DIR/${pkgname}-${pkgver}.tar.gz"

echo "Creating local source archive: $archive"
tar -czf "$archive" \
  --exclude='./.git' \
  --exclude='./dist' \
  --exclude='./target' \
  --exclude='./pkg' \
  --exclude='./src' \
  --exclude='./packaging/arch/pkg' \
  --exclude='./packaging/arch/src' \
  --exclude='./packaging/arch/*.pkg.tar*' \
  --exclude='./packaging/arch/*.tar.gz' \
  --transform "s#^\./#${pkgname}-${pkgver}/#" \
  -C "$ROOT_DIR" .

cd "$PACKAGING_DIR"
makepkg -p PKGBUILD.local --nodeps --noconfirm --needed --cleanbuild --clean --force "$@"

packages="$(find "$PACKAGING_DIR" -maxdepth 1 -type f -name "${pkgname}-*.pkg.tar.zst" -print | sort)"
if [[ -n "$packages" ]]; then
  DIST_DIR="$ROOT_DIR/dist"
  mkdir -p "$DIST_DIR"
  mv -f $packages "$DIST_DIR/"
  printf 'Packages created in %s:\n' "$DIST_DIR"
  printf '%s\n' "$packages" | xargs -I{} basename {}
fi
