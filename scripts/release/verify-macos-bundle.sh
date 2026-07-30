#!/bin/sh
set -eu

binary="${1:?Pass the universal app binary path}"
architectures="$(lipo -archs "$binary")"
case " $architectures " in
  *" arm64 "*) ;;
  *) echo "Missing arm64 slice" >&2; exit 1 ;;
esac
case " $architectures " in
  *" x86_64 "*) ;;
  *) echo "Missing x86_64 slice" >&2; exit 1 ;;
esac

min_versions="$(otool -l "$binary" | awk '/minos/{print $2}')"
test -n "$min_versions"
if printf '%s\n' "$min_versions" | grep -Ev '^15(\.0){0,2}$' >/dev/null; then
  echo "Unexpected macOS minimum version:" >&2
  printf '%s\n' "$min_versions" >&2
  exit 1
fi
echo "Universal slices and macOS 15.0 minimum verified: $architectures"
