---
layout: default
title: Anleitung
description: So benutzt du PrintZilla – Installation, Import, 3D-Ansicht, Ordner überwachen, Updates.
permalink: /anleitung/
---

<div class="doc" markdown="1">

# Anleitung

PrintZilla sammelt deine 3D-Druck-Dateien an einem Ort: Du importierst sie aus beliebigen Ordnern,
siehst sie dir in 3D an, ordnest sie und öffnest sie mit einem Klick im Slicer.

**Inhalt:** [Installation](#installation) · [Dateien importieren](#dateien-importieren) ·
[3D-Ansicht](#3d-ansicht) · [Ordnen und finden](#ordnen-und-finden) ·
[In anderen Programmen öffnen](#in-anderen-programmen-öffnen) ·
[Ordner überwachen](#ordner-überwachen) · [Updates](#updates) · [Häufige Fragen](#häufige-fragen)

![PrintZilla mit Bibliothek und 3D-Ansicht]({{ '/assets/img/screenshot.png' | relative_url }})

## Installation

1. Lade auf der [Startseite]({{ '/#download' | relative_url }}) die Datei für dein System herunter.
2. **Windows:** Zip entpacken und `PrintZilla.exe` starten. Erscheint „Der Computer wurde durch
   Windows geschützt“, auf *Weitere Informationen* → *Trotzdem ausführen* klicken.
3. **macOS:** Zip öffnen, `PrintZilla.app` in den Ordner *Programme* ziehen und beim ersten Mal mit
   *Rechtsklick → Öffnen* starten.

Deine Bibliothek liegt danach im Ordner `Dokumente/3D-Druck-Bibliothek`. Über das Menü
**PrintZilla ▾** oben links kannst du einen anderen Ordner wählen.

## Dateien importieren

1. Klicke oben auf **Importieren** – oder ziehe einen Ordner einfach ins PrintZilla-Fenster.
2. Wähle den **Quellordner**, zum Beispiel deinen Download-Ordner. PrintZilla durchsucht ihn sofort.
3. Die Liste zeigt alle gefundenen Dateien, sortiert nach Unterordnern. Neue Dateien sind
   mit **neu** markiert, schon vorhandene mit **schon vorhanden**. Mit *Alle*, *Nur neue* und
   *Keine* wählst du schnell aus.
4. Unten legst du fest, was mit den Originalen passiert:

   | Auswahl | Bedeutung |
   | --- | --- |
   | Originale behalten | Die Dateien werden nur kopiert. |
   | In den Papierkorb legen | Das Original wandert in den Papierkorb, sobald die Kopie sicher in der Bibliothek liegt. |
   | Endgültig löschen | Wie oben, aber ohne Papierkorb. PrintZilla fragt vorher noch einmal nach. |

5. Klicke auf **Dateien importieren**.

Unter **Einstellungen** im Import-Fenster stellst du ein, ob Unterordner durchsucht werden, welche
Dateitypen gesucht werden und ob der Ordnername als Schlagwort übernommen wird.

## 3D-Ansicht

Klicke ein Modell an – rechts erscheint es in 3D. Das klappt mit **3MF** (auch mehrteiligen
Projekten aus dem Slicer), **STL** und **OBJ**.

- **Ziehen:** drehen
- **Rechts ziehen** oder **Umschalt + Ziehen:** verschieben
- **Mausrad:** zoomen
- **Doppelklick:** zurück zur Ausgangsansicht
- Knöpfe **3D**, **Vorne**, **Seite**, **Oben** für feste Ansichten, **Platte** blendet die Druckplatte ein oder aus
- **Großansicht** zeigt das Modell über die ganze Fensterbreite

Darunter stehen Maße, Anzahl der Objekte und Dreiecke sowie die Angaben aus der 3MF-Datei.

## Ordnen und finden

- **Seitenleiste:** Alle Dateien, *Ohne Schlagwort*, *Favoriten*, *Zuletzt importiert*, Dateitypen und Ordner
- **Suche** oben findet Namen, Schlagworte und Notizen
- **Filter** *Typen*, *Schlagworte*, *Favoriten* und die **Sortierung** stehen unter der Werkzeugleiste
- **Regler** oben ändert die Größe der Vorschaubilder
- Rechts kannst du **Name**, **Beschreibung** und **Schlagworte** ändern und mit dem Stern einen **Favoriten** markieren

## In anderen Programmen öffnen

Rechtsklick auf eine Datei:

- **In SolidCraft öffnen** – für 3MF, STL, STEP und IGES
- **Öffnen mit** – zeigt gefundene Programme wie Bambu Studio, OrcaSlicer, PrusaSlicer oder
  UltiMaker Cura, dazu das Standardprogramm und *Anderes Programm hinzufügen …*

Findet PrintZilla ein Programm nicht, fragt es einmal nach dem Speicherort und merkt ihn sich.

## Ordner überwachen

PrintZilla kann Ordner im Blick behalten, zum Beispiel deinen Download-Ordner:

1. Menü **PrintZilla ▾ → Ordner überwachen …**
2. **Downloads-Ordner hinzufügen** oder **Ordner hinzufügen …** wählen.

Sobald dort eine neue 3D-Druck-Datei fertig heruntergeladen ist, erscheint ein blauer Hinweis in
der Seitenleiste, eine **Zahl am App-Symbol** (Dock bzw. Taskleiste) und auf Wunsch eine
Mitteilung. Mit **Ansehen und übernehmen …** öffnet sich das Import-Fenster mit den neuen Dateien.
Dateien, die du nicht willst, blendest du mit **Nicht mehr anzeigen** aus. Ohne deine Bestätigung
wird nichts kopiert.

## Updates

PrintZilla sucht beim Start nach einer neuen Version. Gibt es eine, erscheint links ein grüner
Hinweis. Mit **Jetzt installieren** und **Jetzt neu starten** bist du auf dem neuesten Stand –
deine Bibliothek bleibt dabei unverändert. Von Hand: **PrintZilla ▾ → Nach Updates suchen …**

Das PrintZilla-Symbol in der **Menüleiste** (macOS) bzw. im **Infobereich** (Windows) bietet die
wichtigsten Aktionen auch, wenn das Fenster geschlossen ist. Es lässt sich im Menü
**PrintZilla ▾** ausschalten.

## Häufige Fragen

**Werden meine Dateien verändert?**
Nein. PrintZilla kopiert Dateien in die Bibliothek. Originale werden nur entfernt, wenn du das im
Import-Fenster ausdrücklich auswählst.

**Kostet PrintZilla etwas?**
Nein. PrintZilla ist freie Software unter der
[MIT-Lizenz]({{ site.github_url }}/blob/main/LICENSE).

**Ich habe einen Fehler gefunden oder eine Idee.**
Gern als [Issue auf GitHub]({{ site.github_url }}/issues).

</div>
