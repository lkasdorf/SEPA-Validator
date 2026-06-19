# Schemas zur Laufzeit laden + Import (Teil 1) — Design

**Datum:** 2026-06-19
**Branch:** feature/tauri-rust-rewrite
**Komponente:** Tauri/Svelte-App (`app/`) — Schema-Laden, Import, Status-UI

## Ziel

Die ISO-20022/GBIC-XSDs werden **nicht mehr in die Binary eingebettet** (rechtliche
Weitergabe-Problematik). Stattdessen lädt der Validator sie zur Laufzeit aus einem
Schema-Ordner im App-Datenverzeichnis. Der Nutzer importiert die XSDs selbst über
einen Schemas-Dialog. Damit wird die App **ohne** geschützte Inhalte ausgeliefert
und ist veröffentlichbar.

Dies ist **Teil 1**. Der automatische Download von ebics.de ist **Teil 2** (eigener
Spec/Plan).

## Scope-Entscheidungen

- **Schema-Ordner:** `app_data_dir()/schemas/` (über Tauri `AppHandle` aufgelöst,
  `create_dir_all`). Plus „Ordner öffnen".
- **Import-Quellen:** einzelne `.xsd`-Dateien (Mehrfachauswahl) und ein Ordner
  (alle `.xsd` der obersten Ebene). Kein ZIP (Teil 2).
- **UI:** ein „Schemas…"-Knopf in der Toolbar (mit Status-Badge „N/M") öffnet einen
  modalen Dialog mit Schema-Status + Import/Ordner-öffnen.
- **Fehlendes Schema:** bekannter Namespace, aber Datei nicht importiert →
  bestehender Status `no_schema` mit klarer Meldung (kein neuer Status).
- **Ordner öffnen:** kleiner Rust-Command (Explorer, Windows-only), kein neues Plugin.

## Architektur / Komponenten

### Backend

**`schema.rs`** — Einbettung entfernen:
- `SCHEMAS: &[(&str, &str)]` (Namespace, Dateiname) — `include_bytes!` und das
  Bytes-Feld entfallen. Die 8 bekannten Einträge bleiben.
- `lookup(namespace: &str) -> Option<&'static str>` gibt nur noch den Dateinamen.
- Neu: `known_schemas() -> &'static [(&'static str, &'static str)]` für die Status-UI.
- Tests in `schema.rs` (Bytes-Prüfung) entfallen/werden auf Namen-/Lookup-Tests
  reduziert.

**`build.rs`** — den „Required schema file missing"-Block und den `xml_schema/`-Bezug
entfernen. `cargo:rustc-link-lib=dylib=bcrypt` und `tauri_build::build()` bleiben.

**Schema-Ordner-Helfer** (in `commands.rs`):
- `fn schema_dir(app: &AppHandle) -> Result<PathBuf, String>` — gibt
  `app.path().app_data_dir()?.join("schemas")` zurück und legt ihn an
  (`create_dir_all`).

**`validator.rs`** — von Platte laden:
- `Validator` erhält ein Feld `schema_dir: PathBuf`; `Validator::new(schema_dir: PathBuf)`.
- In `validate_file`: nach Namespace-Erkennung `schema::lookup(&ns)`:
  - `None` → `no_schema` „kein Schema für Namespace `<ns>`" (wie bisher).
  - `Some(filename)`: existiert `schema_dir/filename` **nicht** → `no_schema`
    „Schema `<filename>` nicht importiert (Schemas… → Importieren)". Sonst kompilieren
    (Cache-Key bleibt der `&'static`-Dateiname).
- `compile(ns)`: `let path = self.schema_dir.join(filename); let bytes =
  std::fs::read(path).map_err(...)?; SchemaParserContext::from_buffer(&bytes); …`.

**Commands** (`commands.rs`, registriert in `lib.rs` `generate_handler!`):
- `start_validation(app: AppHandle, paths, on_event)` — neu mit `app`; löst
  `schema_dir` auf und gibt den `PathBuf` in den Worker-Thread → `Validator::new(dir)`.
- `schema_status(app) -> Result<Vec<SchemaInfo>, String>` mit
  `SchemaInfo { namespace: String, filename: String, present: bool }` (serde
  camelCase), aus `known_schemas()` + Existenzprüfung im `schema_dir`.
- `import_schemas(app, paths: Vec<String>) -> Result<ImportResult, String>` mit
  `ImportResult { imported: u32, skipped: Vec<String> }`. Je Pfad: ist es ein
  Verzeichnis → alle `.xsd` der obersten Ebene kopieren; ist es eine `.xsd`-Datei →
  kopieren; sonst in `skipped`. Ziel: `schema_dir`. Überschreibt vorhandene gleichen
  Namens.
- `open_schema_dir(app) -> Result<(), String>` — `schema_dir` ermitteln und
  `std::process::Command::new("explorer").arg(dir).spawn()` (Windows).

