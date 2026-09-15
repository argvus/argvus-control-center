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
staging_dir="$(mktemp -d)"
trap 'rm -rf -- "$staging_dir"' EXIT

mkdir -p "$staging_dir/${pkgname}-${pkgver}" "$staging_dir/argvus-i18n"

tar -cf - \
  --exclude='./.git' \
  --exclude='./dist' \
  --exclude='./target' \
  --exclude='./pkg' \
  --exclude='./packaging/arch/pkg' \
  --exclude='./packaging/arch/src' \
  --exclude='./packaging/arch/*.pkg.tar*' \
  --exclude='./packaging/arch/*.tar.gz' \
  -C "$ROOT_DIR" . \
  | tar -xf - -C "$staging_dir/${pkgname}-${pkgver}"

i18n_root="$ROOT_DIR/../argvus-i18n"
if [[ ! -f "$i18n_root/Cargo.toml" ]]; then
  echo "argvus-i18n checkout not found beside argvus-control-center: $i18n_root" >&2
  exit 1
fi

tar -cf - \
  --exclude='./.git' \
  --exclude='./target' \
  --exclude='./dist' \
  --exclude='./packaging/arch/pkg' \
  --exclude='./packaging/arch/src' \
  --exclude='./packaging/arch/*.pkg.tar*' \
  --exclude='./packaging/arch/*.tar.gz' \
  -C "$i18n_root" . \
  | tar -xf - -C "$staging_dir/argvus-i18n"

echo "Creating local source archive: $archive"
tar -czf "$archive" -C "$staging_dir" "${pkgname}-${pkgver}" argvus-i18n

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
