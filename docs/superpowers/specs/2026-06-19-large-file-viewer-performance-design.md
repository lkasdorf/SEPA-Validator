# Schlanker Viewer-Modus für große Dateien — Design

**Datum:** 2026-06-19
**Branch:** feature/tauri-rust-rewrite
**Komponente:** Tauri/Svelte-App (`app/`) — XML-Viewer (`CodeViewer.svelte`)

## Problem / Ursache

Bei ~60 MB großen Dateien ist die **Editor-Interaktion** (Scrollen, Tippen, Falten,
Tab-Wechsel) dauerhaft träge. Diagnose (systematic-debugging): Der XML-Viewer ist eine
CodeMirror-6-Instanz mit dem **kompletten** Dokument und mehreren *dokumentweit*
arbeitenden Extensions:

- `xml()` — Lezer-XML-Parser baut/pflegt einen Syntaxbaum über die ganze Datei
  (Highlighting + Fold-Service).
- `codeFolding()` + `foldGutter()` — berechnen faltbare Bereiche aus diesem Baum.
- `highlightSelectionMatches()` — scannt bei jeder Auswahländerung das **gesamte**
  Dokument.

Über 60 MB macht das jede Interaktion teuer (großer Syntaxbaum + Volltext-Scan +
Speicherdruck). **Root Cause:** der voll ausgestattete Editor über dem kompletten
Riesendokument.

## Lösung (Ansatz A)

Ab einem Schwellwert baut der Viewer **ohne** die schweren Extensions. Das Lade-/
Validier-Verhalten (vom Nutzer nicht als Problem genannt) bleibt unverändert.

## Scope-Entscheidungen

- **Schwellwert:** `HEAVY_LIMIT = 10 * 1024 * 1024` (≈ 10 MB). „Groß", wenn die Länge des
  geladenen (formatierten) Anzeige-Textes diesen Wert überschreitet.
- **Im großen Modus deaktiviert:** `xml()`, `codeFolding()`, `foldGutter()`,
  `highlightSelectionMatches()`.
- **Erhalten:** Zeilennummern, Fehlerzeilen-Markierung, Aktiv-Zeile-Flash,
  Klick-zu-Fehlerzeile, Suche, schreibgeschützt.
- Kein Pretty-Print-Skip / Roh-Modus, kein konfigurierbarer Schwellwert (YAGNI).

## Architektur / Komponenten

### `CodeViewer.svelte`

- Die schweren Extensions wandern in ein CodeMirror-**Compartment**
  (`heavyComp = new Compartment()`, importiert aus `@codemirror/state`). Im
  `EditorState.create`-Extensions-Array ersetzt `heavyComp.of([xml(), foldGutter(),
  codeFolding(), highlightSelectionMatches()])` die bisher dort einzeln stehenden
  vier Extensions. Position: direkt nach `lineNumbers()`, damit der Fold-Gutter (wenn
  aktiv) rechts der Zeilennummern erscheint. Alles andere (`oneDark`, `search`,
  `keymap`, `errorField`, `activeLineField`, `editable`, `readOnly`, `theme`) bleibt
  unverändert.
- Konstante `const HEAVY_LIMIT = 10 * 1024 * 1024;` (Modul-Ebene im `<script>`).
- In `loadFor(path, errorLines)`: nach erfolgreichem Lesen des Textes
  `const large = text.length > HEAVY_LIMIT;`. Die Text-Änderung wird zusammen mit der
  Compartment-Umkonfiguration dispatched:
  `view.dispatch({ changes: {...}, effects: [heavyComp.reconfigure(large ? [] : HEAVY)] })`
  (mit `HEAVY` = derselbe Vier-Extension-Array). Anschließend `viewerLarge.set(large)`.
  Wird nur ausgewertet, wenn sich der Pfad ändert (wie bisher die Text-Last);
  die nachfolgende `setErrorLines`/`setActiveLine`-Dispatch bleibt.
- `viewerLarge` wird **nicht** zurückgesetzt, wenn kein Pfad gewählt ist — der Wert
  spiegelt die zuletzt geladene Datei; das ist für die UI ausreichend, da die
  Collapse/Expand-Buttons ohnehin per `$selectedResult` gesteuert werden.

> Hinweis: `foldKeymap` bleibt im `keymap` (außerhalb des Compartments). Ohne
> `codeFolding()` sind seine Befehle wirkungslose No-ops — harmlos.

### `stores.ts`

- `export const viewerLarge = writable<boolean>(false);`

### `App.svelte`

- Importiert `viewerLarge` aus den Stores.
- In der Viewer-Leiste (nur im `xml`-Tab): die Buttons **Collapse all** / **Expand all**
  zusätzlich an `!$viewerLarge` koppeln (`{#if viewerTab === "xml" && !$viewerLarge}` für
  diese beiden; **Search** bleibt unter `viewerTab === "xml"`). Ein dezenter Hinweis
  erscheint, wenn `$viewerLarge` (und `viewerTab === "xml"`):
  „Große Datei: Syntax-Highlighting & Falten deaktiviert (Performance)".
- Layout/CSS für den Hinweis: kleine, gedämpfte Schrift in der Leiste (`app.css`).

## Datenfluss

Dateiauswahl → `selectedResult` → `CodeViewer.loadFor` liest den formatierten Text,
entscheidet `large` anhand der Textlänge, konfiguriert das Compartment um und setzt
`viewerLarge`. `App` blendet anhand von `$viewerLarge` die Falt-Buttons aus/ein und
zeigt den Hinweis. Keine Backend-Änderung.

## Fehler-/Leerfälle

- **Kein Dokument / Lesefehler:** `loadFor` kehrt früh zurück (wie bisher); `viewerLarge`
  bleibt auf dem letzten Wert (Buttons sind ohnehin per `$selectedResult` deaktiviert).
- **Datei knapp unter/über der Grenze:** rein größenbasiert; kein Sonderfall.
- **Wechsel groß → klein:** beim Laden der kleinen Datei wird das Compartment wieder voll
  bestückt; Highlighting/Folding sind zurück.

## Test / Verifikation

- `cd app && npm run check` (svelte-check + tsc) grün.
- `cargo test` nicht betroffen (reine Frontend-Änderung).
- Manueller GUI-Check: >10-MB-Datei → kein Syntax-Highlighting/Folding, Collapse/Expand
  ausgeblendet, Hinweis sichtbar, Scrollen/Suchen flüssig; kleine Datei → voll
  ausgestattet wie bisher; Klick-zu-Fehlerzeile funktioniert in beiden Modi. (Tatsächliche
  Flüssigkeit ist nur in der GUI bestätigbar, nicht headless.)

## Bewusst weggelassen (YAGNI)

- Pretty-Print-Skip / Roh-Modus (Ansatz B), inkl. Backend/Validierung.
- Konfigurierbarer Schwellwert, Umschalter „trotzdem voll laden".
- Eigene/virtualisierte Viewer-Komponente (Ansatz C).
- Performance-Optimierung des Lade-/Validier-Pfads (nicht als Problem gemeldet).
