#!/bin/bash
# Doppelklick: veröffentlicht eine PrintZilla-Version auf GitHub. GitHub baut daraus
# die Windows- und Mac-Downloads, und die Update-Funktion bietet sie allen an.
set -e
cd "$(dirname "$0")"

echo "== PrintZilla-Version veröffentlichen =="
git fetch origin --tags --quiet

CURRENT=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
if git ls-remote --exit-code --tags origin "refs/tags/v$CURRENT" >/dev/null 2>&1; then
  # this version is already online: suggest the next one
  IFS=. read -r MA MI PA <<< "$CURRENT"
  SUGGEST="$MA.$MI.$((PA + 1))"
  echo "Version $CURRENT ist schon auf GitHub."
else
  # prepared but not published yet: publish it as it is
  SUGGEST="$CURRENT"
  echo "Version $CURRENT ist vorbereitet und noch nicht auf GitHub."
fi

read -r -p "Versionsnummer zum Veröffentlichen [$SUGGEST]: " NEW
NEW=${NEW:-$SUGGEST}
read -r -p "Was ist neu? (eine Zeile, Enter = Text aus der letzten Änderung): " NOTES

if [ "$NEW" != "$CURRENT" ]; then
  perl -0pi -e "s/^version = \"[^\"]+\"/version = \"$NEW\"/m" Cargo.toml
  cargo update --workspace >/dev/null 2>&1 || true
fi
git add -A
git diff --cached --quiet || git commit -m "Version $NEW${NOTES:+: $NOTES}"

# take changes made directly on GitHub (e.g. README edits) along
git pull --rebase origin main

if [ -z "$NOTES" ]; then
  if git rev-parse -q --verify "refs/tags/v$NEW" >/dev/null; then
    # a prepared tag already carries the "Was ist neu" text
    NOTES=$(git tag -l --format='%(contents)' "v$NEW")
  else
    NOTES=$(git log -1 --format=%B | grep -v '^Co-Authored-By' || true)
  fi
fi
# (re)create the tag on the current state, so changes pulled from GitHub are included
git tag -f -a "v$NEW" -m "$NOTES" >/dev/null

git push origin main
git push origin "v$NEW"

echo
echo "Version $NEW ist unterwegs. GitHub baut jetzt die Windows- und Mac-Version (etwa 10 Minuten)."
echo "Danach bietet PrintZilla das Update überall an."
open "$(git remote get-url origin | sed 's/\.git$//')/actions" || true
read -r -p "Enter zum Schließen"
