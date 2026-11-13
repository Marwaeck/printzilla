#!/bin/bash
# Doppelklick: legt PrintZilla als öffentliches Repository auf GitHub an,
# lädt alles hoch und startet den Bau der ersten Windows- und Mac-Version.
set -e
cd "$(dirname "$0")"
REPO="printzilla"

echo "== PrintZilla auf GitHub veröffentlichen =="

if ! command -v gh >/dev/null 2>&1; then
  if command -v brew >/dev/null 2>&1; then
    echo "Das GitHub-Werkzeug fehlt noch – es wird jetzt installiert …"
    brew install gh
  else
    echo "Bitte zuerst das GitHub-Werkzeug von https://cli.github.com installieren."
    read -r -p "Enter zum Schließen"
    exit 1
  fi
fi

if ! gh auth status >/dev/null 2>&1; then
  echo "Gleich öffnet sich der Browser – bitte bei GitHub anmelden und den Code bestätigen."
  gh auth login --hostname github.com --git-protocol https --web
fi
gh auth setup-git

OWNER=$(gh api user --jq .login)
echo "Angemeldet als: $OWNER"

# Die Update-Funktion muss auf das richtige Konto zeigen.
if [ "$OWNER" != "Marwaeck" ] && grep -q "Marwaeck/printzilla" src/updater.rs; then
  sed -i '' "s#Marwaeck/printzilla#$OWNER/$REPO#g" src/updater.rs Cargo.toml
  git add src/updater.rs Cargo.toml
  git commit -m "Update-Adresse auf $OWNER/$REPO setzen"
fi

if gh repo view "$OWNER/$REPO" >/dev/null 2>&1; then
  echo "Repository $OWNER/$REPO gibt es schon – lade Änderungen hoch …"
  git remote get-url origin >/dev/null 2>&1 || git remote add origin "https://github.com/$OWNER/$REPO.git"
  git push -u origin main
else
  echo "Lege das öffentliche Repository $OWNER/$REPO an …"
  gh repo create "$OWNER/$REPO" --public --source . --remote origin --push \
    --description "PrintZilla – Bibliothek für 3D-Druck-Dateien mit 3MF-Ansicht (Windows & macOS)"
fi

VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
if ! git rev-parse "v$VERSION" >/dev/null 2>&1; then
  git tag "v$VERSION"
fi
git push origin "v$VERSION" || true

echo
echo "Fertig! Die Seite: https://github.com/$OWNER/$REPO"
echo "GitHub baut jetzt die Windows- und Mac-Version (dauert etwa 10 Minuten)."
open "https://github.com/$OWNER/$REPO/actions" || true
read -r -p "Enter zum Schließen"
