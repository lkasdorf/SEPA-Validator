# SEPA Validator — Tauri/Rust Rewrite (v1)

**Datum:** 2026-06-18
**Status:** Design akzeptiert, bereit für Implementierungsplan
**Plattform v1:** Windows (zunächst nur Windows)

## Ziel

Den bestehenden SEPA-XML-Validator (PowerShell/WinForms) als **moderne Desktop-App
in Rust** neu bauen. Schwerpunkt: eine **moderne Oberfläche** mit einer **richtig
guten, hilfreichen Logansicht** beim Validieren. Die bestehenden PowerShell- und
Bash-Tools bleiben unverändert im Repo erhalten.

## Kernentscheidungen (bestätigt)

| Thema            | Entscheidung |
|------------------|--------------|
| GUI-Fundament    | **Tauri** (Rust-Backend + Web-Frontend) |
| Frontend         | **Svelte + TypeScript** (Vite) |
| XSD-Engine       | **libxml2** über die `libxml`-Crate, statisch ins Binary gelinkt (vcpkg unter Windows) |
| Theme            | **System-abhängig** (Windows hell/dunkel) + manueller Toggle; Code-Pane immer dunkel |
| Export           | **TXT + CSV** |
| Plattform        | Windows (v1) |

## Logansicht — Anforderungen (alle bestätigt)

1. **Live-Stream beim Prüfen** — jede Datei erscheint im Log, während sie validiert wird; laufender Fortschritt.
2. **Klickbar → Stelle im XML** — Klick auf eine Meldung springt zur Zeile/Spalte und zeigt den XML-Ausschnitt mit Syntax-Highlighting im Kontext.
3. **Filtern & Suchen** — nach Schweregrad (Fehler / Warnungen / Alle) filtern und per Textsuche durchsuchen.
4. **Farbig & gruppiert** — Meldungen farbcodiert nach Schweregrad, pro Datei gruppiert, mit Zusammenfassung oben.

## Architektur

```
sepa/
├─ windows/        ← bleibt (PowerShell-Tool, unverändert)
├─ scripts/        ← bleibt (Bash-CLI, unverändert)
├─ xml_schema/     ← XSDs (gitignored, werden beim Build eingebettet)
└─ app/            ← NEU
   ├─ src-tauri/   ← Rust-Backend
   │  ├─ build.rs  ← bettet XSDs aus ../../xml_schema/ gzip-komprimiert ein
   │  └─ src/
   │     ├─ main.rs       ← Tauri-Setup, Commands, Event-Emitter
   │     ├─ schema.rs     ← eingebettete XSDs, Namespace→XSD-Map, Schema-Cache
   │     ├─ validator.rs  ← validiert eine Datei via libxml2 → ValidationResult
   │     └─ scanner.rs     ← Ordner/Drag&Drop → XML-Dateiliste (rekursiv)
   └─ src/         ← Svelte-Frontend
      ├─ App.svelte
      └─ lib/      ← FileList, CodeViewer (CodeMirror 6), LogPanel, SummaryBar, Toolbar
```

### Backend-Module

- **`schema`** — Eingebettete XSDs (im Binary, beim Build via `build.rs` aus `xml_schema/`
  gzip+eingebettet, analog zum heutigen `build.ps1`). Hält die geordnete
  Namespace→XSD-Map (identisch zum heutigen `$SchemaMap`). Kompilierte
  `libxml`-Schema-Objekte werden pro Namespace gecacht (lazy, einmalig).
- **`validator`** — `validate_file(path) -> ValidationResult`. Liest zuerst den
  Namespace des Wurzelelements, wählt das Schema, validiert via libxml2 und sammelt
  alle Meldungen (Fehler + Warnungen) mit Zeile/Spalte.
- **`scanner`** — expandiert übergebene Pfade (Dateien + Ordner) rekursiv zu einer
  Liste von `.xml`-Dateien; filtert `*:Zone.Identifier`.

### Datenmodell

```rust
struct ValidationResult {
    file: String,        // Dateiname
    path: String,        // voller Pfad
    namespace: String,
    schema: String,      // gewählte XSD-Datei oder ""
    status: Status,
    messages: Vec<Message>,
}

enum Status { Ok, Invalid { errors: u32, warnings: u32 },
              Warnings(u32), NoSchema, Error }

struct Message {
    severity: Severity,  // Error | Warning
    text: String,
    line: Option<u32>,
    column: Option<u32>,
}
```

Statuslogik identisch zum heutigen Verhalten:
- Fehler vorhanden → `INVALID (n errors, m warnings)`
- nur Warnungen → `WARNINGS (n)`
- kein passendes Schema → `NO SCHEMA`
- kein Namespace / Datei kaputt / Schema-Kompilierfehler → `ERROR`
- sonst → `OK`

