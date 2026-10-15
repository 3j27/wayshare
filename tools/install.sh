#!/usr/bin/env bash
# Installs a release build: tools/install.sh [--uninstall]
#
#   PREFIX                 default /usr/local (packages use /usr)
#   DESTDIR                staging root for packaging; never written into the unit
#   SYSTEMD_USER_UNIT_DIR  default $PREFIX/lib/systemd/user
set -euo pipefail

PREFIX=${PREFIX:-/usr/local}
DESTDIR=${DESTDIR:-}
BINDIR=${BINDIR:-$PREFIX/bin}
UNITDIR=${SYSTEMD_USER_UNIT_DIR:-$PREFIX/lib/systemd/user}
DOCDIR=${DOCDIR:-$PREFIX/share/doc/wayshare}

if [ -z "$DESTDIR" ] && [ "$(id -u)" -ne 0 ] && ! [ -w "$PREFIX" ]; then
  exec sudo env PREFIX="$PREFIX" BINDIR="$BINDIR" SYSTEMD_USER_UNIT_DIR="$UNITDIR" DOCDIR="$DOCDIR" "$0" "$@"
fi

files=("$DESTDIR$BINDIR/wayshare" "$DESTDIR$UNITDIR/wayshare.service" "$DESTDIR$DOCDIR/config.example.toml")

if [ "${1:-}" = --uninstall ]; then
  rm -fv "${files[@]}"
  rmdir "$DESTDIR$DOCDIR" 2>/dev/null || true
  exit
fi

cd "$(dirname "$0")/.."
[ -x target/release/wayshare ] || { echo "no release build; run: cargo build --release" >&2; exit 1; }

unit=$(mktemp)
trap 'rm -f "$unit"' EXIT
sed "s|@bindir@|$BINDIR|g" contrib/wayshare.service.in > "$unit"

install -Dm755 target/release/wayshare "${files[0]}"
install -Dm644 "$unit" "${files[1]}"
install -Dm644 contrib/config.example.toml "${files[2]}"
printf '%s\n' "${files[@]}"
