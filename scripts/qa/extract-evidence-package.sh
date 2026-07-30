#!/bin/sh
set -eu

archive="${1:?Pass release-evidence-<version>.tar.gz}"
destination="${2:?Pass an empty extraction directory}"

test -f "$archive"
test -d "$destination"
test -z "$(find "$destination" -mindepth 1 -maxdepth 1 -print -quit)"

tar -tzf "$archive" | while IFS= read -r entry; do
  case "$entry" in
    "" | /* | ../* | */../* | *"/.." | *\\*)
      echo "Unsafe evidence archive path: $entry" >&2
      exit 1
      ;;
  esac
done

if tar -tvzf "$archive" | grep -Eq '^[lh]'; then
  echo "Evidence package must not contain links" >&2
  exit 1
fi

tar -xzf "$archive" -C "$destination" --no-same-owner --no-same-permissions
test -f "$destination/release-evidence.json"
echo "Extracted validated evidence package to $destination"
