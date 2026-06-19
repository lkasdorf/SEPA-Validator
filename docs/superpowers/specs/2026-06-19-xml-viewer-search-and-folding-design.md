# XML-Viewer: Suche & Einklappen — Design

**Datum:** 2026-06-19
**Branch:** feature/tauri-rust-rewrite
**Komponente:** Tauri/Svelte-App (`app/`), XML-Viewer (`CodeViewer.svelte`)

## Ziel

Im aktuell angezeigten XML soll der Nutzer

1. **suchen** können (Treffer hervorheben, dazwischen springen, mit Optionen), und
2. **Blöcke einklappen** können (XML-Elemente falten/entfalten).

Beides ist reine Frontend-Funktionalität im bestehenden CodeMirror-6-Viewer. Kein
Backend-Code, kein neuer Tauri-Command, kein eigener Such- oder Fold-Algorithmus —
CodeMirror liefert beides als Extensions.

## Scope-Entscheidungen

- **Suchbereich:** nur die aktuell im Viewer geöffnete XML-Datei (keine
  dateiübergreifende Suche).
- **Bedienung Suche:** `Strg+F` (eingebautes CodeMirror-Find-Panel) **plus** ein
  sichtbarer `Search`-Button, damit die Funktion auffindbar ist.
- **Bedienung Folding:** anklickbare Falt-Pfeile in der Gutter-Spalte **plus**
  `Collapse all` / `Expand all`-Buttons.
- **Sprache:** UI ist Englisch ("Select Files…", "Export TXT"); Buttons und das
  CodeMirror-Standard-Panel bleiben Englisch. Keine Lokalisierung.

## Architektur / Komponenten

### Gemeinsame UI — Viewer-Leiste

Eine schmale Button-Leiste oben in der `.viewer`-Section in `App.svelte`, im
bestehenden App-Button-Stil. Drei Buttons:

- **Search** — öffnet das Find-Panel.
- **Collapse all** — faltet alle Blöcke.
- **Expand all** — entfaltet alle Blöcke.

Die Buttons sind deaktiviert, wenn kein Dokument geladen ist (`$selectedResult`
leer). Die Leiste liegt direkt beim Inhalt, den sie bedient (nicht in der
Top-Toolbar).

### Anbindung Button → Editor

Analog zum bestehenden `jumpToLine`-Muster: `CodeViewer.svelte` legt beim Mount
Funktions-Referenzen in Stores ab, die die Button-Leiste aufruft. Kein
Prop-Drilling.

- Neue Stores in `stores.ts` (gleiches Muster wie `jumpToLine`):
  - `openSearch: Writable<() => void>`
  - `foldAllBlocks: Writable<() => void>`
  - `unfoldAllBlocks: Writable<() => void>`
- `CodeViewer` setzt diese in `onMount` auf Funktionen, die `openSearchPanel(view)`,
  `foldAll(view)` bzw. `unfoldAll(view)` aufrufen.

### Feature 1 — Suche

- **Neue Abhängigkeit:** `@codemirror/search` (passend zur installierten CM-6.x-Linie).
- **Extensions in `CodeViewer.svelte`** ergänzen:
  - `search({ top: true })` — Panel oben.
  - `highlightSelectionMatches()` — markiert weitere Vorkommen der Auswahl.
  - `keymap.of(searchKeymap)` — `Strg+F` öffnet, `Enter`/`Shift+Enter` = nächster/
    voriger Treffer, `Esc` schließt.
  - `EditorState.readOnly.of(true)` **zusätzlich** zum bestehenden
    `EditorView.editable.of(false)` — dadurch blendet das Such-Panel die
    Replace-Felder aus; es bleibt eine reine Find-Leiste.
- Der `Search`-Button ruft `openSearchPanel(view)` (über den Store) auf.

### Feature 2 — Einklappen (Folding)

- **Abhängigkeit:** `@codemirror/language` ist bereits installiert (`^6.12.3`).
- **Extensions in `CodeViewer.svelte`** ergänzen:
  - `codeFolding()` — Fold-State.
  - `foldGutter()` — anklickbare Pfeile (▾ offen / ▸ eingeklappt) neben den
    Zeilennummern.
  - `keymap.of(foldKeymap)` — Tastenkürzel zum Falten/Entfalten.
- XML-Elemente sind faltbar, weil `@codemirror/lang-xml` (`xml()`) `foldNodeProp`
  für Element-Knoten registriert.
- Die `Collapse all` / `Expand all`-Buttons rufen `foldAll(view)` /
  `unfoldAll(view)` (über die Stores) auf.

## Datenfluss

Unverändert: Dateiauswahl/Validierung streamt Ergebnisse → `selectedResult` →
`CodeViewer.loadFor()` lädt formatierten XML-Text via `readFormatted`. Suche und
Folding wirken rein clientseitig auf das bereits geladene Dokument im
`EditorView`.

## Koexistenz mit bestehenden Features

Such-Hervorhebung, Fold-Gutter, Fehlerzeilen-Highlight (`cm-error-line`),
Aktiv-Zeile-Flash (`cm-active-error-line`) und Klick-zu-Zeile (`jumpTo`) nutzen
getrennte Gutter bzw. Decoration-Layer und beeinflussen sich nicht. Reihenfolge der
Gutter: Falt-Gutter neben den Zeilennummern.

## Fehlerfälle

- **Kein Dokument geladen:** Viewer-Buttons sind deaktiviert; selbst bei Aufruf ist
  ein leeres Such-Panel bzw. ein No-op-Fold harmlos.
- **Such-Panel im read-only Editor:** durch `EditorState.readOnly.of(true)` werden
  Replace-Felder ausgeblendet; Find funktioniert weiter.

## Test / Verifikation

- `cd app && npm run check` (svelte-check + tsc) muss grün bleiben.
- `cargo test` ist nicht betroffen (reine Frontend-Änderung).
- Manuell: XML laden; `Strg+F` und `Search`-Button testen (Treffer-Highlight,
  Weiter/Zurück, `Esc`); Falt-Pfeile am Rand testen; `Collapse all` / `Expand all`.

## Bewusst weggelassen (YAGNI)

- Dateiübergreifende Suche.
- Ersetzen (Replace).
- Eigene Suchleiste im App-Stil statt des CodeMirror-Panels.
- Persistenz von Suchbegriff oder Fold-Zustand über Dateiwechsel/Neustart hinweg.
