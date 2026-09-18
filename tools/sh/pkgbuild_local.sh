#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/../.." && pwd)"

BUILD_DIR="$ROOT_DIR/build"
ARTIFACTS_DIR="$BUILD_DIR/artifacts"
DIST_DIR="$BUILD_DIR/dist"
CARGO_TARGET_DIR="$BUILD_DIR/cargo-target"
PKGBUILD_DIR="$ROOT_DIR/packaging/arch/local"
PKGBUILD="$PKGBUILD_DIR/PKGBUILD"

read -r pkgname pkgver <<<"$(bash -c 'source "$1"; printf "%s %s" "$pkgname" "$pkgver"' bash "$PKGBUILD")"

mkdir -p "$ARTIFACTS_DIR" "$CARGO_TARGET_DIR" "$DIST_DIR"
find "$DIST_DIR" -maxdepth 1 -type f -name "${pkgname}-*.pkg.tar.*" -delete

archive="$ARTIFACTS_DIR/${pkgname}-${pkgver}.tar.gz"
staging_dir="$(mktemp -d)"
trap 'rm -rf -- "$staging_dir"' EXIT

mkdir -p "$staging_dir/${pkgname}-${pkgver}" "$staging_dir/argvus-i18n"

tar -cf - \
  --exclude='./.git' \
  --exclude='./.release' \
  --exclude='./packages-repo' \
  --exclude='./build' \
  --exclude='*/build' \
  --exclude='./dist' \
  --exclude='*/dist' \
  --exclude='./target' \
  --exclude='*/target' \
  --exclude='./tools' \
  --exclude='./packaging/arch/ci/src' \
  --exclude='./packaging/arch/ci/pkg' \
  --exclude='./packaging/arch/local/src' \
  --exclude='./packaging/arch/local/pkg' \
  --exclude='./packaging/arch/local/*.pkg.tar*' \
  --exclude='./packaging/arch/local/*.tar.gz' \
  -C "$ROOT_DIR" . \
  | tar -xf - -C "$staging_dir/${pkgname}-${pkgver}"

i18n_root="$ROOT_DIR/../argvus-i18n"
if [[ ! -f "$i18n_root/Cargo.toml" ]]; then
  echo "argvus-i18n checkout not found beside argvus-control-center: $i18n_root" >&2
  exit 1
fi

tar -cf - \
  --exclude='./.git' \
  --exclude='./build' \
  --exclude='*/build' \
  --exclude='./dist' \
  --exclude='*/dist' \
  --exclude='./target' \
  --exclude='*/target' \
  -C "$i18n_root" . \
  | tar -xf - -C "$staging_dir/argvus-i18n"

echo "Creating local source archive: $archive"
tar -czf "$archive" -C "$staging_dir" "${pkgname}-${pkgver}" argvus-i18n

cd "$PKGBUILD_DIR"
export BUILDDIR="$ARTIFACTS_DIR"
export SRCDEST="$ARTIFACTS_DIR"
export PKGDEST="$DIST_DIR"
export CARGO_TARGET_DIR

makepkg -p PKGBUILD --nodeps --noconfirm --needed --cleanbuild --clean --force "$@"

printf 'Packages created in %s:\n' "$DIST_DIR"
find "$DIST_DIR" -maxdepth 1 -type f -name "${pkgname}-*.pkg.tar.zst" -printf '  %f\n' | sort
