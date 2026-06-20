# Schemas beschaffen: Download-Seite öffnen + ZIP-Import (Teil 2) — Design

**Datum:** 2026-06-20
**Branch:** feature/tauri-rust-rewrite
**Komponente:** Tauri/Svelte-App (`app/`) — Schema-Import + Schemas-Dialog

## Hintergrund / Machbarkeit

Aufbauend auf Teil 1 (XSDs nicht eingebettet, Laufzeit-Laden + manueller Import).
Ein echtes Auto-Download von ebics.de ist **nicht** sinnvoll umsetzbar: die Download-
Links sind tokenisiert und laufen ab (`/securedl/sdl-<JWT>/…` mit `exp`), die XSDs sind
nicht direkt auf der Datenformate-Seite verlinkt, und die Seite ist „Alle Rechte
vorbehalten". Statt fragilem, rechtlich heiklem Scraping: der Nutzer lädt die Schemas
selbst von der offiziellen Quelle (klar zulässig für den Eigengebrauch), und die App
macht das **Öffnen der Quelle** und den **ZIP-Import** bequem.

## Ziel

1. Ein **„Herunterladen…"**-Knopf öffnet die offizielle Schema-Quelle im Browser.
2. Der **Import akzeptiert ZIP-Bundles** (entpackt die enthaltenen `.xsd`), zusätzlich zu
   den bisherigen XSD-Dateien/Ordnern.

## Scope-Entscheidungen

- Download-Knopf öffnet `https://www.ebics.de/de/datenformate` im Standardbrowser
  (Windows, via `explorer <url>`). Der Dialog-Hinweis nennt ebics.de (DK/GBIC) und
  iso20022.org (ISO-Schemas) als Quellen.
- ZIP-Import: ausgewählte `.zip` → enthaltene `.xsd` (Eintragsname auf Basisnamen
  reduziert) in den Schema-Ordner. Der bestehende `.xsd`-Datei-/Ordner-Import bleibt.
- Der Datei-Picker akzeptiert nun `.xsd` **und** `.zip` (statt eines separaten ZIP-Knopfs).
- Kein Auto-Download/Scraping; keine rekursive ZIP-Suche in Ordnern; keine XSD-Inhalts-
  prüfung; ein fester Download-Link (kein Mehr-Quellen-Dialog). (YAGNI)

## Architektur / Komponenten

### Backend

- **`Cargo.toml`:** Abhängigkeit `zip` (aktuelle Version) hinzufügen.
- **`commands.rs`:**
  - Neue Hilfsfunktion `extract_zip_xsds(zip_path: &Path, dest: &Path) -> (u32, Vec<String>)`
    — öffnet die ZIP (`zip::ZipArchive`), iteriert Einträge; für jeden Eintrag, dessen
    Name (case-insensitive) auf `.xsd` endet, wird der **Basisname** ermittelt und der
    Eintragsinhalt nach `dest/<basisname>` geschrieben (vorhandene überschrieben).
    Rückgabe: Anzahl entpackter XSDs und übersprungene Hinweise. Lese-/Schreibfehler →
    der ZIP-Pfad landet in `skipped`.
  - `copy_xsds(paths, dest)` (aus Teil 1) wird erweitert: ist ein Pfad eine `.zip`-Datei,
    wird `extract_zip_xsds` aufgerufen und dessen Ergebnis in `imported`/`skipped`
    aggregiert; `.xsd`-Datei und Verzeichnis wie bisher; sonst `skipped`.
  - Neuer Command `open_url(url: String) -> Result<(), String>` —
    `std::process::Command::new("explorer").arg(&url).spawn()` (Windows), registriert in
    `lib.rs`.
- **Tests (`commands.rs`):** Mit dem `zip`-Crate (auch als dev-Writer nutzbar) eine
  Test-ZIP im Temp anlegen (ein `a.xsd` + ein `readme.txt`), dann `copy_xsds([zip], dest)`
  → `imported == 1`, `dest/a.xsd` existiert, `readme.txt` nicht. Die bestehenden
  Datei-/Ordner-Tests bleiben.

### Frontend

- **`api.ts`:** `openUrl(url: string): Promise<void>` (`invoke("open_url", { url })`).
- **`SchemaDialog.svelte`:**
  - Der „XSD-Dateien…"-Knopf wird zu **„XSD/ZIP-Dateien…"**; sein `dialog.open`-Filter
    akzeptiert `extensions: ["xsd", "zip"]`. Die Import-Logik (`runImport` → `importSchemas`)
    bleibt; das Backend entpackt ZIPs.
  - Neuer Knopf **„Herunterladen…"** ruft `openUrl("https://www.ebics.de/de/datenformate")`.
  - Der Hinweistext wird ergänzt: „Lade die Schemas von der offiziellen Quelle (ebics.de
    für DK/GBIC, iso20022.org für die ISO-Schemas) und importiere sie hier als ZIP oder XSD."

## Datenfluss

Unverändert zu Teil 1: `import_schemas` → `copy_xsds` schreibt (jetzt auch aus ZIPs) nach
`app_data_dir/schemas/`; danach lädt der Dialog `schema_status` neu. Der Download-Knopf
ist ein reiner Browser-Öffner (kein App-seitiges Laden).

## Fehler-/Leerfälle

- **ZIP ohne XSD / beschädigte ZIP:** `imported == 0`, ZIP-Pfad bzw. ein Hinweis in
  `skipped`; Status unverändert.
- **`open_url` schlägt fehl** (kein Browser): Command gibt `Err`; der Dialog zeigt eine
  Meldung (try/catch um `openUrl`).
- **Bestehende Fälle** (Nicht-XSD, leerer Ordner) wie in Teil 1.

## Test / Verifikation

- `cd app/src-tauri && cargo test` grün (neuer ZIP-Test + bestehende).
- `cd app && npm run check` grün.
- Manuell: „Herunterladen…" öffnet die ebics.de-Seite; ein heruntergeladenes ZIP über
  „XSD/ZIP-Dateien…" importieren → die enthaltenen Schemas erscheinen als „vorhanden".

## Bewusst weggelassen (YAGNI)

- Auto-Download/Scraping ablaufender Token-URLs.
- Rekursives Entpacken von ZIPs in Ordnern; verschachtelte ZIP-in-ZIP.
- Inhaltliche XSD-Prüfung; Auswahl mehrerer Download-Quellen im UI.
- Cross-Plattform-URL-Öffnen (Windows-only, wie `open_schema_dir`).
