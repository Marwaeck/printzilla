#!/bin/bash
# Doppelklick: veröffentlicht eine neue PrintZilla-Version. GitHub baut daraus
# die Windows- und Mac-Downloads, und die Update-Funktion bietet sie allen an.
set -e
cd "$(dirname "$0")"

CURRENT=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
IFS=. read -r MA MI PA <<< "$CURRENT"
SUGGEST="$MA.$MI.$((PA + 1))"

echo "Aktuelle Version: $CURRENT"
read -r -p "Neue Versionsnummer [$SUGGEST]: " NEW
NEW=${NEW:-$SUGGEST}
read -r -p "Was ist neu? (eine Zeile, optional): " NOTES

perl -0pi -e "s/^version = \"[^\"]+\"/version = \"$NEW\"/m" Cargo.toml
cargo generate-lockfile >/dev/null 2>&1 || true
git add -A
git commit -m "Version $NEW${NOTES:+: $NOTES}"
git tag -a "v$NEW" -m "${NOTES:-Version $NEW}"
git push origin main
git push origin "v$NEW"

echo
echo "Version $NEW ist unterwegs. GitHub baut sie jetzt (etwa 10 Minuten)."
open "$(git remote get-url origin | sed 's/\.git$//')/actions" || true
read -r -p "Enter zum Schließen"
