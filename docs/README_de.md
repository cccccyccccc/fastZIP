# FastZIP

[English](../README.md) | [简体中文](README_zh.md) | [日本語](README_ja.md) | [한국어](README_ko.md) | [Français](README_fr.md) | **Deutsch** | [Español](README_es.md) | [Italiano](README_it.md) | [Português](README_pt-BR.md) | [Русский](README_ru.md) | [العربية](README_ar.md) | [Türkçe](README_tr.md)

<img src="../assets/fastzip-icon.png" width="64" height="64" align="right">

Ein natives Archiv-Tool in Rust mit GUI und CLI. Komprimieren, entpacken, testen und benchmarken — alles in einer Binary, ohne externe Abhängigkeiten für gängige Formate.

## Funktionen

- **Komprimierung & Extraktion** — 12 Ausgabeformate mit 5 Komprimierungsstufen
- **Archiv-Test** — Einträge durchlaufen und CRC32-Integrität ohne Extrahieren prüfen
- **Performance-Benchmark** — integrierte Benchmark-Suite für alle Format/Level-Kombinationen
- **Selbstextrahierende Archive** — beliebiges Archiv in eigenständige .exe (SFX) verpacken
- **AMSI-Malware-Scan** — optionale Windows Antimalware Scan Interface Integration
- **Hash/Prüfsumme** — SHA-256, BLAKE3, CRC32 für beliebige Dateien
- **Aufgeteilte Volumes** — Multi-Volume-Archive erstellen und lesen (.zip, .7z)
- **Passwortschutz** — AES-verschlüsselte ZIP- und 7z-Archive
- **Codepage-Behandlung** — automatische Erkennung von Nicht-UTF-8-Dateinamen (Shift-JIS, GBK, etc.)
- **Pipe-Unterstützung** — Archive von stdin/stdout lesen/schreiben (`-`)
- **Komprimierungs-Presets** — Einstellungen speichern und wiederverwenden
- **WIM / ISO (schreibgeschützt)** — Dateien in WIM- und ISO 9660-Images auflisten
- **Windows Shell-Integration** — Rechtsklick-Kontextmenü für Komprimieren/Extrahieren
- **Lokalisierung** — 12 Sprachen

## Format-Unterstützung

| Format | Erweiterung | Komprimieren | Extrahieren |
|--------|-------------|-------------|------------|
| ZIP | `.zip` | Ja | Ja |
| 7-Zip | `.7z` | Ja | Ja |
| Tar | `.tar` | Ja | Ja |
| Tar + Gzip | `.tar.gz` `.tgz` | Ja | Ja |
| Tar + Bzip2 | `.tar.bz2` `.tbz2` | Ja | Ja |
| Tar + XZ | `.tar.xz` `.txz` | Ja | Ja |
| Tar + Zstd | `.tar.zst` `.tzst` | Ja | Ja |
| Tar + LZ4 | `.tar.lz4` `.tlz4` | Ja | Ja |
| Gzip | `.gz` | Ja | Ja |
| Bzip2 | `.bz2` | Ja | Ja |
| XZ | `.xz` | Ja | Ja |
| Zstd | `.zst` | Ja | Ja |
| LZ4 | `.lz4` | Ja | Ja |
| RAR | `.rar` | Nein | Ja (via `unrar.exe`) |
| WIM | `.wim` | Nein | Nur Auflistung |
| ISO 9660 | `.iso` | Nein | Ja |

## CLI-Nutzung

Die Haupt-Binary enthält sowohl GUI als auch CLI.

### Archive inspizieren

```powershell
fastzip list archive.zip
fastzip list archive.rar --password secret
fastzip list - < archive.tar.gz --format tar.gz
```

### Extrahieren

```powershell
fastzip extract archive.zip -o ./output
fastzip extract archive.7z --flat --password secret
fastzip extract archive.zip --scan          # AMSI-Scan
fastzip extract archive.zip --codepage 932  # Shift-JIS-Dateinamen
```

### Komprimieren

```powershell
fastzip compress ./folder -o output.zip
fastzip compress ./folder -o output.7z --level maximum
fastzip compress ./folder -o output.zip --sfx            # SFX
fastzip compress ./folder -o output.zip --volume 100M    # Aufgeteilt
fastzip compress ./folder -o output.7z --password secret --encrypt-file-names
fastzip compress ./folder -o output.tar.zst --threads 4
```

### Integritätstest

```powershell
fastzip test archive.zip
fastzip test archive.7z --password secret
```

### Hash / Prüfsumme

```powershell
fastzip checksum file.dat --algo sha256
fastzip checksum file.dat --algo blake3
fastzip checksum file.dat --algo crc32
```

### Benchmark

```powershell
fastzip benchmark -o ./results
```

### Weitere Befehle

```powershell
fastzip formats              # Unterstützte Erweiterungen auflisten
fastzip backends             # Backend-Status anzeigen
```

## Screenshots

![FastZIP Hauptfenster](../images/app.png)

## Aus dem Quellcode bauen

Voraussetzungen: Rust Toolchain (Edition 2024), Windows 10+.

```powershell
git clone https://github.com/cccccyccccc/fastZIP.git
cd fastZIP
cargo build --release
```

Build-Artefakte (`target/release/`):
- `fastzip.exe` — Haupt-GUI + CLI
- `fastzip-cli.exe` — Nur CLI
- `sfx-stub.exe` — SFX-Stub

## Architektur

```
src/
  archive/
    mod.rs       — gemeinsame Typen, Komprimierungs-/Extraktions-Pipeline
    native.rs    — natives Rust-Backend
    rar.rs       — RAR-Adapter (externes unrar.exe)
    service.rs   — Backend-Routing-Fassade
    sfx.rs       — SFX-Builder
    test.rs      — Archiv-Integritätstest
    iso.rs       — ISO 9660-Leser
    wim.rs       — WIM-Metadatenleser
  bin/
    fastzip-cli.rs  — schlanke CLI-Binary
    sfx-stub.rs     — SFX-Stub-Binary
  gui.rs          — native egui/eframe GUI
  amsi.rs         — Windows AMSI-Integration
  benchmark.rs    — Komprimierungs-Benchmark
  hash.rs         — SHA-256 / BLAKE3 / CRC32
  encoding.rs     — Codepage-Erkennung
  localization.rs — 12-Sprachen-Lokalisierung
  settings.rs     — INI-Einstellungen + Presets
```

## Lizenz

GPL-3.0