### Datenfluss (live)

1. Nutzer fügt Dateien hinzu (Dateiauswahl / Ordnerauswahl / Drag-&-Drop).
2. Frontend ruft Tauri-Command `start_validation(paths)` auf.
3. Backend expandiert via `scanner`, startet einen Hintergrund-Task (async/thread).
4. Pro Datei: validieren und ein **`validation:result`**-Event mit dem
   `ValidationResult` senden → Frontend hängt es **live** an die Liste; dazu
   `validation:progress`-Events (n/total) für die Fortschrittsanzeige.
5. Abschluss-Event mit Zusammenfassung (OK / ungültig / Warnungen / kein Schema).
6. Klick auf eine Meldung → Command `read_file(path)` (oder bereits geladener
   Inhalt) → CodeViewer zeigt XML, scrollt zur Zeile, markiert die Fehlerstelle.

## UI / Layout

- **Kopfzeile**: Titel + globale Suche + Theme-Toggle.
- **Toolbar**: „Dateien…", „Ordner…", „Export…".
- **Links**: Dateiliste mit Status-Icon (✓/✗/⚠, farbcodiert) und Dateiname; gruppiert/sortierbar.
- **Rechts**: CodeViewer (CodeMirror 6, XML-Highlighting, dunkles Theme) mit
  Inline-Fehlermarkern; darunter/daneben die Meldungsliste der aktiven Datei.
- **Unten**: Filterleiste (Fehler / Warnungen / Alle) + Zusammenfassung
  (Gesamt | OK | Ungültig | Warnungen | Kein Schema) + Fortschrittsbalken während der Prüfung.
- **Theme**: folgt Windows hell/dunkel, manueller Umschalter; Code-Pane immer dunkel.

## Umfang v1 (YAGNI)

**Enthalten:** Validieren (Dateiauswahl / Ordner / Drag-&-Drop), Live-Log,
klickbare XML-Ansicht mit Sprung zur Zeile, Filter & Suche, farbig+gruppiert,
Export als TXT + CSV.

**Nicht enthalten (v1):** Umbenennen/Sortieren-Kuratierung (bleibt in den
Bash-Skripten), Cross-Platform-Pakete (macOS/Linux), Auto-Update, Signierung.

## Fehlerbehandlung

Abgedeckte Fälle (gespiegelt vom heutigen Tool): Datei nicht gefunden, kein
XML-Namespace erkennbar, kein passendes Schema, Schema-Kompilierfehler,
fehlerhaftes XML. Jeder Fall mündet in einen definierten `Status` mit klarer
Meldung im Log statt in einem Absturz. Keine Netzwerkzugriffe bei der Validierung
(libxml2 ohne externe Auflösung — Pendant zum heutigen `XmlResolver = $null`).

## Testing

- **Rust-Unit-/Integrationstests** für `validator` gegen die vorhandenen Fixtures:
  `to_check/valid/` muss `OK` liefern, `to_check/invalid/` muss `INVALID`/`ERROR`
  liefern. Direkte Regressionssicherung gegen die alte .NET-Engine.
- `schema`-Test: jeder Namespace der Map kompiliert ohne Fehler.
- `scanner`-Test: Ordner-Expansion und `Zone.Identifier`-Filter.
- Frontend: minimal (manuelle UI-Prüfung in v1).

## Risiken & offene Punkte

1. **libxml2-Build unter Windows** — vcpkg-Integration mit der `libxml`-Crate.
   *Mitigation:* früher Spike als erster Implementierungsschritt; Plan B ist
   `xmllint.exe` mitliefern und aufrufen.
2. **Schema-Selbstständigkeit** — Annahme: die XSDs sind in sich geschlossen
   (keine externen `import`/`include`-Auflösungen nötig), wie schon beim heutigen
   `XmlResolver = $null`-Ansatz. *Mitigation:* im Spike pro Namespace
   Kompilierung verifizieren.
3. **Abweichende Fehlertexte** — libxml2 formuliert Meldungen anders als .NET.
   Gleichwertiges Urteil, aber anderer Wortlaut. Akzeptiert.
4. **Schema-Lizenz** — XSDs sind nicht weiterverteilbar; bleiben gitignored und
   werden nur lokal beim Build eingebettet (wie heute). Verteilung der App muss
   das berücksichtigen.

## Nächster Schritt

Implementierungsplan via writing-plans-Skill erstellen. Erster Plan-Schritt:
libxml2-Spike (Risiko 1 + 2 entschärfen), bevor UI gebaut wird.
