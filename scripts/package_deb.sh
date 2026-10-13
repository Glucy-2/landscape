#!/usr/bin/env bash
# Build landscape-router_<version>_<arch>.deb from an already-built
# landscape-webserver binary and web UI.
#
#   scripts/package_deb.sh [--arch ARCH] [--bin PATH] [--static DIR|ZIP]
#                          [--version VER] [--depends DEPS] [--out DIR]
#
# ARCH is a Rust or Debian arch name (x86_64/amd64, aarch64/arm64, ...),
# default: host. Defaults follow build.sh output: output/landscape-webserver-<arch>,
# output/static. Dependencies are computed with dpkg-shlibdeps unless the
# binary is static or --depends is given.
#
# Installed layout (see landscape-common/src/paths.rs):
#   /usr/bin/landscape-webserver
#   /usr/share/landscape-router/static/
#   /usr/lib/systemd/system/landscape-router.service

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PKG_NAME=landscape-router

ARCH="$(uname -m)"
BIN=""
STATIC=""
VERSION=""
DEPENDS=""
DEPENDS_SET=0
OUT_DIR="$ROOT_DIR/output"

die() {
    echo "package_deb: $*" >&2
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --arch) ARCH="$2"; shift 2 ;;
        --bin) BIN="$2"; shift 2 ;;
        --static) STATIC="$2"; shift 2 ;;
        --version) VERSION="$2"; shift 2 ;;
        --depends) DEPENDS="$2"; DEPENDS_SET=1; shift 2 ;;
        --out) OUT_DIR="$2"; shift 2 ;;
        -h | --help) sed -n '2,16p' "$0"; exit 0 ;;
        *) die "unknown argument: $1" ;;
    esac
done

case "$ARCH" in
    x86_64 | amd64) RUST_ARCH=x86_64; DEB_ARCH=amd64 ;;
    aarch64 | arm64) RUST_ARCH=aarch64; DEB_ARCH=arm64 ;;
    armv7 | armhf) RUST_ARCH=armv7; DEB_ARCH=armhf ;;
    riscv64) RUST_ARCH=riscv64; DEB_ARCH=riscv64 ;;
    loongarch64 | loong64) RUST_ARCH=loongarch64; DEB_ARCH=loong64 ;;
    s390x) RUST_ARCH=s390x; DEB_ARCH=s390x ;;
    *) die "unsupported arch: $ARCH" ;;
esac

BIN="${BIN:-$ROOT_DIR/output/landscape-webserver-$RUST_ARCH}"
STATIC="${STATIC:-$ROOT_DIR/output/static}"
[[ -f "$BIN" ]] || die "binary not found: $BIN"
[[ -e "$STATIC" ]] || die "web UI not found: $STATIC"

if [[ -z "$VERSION" ]]; then
    VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT_DIR/Cargo.toml" | head -n1)"
    [[ -n "$VERSION" ]] || die "cannot read workspace version from Cargo.toml"
fi
VERSION="${VERSION#v}"
# 0.27.0-beta.1 -> 0.27.0~beta.1 so pre-releases sort before the release.
VERSION="${VERSION//-/\~}"
[[ "$VERSION" =~ ^[0-9][A-Za-z0-9.+~]*$ ]] || die "invalid package version: $VERSION"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
# debian/<package>/ with a DEBIAN/ dir is how dpkg-shlibdeps finds the
# package root of the binary it analyzes.
PKG="$WORK/debian/$PKG_NAME"
mkdir -p "$PKG/DEBIAN"

install -Dm755 "$BIN" "$PKG/usr/bin/landscape-webserver"
install -Dm644 "$ROOT_DIR/packaging/landscape-router.service" \
    "$PKG/usr/lib/systemd/system/landscape-router.service"
install -Dm644 "$ROOT_DIR/packaging/deb/copyright" "$PKG/usr/share/doc/$PKG_NAME/copyright"

WEB_ROOT="$PKG/usr/share/landscape-router/static"
mkdir -p "$WEB_ROOT"
if [[ -d "$STATIC" ]]; then
    cp -r "$STATIC/." "$WEB_ROOT/"
else
    # Release static.zip wraps everything in a top-level static/ directory.
    unzip -q "$STATIC" -d "$WORK/static"
    SRC="$WORK/static"
    [[ -d "$SRC/static" ]] && SRC="$SRC/static"
    cp -r "$SRC/." "$WEB_ROOT/"
fi
[[ -f "$WEB_ROOT/index.html" ]] || die "web UI has no index.html: $STATIC"
find "$WEB_ROOT" -type d -exec chmod 755 {} +
find "$WEB_ROOT" -type f -exec chmod 644 {} +

if [[ "$DEPENDS_SET" -eq 0 ]]; then
    # Not `readelf | grep -q`: an early grep exit can SIGPIPE readelf and,
    # under pipefail, misreport a dynamic binary as static.
    DYNAMIC_SECTION="$(readelf -d "$BIN")"
    if grep -q NEEDED <<<"$DYNAMIC_SECTION"; then
        # dpkg-shlibdeps needs debian/control and the target's libraries
        # installed, i.e. it runs on a host of the package's architecture.
        printf 'Source: %s\n\nPackage: %s\nArchitecture: any\n' "$PKG_NAME" "$PKG_NAME" \
            > "$WORK/debian/control"
        SHLIBS="$(cd "$WORK" && dpkg-shlibdeps -O "debian/$PKG_NAME/usr/bin/landscape-webserver")" \
            || die "dpkg-shlibdeps failed; build on a $DEB_ARCH host or pass --depends"
        DEPENDS="${SHLIBS#shlibs:Depends=}"
    fi
fi
DEPENDS="${DEPENDS:+$DEPENDS, }iproute2"

INSTALLED_SIZE="$(du -sk --apparent-size --exclude=DEBIAN "$PKG" | cut -f1)"

install -m755 "$ROOT_DIR/packaging/deb/postinst" "$ROOT_DIR/packaging/deb/prerm" \
    "$ROOT_DIR/packaging/deb/postrm" "$PKG/DEBIAN/"
cat > "$PKG/DEBIAN/control" <<EOF
Package: $PKG_NAME
Version: $VERSION
Architecture: $DEB_ARCH
Maintainer: Landscape contributors <https://github.com/ThisSeanZhang/landscape/issues>
Installed-Size: $INSTALLED_SIZE
Depends: $DEPENDS
Suggests: ppp, hostapd, iw, docker.io
Section: net
Priority: optional
Homepage: https://github.com/ThisSeanZhang/landscape
Description: eBPF-based Linux router with a web management UI
 Landscape turns a Linux host into a router: NAT, firewall, DHCP, PPPoE,
 per-flow DNS and traffic steering run on an eBPF dataplane, managed from a
 web UI on port 6443. Requires Linux >= 6.9 with BTF.
 .
 Configuration lives in /etc/landscape-router, state in
 /var/lib/landscape-router and logs in /var/log/landscape-router.
EOF

mkdir -p "$OUT_DIR"
DEB="$OUT_DIR/${PKG_NAME}_${VERSION}_${DEB_ARCH}.deb"
dpkg-deb --build --root-owner-group -Zxz "$PKG" "$DEB" >/dev/null
echo "$DEB"
