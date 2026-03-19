# CLI Reference

## Projekt-Management

| Befehl | Beschreibung |
|--------|-------------|
| `amigo new <name> [--template T]` | Neues Spielprojekt erstellen |
| `amigo scene <name> [--preset P]` | Szene zum Projekt hinzufuegen |
| `amigo info` | Projekt-Infos anzeigen |
| `amigo list-templates` | Verfuegbare Projekt-Templates |
| `amigo list-presets` | Verfuegbare Szenen-Presets |

## Build & Run

| Befehl | Beschreibung |
|--------|-------------|
| `amigo build` | Kompilierung pruefen |
| `amigo run [--headless] [--api]` | Spiel starten |
| `amigo editor` | Level-Editor oeffnen |
| `amigo pack` | Assets in Atlas packen |
| `amigo release [--target T]` | Optimiertes Release-Binary bauen |

## Publishing

| Befehl | Beschreibung |
|--------|-------------|
| `amigo publish steam` | Auf Steam hochladen (via steamcmd) |
| `amigo publish itch [--channel C]` | Auf itch.io hochladen (via butler) |

## Setup (Python-Toolchain)

Siehe [AI Setup](AI-Setup) fuer Details.

| Befehl | Beschreibung |
|--------|-------------|
| `amigo setup` | Volle Installation |
| `amigo setup --only <group>` | Nur bestimmte Tool-Gruppe |
| `amigo setup --gpu <backend>` | GPU-Backend waehlen (cpu/nvidia/mps) |
| `amigo setup --check` | Status anzeigen |
| `amigo setup --update` | Packages aktualisieren |
| `amigo setup --clean [--all]` | Aufraumen |

## Pipeline (Audio-to-TidalCycles)

Siehe [Audio Pipeline](Audio-Pipeline) fuer Details.

| Befehl | Beschreibung |
|--------|-------------|
| `amigo pipeline convert --input F --output F` | Volle Pipeline |
| `amigo pipeline separate --input F --output D` | Nur Stem-Separation |
| `amigo pipeline transcribe --input D --output D` | Nur Audio-to-MIDI |
| `amigo pipeline notate --input D --output F` | Nur MIDI-to-TidalCycles |
| `amigo pipeline batch --input D --output D` | Batch-Verarbeitung |
| `amigo pipeline play <file>` | .amigo.tidal Datei abspielen |

### Gemeinsame Pipeline-Flags

| Flag | Beschreibung |
|------|-------------|
| `--input <path>` | Input-Datei oder Verzeichnis |
| `--output <path>` | Output-Datei oder Verzeichnis |
| `--config <path>` | Pipeline-Konfiguration (TOML) |
| `--bpm <zahl>` | BPM ueberschreiben |
| `--name <text>` | Kompositionsname |
| `--license <text>` | Lizenz-Metadaten |
| `--author <text>` | Autor-Metadaten |

## Utilities

| Befehl | Beschreibung |
|--------|-------------|
| `amigo export-level <path> [--format json]` | Level als JSON exportieren |