**Tests:**
- `validator.rs`: ein Helfer `test_schema_dir() -> Option<PathBuf>` gibt das Repo-
  `xml_schema/` zurück (`CARGO_MANIFEST_DIR/../../xml_schema`), falls vorhanden.
  Tests, die echt kompilieren (`valid_fixture_is_ok` etc.), holen sich den Ordner und
  **return**en früh, wenn er fehlt (Skip) — so bleibt die Suite im sauberen Checkout
  grün (XSDs sind nie im Repo). `Validator::new(dir)` statt parameterlos.
- Neue Tests in `commands.rs` (oder einem `schema`-Testmodul): `import_schemas` kopiert
  eine `.xsd`-Datei und alle `.xsd` aus einem Ordner in ein temporäres Ziel und meldet
  die Anzahl; nicht-`.xsd` landen in `skipped`. (Diese Tests nutzen ein Temp-Verzeichnis
  als Ziel; sie hängen nicht am `AppHandle` — die Kopierlogik wird in eine reine
  Hilfsfunktion `copy_xsds(paths, dest) -> ImportResult` ausgelagert und getestet.)

### Frontend

- **`api.ts`**: `schemaStatus(): Promise<SchemaInfo[]>`, `importSchemas(paths: string[]):
  Promise<ImportResult>`, `openSchemaDir(): Promise<void>`. TS-Typen `SchemaInfo`,
  `ImportResult` in `types.ts`.
- **`SchemaDialog.svelte`** (neu, modales Overlay): lädt beim Öffnen `schemaStatus()`.
  Tabelle: je bekanntem Schema Namespace · Datei · Status (✓ grün / ✗ rot). Buttons:
  - **„XSD-Dateien…"** → `dialog.open({ multiple: true, filters: [{ name: "XSD",
    extensions: ["xsd"] }] })` → `importSchemas(paths)` → Status neu laden.
  - **„Ordner…"** → `dialog.open({ directory: true })` → `importSchemas([dir])` → neu laden.
  - **„Ordner öffnen"** → `openSchemaDir()`.
  - **„Schließen"**. (Platzhalter-Bereich/-Knopf „Herunterladen" für Teil 2 wird hier
    später ergänzt.)
- **`Toolbar.svelte`**: Knopf **„Schemas…"** öffnet den Dialog (lokaler `open`-State in
  `App.svelte` oder im Toolbar). Kleines Badge „N/M" aus `schemaStatus()` (beim Start
  und nach Import aktualisiert).

## Datenfluss

Validierung: `start_validation` löst den Schema-Ordner auf → Worker-Thread baut
`Validator::new(schema_dir)` → lädt je Namespace die XSD-Datei aus dem Ordner. Fehlt
sie, streamt das Ergebnis `no_schema` mit der Import-Hinweis-Meldung. Der Schemas-Dialog
liest/aktualisiert den Status unabhängig.

## Fehler-/Leerfälle

- **Keine Schemas importiert:** Validierung liefert für jede Datei `no_schema` mit
  Hinweis; das Toolbar-Badge zeigt „0/8"; der Dialog zeigt alle als fehlend.
- **Import einer Nicht-XSD / leerer Ordner:** `imported: 0`, Pfade in `skipped`; Dialog
  zeigt unveränderten Status.
- **`app_data_dir()` nicht auflösbar:** Command gibt `Err(String)`; Frontend zeigt die
  Fehlermeldung.

## Test / Verifikation

- `cd app/src-tauri && cargo test` grün (Schema-kompilierende Tests skippen ohne lokale
  XSDs; `copy_xsds`-Tests laufen immer).
- `cd app && npm run check` grün.
- Manueller GUI-Check: „Schemas…" öffnen → alle fehlen; „Ordner…" auf lokales
  `xml_schema/` → Badge/Status werden vollständig; eine Datei validieren → ok statt
  no_schema.

## Distribution / Docs

- Build bettet keine XSDs mehr ein → Installer/Portable-EXE enthalten keine geschützten
  Inhalte → veröffentlichbar.
- `CLAUDE.md` und `app/README.md`: „Schemas eingebettet via `include_bytes!`" →
  „zur Laufzeit aus `app_data_dir/schemas/` geladen; per Schemas-Dialog importieren".
- **Dev-Migration:** nach dem Umbau einmalig die lokalen `xml_schema/`-XSDs über den
  Dialog importieren.

## Bewusst weggelassen (YAGNI)

- ZIP-Import und Auto-Download (Teil 2).
- Konfigurierbarer Schema-Pfad / mehrere Schema-Ordner.
- Inhaltliche XSD-Prüfung beim Import (nur `.xsd`-Endung + Dateiname zählen).
- Cross-Plattform „Ordner öffnen" (Windows-only).
