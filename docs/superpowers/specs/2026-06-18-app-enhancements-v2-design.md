# SEPA Validator App — Enhancements v2.0.0

**Datum:** 2026-06-18
**Status:** Design akzeptiert, bereit für Implementierungsplan
**Basis:** Tauri/Rust/Svelte App unter `app/` (siehe `2026-06-18-sepa-tauri-rewrite-design.md`)

## Ziel

Fünf konkrete Verbesserungen an der Desktop-App, plus ein neues korrekt
versioniertes Beta-Draft-Release.

## Erweiterungen

### 1. App-Icon
Ein eigenes Icon gestalten: gerundetes Quadrat in App-Blau, Motiv „XML-Dokument
+ Häkchen" (Validierung). Als 1024×1024-PNG erzeugen, dann per `npx tauri icon
<png>` alle Plattform-Formate nach `app/src-tauri/icons/` generieren. Wirkt auf
Fenster-, Taskbar- und Installer-Icon. PNG-Quelle wird deterministisch erzeugt
(z. B. Python/Pillow) und im Repo unter `app/src-tauri/icons/source.png` abgelegt.

### 2. Breitenverstellbare Panels
Das feste 3-Spalten-Grid (`260px / 1fr / 360px`) wird durch **ziehbare Trenner**
ersetzt:
- Gutter zwischen Code-Ansicht und Fehler-Panel (rechts) — die Hauptanforderung.
- Gutter zwischen Dateiliste und Code-Ansicht (links).
Spaltenbreiten werden in Svelte-State gehalten und als CSS-Breiten gebunden;
Drag über `mousedown`/`mousemove`/`mouseup` (Listener auf `window` während des
Drags). Breiten auf sinnvolle Min/Max-Werte begrenzt (z. B. links 160–480,
rechts 240–640). Keine externe Library.

### 3. XML-Formatierung mit korrekten Zeilennummern
**Kernpunkt — gekoppelte Anforderung.** Hübsch eingerückte Anzeige würde die
libxml-Zeilennummern (basierend auf der Originaldatei) entkoppeln, besonders bei
Einzeiler-Dateien. Lösung:

- Backend erhält einen deterministischen **Formatierer** `format_xml(path) ->
  Result<String, _>` (Pretty-Print mit Einrückung; nur Whitespace zwischen
  Elementen, Textinhalte unverändert).
- Die Validierung läuft auf der **formatierten Fassung**: `validate_file` liest
  die Datei, formatiert sie, validiert den formatierten Text. Damit beziehen sich
  Zeile/Spalte der Meldungen auf die angezeigte, eingerückte Ansicht.
- Neuer Tauri-Befehl `read_formatted(path) -> Result<String, String>` liefert
  dieselbe formatierte Fassung für die Code-Ansicht. Da Validierung und Anzeige
  denselben `format_xml` nutzen, sind die Zeilennummern garantiert konsistent.
- **Fehlerfälle:** Ist die Datei nicht wohlgeformt (Parse-Fehler), gibt
  `read_formatted` den Rohtext zurück; die Validierung meldet `ERROR` wie bisher
  (kein Zeilen-Mapping nötig). Das Urteil gültig/ungültig bleibt unverändert,
  da Einrückung nur Whitespace zwischen Tags einfügt.

### 4. Fehlerzeile anzeigen + Klick → mittig hervorheben
- Im Log zeigt jede Meldung klar „Zeile X" (mit Spalte, falls vorhanden) —
  bereits vorhanden, bleibt erhalten und ist durch Punkt 3 nun aussagekräftig.
- Klick auf eine Meldung: die betroffene Zeile wird **in die Mitte gescrollt**
  (`scrollIntoView`, `y: "center"`) und **betont hervorgehoben** (eigene
  „aktive Zeile"-Dekoration, kräftiger als die allgemeine Fehlerzeilen-Markierung,
  plus kurzes Aufblinken). Alle Fehlerzeilen bleiben dezent markiert.
- Umsetzung in `CodeViewer.svelte`: zusätzliches `StateField`/`StateEffect` für
  die aktive Zeile; `jumpTo(line)` setzt aktive Zeile + scrollt mittig.

### 5. Version & Release
- App-Version in `tauri.conf.json` (und ggf. `Cargo.toml`) auf `2.0.0` anheben.
- Nach der Umsetzung: den bestehenden `v0.1.0-beta.1`-Draft löschen und ein neues
  **Draft-Pre-Release `v2.0.0-beta.1`** erstellen (NSIS-Installer + portable exe,
  gleiche Notes-Struktur, `--target feature/tauri-rust-rewrite`).
- Begründung: Final im Repo ist `v1.0.0`; der Rust-Neubau ist ein Major-Sprung.

## Architektur / betroffene Dateien

- `app/src-tauri/icons/` (generiert) + `source.png` — Icon.
- `app/src-tauri/src/validator.rs` — `format_xml`, Validierung auf formatiertem Text.
- `app/src-tauri/src/commands.rs` — neuer `read_formatted`-Befehl; in `lib.rs` registrieren.
- `app/src/lib/api.ts` — `readFormatted`-Wrapper; CodeViewer nutzt ihn statt `readFile`.
- `app/src/App.svelte` + neue `Splitter`-Logik/Komponente — ziehbare Gutter.
- `app/src/lib/CodeViewer.svelte` — aktive-Zeile-Dekoration + mittiges Scrollen.
- `app/src-tauri/tauri.conf.json` — Version 2.0.0.

## Nicht im Umfang (YAGNI)
- Umstellung von eingebetteten Schemas auf Laufzeit-Ordner-Laden (separate
  Entscheidung wegen Lizenz/Verteilung; nicht Teil dieser Runde).
- macOS/Linux.

## Tests / Verifikation
- Rust: `format_xml` formatiert eine Einzeiler-Test-XML in mehrere Zeilen;
  Validierung einer invaliden Fixture liefert eine Zeilennummer > 1, die auf die
  formatierte Fassung zeigt (Konsistenz `read_formatted` ↔ Validierungszeilen).
- Frontend: `npm run check` 0/0; manueller Smoke-Test (Trenner ziehen, Klick →
  mittige Hervorhebung, Einzeiler-Datei wird eingerückt angezeigt).
- Build: `cargo test`, `npx tauri build --bundles nsis` für das Release.
