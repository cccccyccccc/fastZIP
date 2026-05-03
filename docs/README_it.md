# FastZIP

[English](../README.md) | [简体中文](README_zh.md) | [日本語](README_ja.md) | [한국어](README_ko.md) | [Français](README_fr.md) | [Deutsch](README_de.md) | [Español](README_es.md) | **Italiano** | [Português](README_pt-BR.md) | [Русский](README_ru.md) | [العربية](README_ar.md) | [Türkçe](README_tr.md)

<img src="../assets/fastzip-icon.png" width="64" height="64" align="right">

Strumento di archiviazione nativo in Rust con GUI e CLI. Comprimi, estrai, testa e benchmark — tutto in un unico binario senza dipendenze esterne per i formati comuni.

## Funzionalità

- **Compressione ed estrazione** — 12 formati di output con 5 livelli di compressione
- **Test di integrità** — analizza le voci e verifica il CRC32 senza estrarre
- **Benchmark delle prestazioni** — suite di benchmark integrata per tutte le combinazioni formato/livello
- **Archivi autoestraenti** — converti qualsiasi archivio in un .exe autonomo (SFX)
- **Scansione malware AMSI** — integrazione opzionale con l'interfaccia antimalware di Windows
- **Hash/checksum** — SHA-256, BLAKE3, CRC32 per qualsiasi file
- **Volumi divisi** — crea e leggi archivi multi-volume (.zip, .7z)
- **Protezione con password** — archivi ZIP e 7z crittografati AES
- **Gestione codepage** — rilevamento automatico della codifica per nomi file non UTF-8 (Shift-JIS, GBK, ecc.)
- **Supporto pipe** — leggi/scrivi archivi da stdin/stdout (`-`)
- **Preimpostazioni di compressione** — salva e riutilizza le impostazioni
- **Lettura WIM / ISO** — elenca i file all'interno di immagini WIM e ISO 9660
- **Integrazione shell Windows** — menu contestuale per comprimere/estrarre
- **Localizzazione** — 12 lingue

## Formati supportati

| Formato | Estensione | Comprimi | Estrai |
|---------|------------|----------|--------|
| ZIP | `.zip` | Sì | Sì |
| 7-Zip | `.7z` | Sì | Sì |
| Tar | `.tar` | Sì | Sì |
| Tar + Gzip | `.tar.gz` `.tgz` | Sì | Sì |
| Tar + Bzip2 | `.tar.bz2` `.tbz2` | Sì | Sì |
| Tar + XZ | `.tar.xz` `.txz` | Sì | Sì |
| Tar + Zstd | `.tar.zst` `.tzst` | Sì | Sì |
| Tar + LZ4 | `.tar.lz4` `.tlz4` | Sì | Sì |
| Gzip | `.gz` | Sì | Sì |
| Bzip2 | `.bz2` | Sì | Sì |
| XZ | `.xz` | Sì | Sì |
| Zstd | `.zst` | Sì | Sì |
| LZ4 | `.lz4` | Sì | Sì |
| RAR | `.rar` | No | Sì (via `unrar.exe`) |
| WIM | `.wim` | No | Solo elenco |
| ISO 9660 | `.iso` | No | Sì |

## Utilizzo CLI

Il binario principale include sia GUI che CLI.

### Ispezionare archivi

```powershell
fastzip list archive.zip
fastzip list archive.rar --password secret
fastzip list - < archive.tar.gz --format tar.gz
```

### Estrarre

```powershell
fastzip extract archive.zip -o ./output
fastzip extract archive.7z --flat --password secret
fastzip extract archive.zip --scan          # Scansione AMSI
fastzip extract archive.zip --codepage 932  # Nomi file Shift-JIS
```

### Comprimere

```powershell
fastzip compress ./folder -o output.zip
fastzip compress ./folder -o output.7z --level maximum
fastzip compress ./folder -o output.zip --sfx            # Autoestraente
fastzip compress ./folder -o output.zip --volume 100M    # Volumi
fastzip compress ./folder -o output.7z --password secret --encrypt-file-names
fastzip compress ./folder -o output.tar.zst --threads 4
```

### Test di integrità

```powershell
fastzip test archive.zip
fastzip test archive.7z --password secret
```

### Hash / checksum

```powershell
fastzip checksum file.dat --algo sha256
fastzip checksum file.dat --algo blake3
fastzip checksum file.dat --algo crc32
```

### Benchmark

```powershell
fastzip benchmark -o ./results
```

### Altri comandi

```powershell
fastzip formats              # Elenca estensioni supportate
fastzip backends             # Mostra stato dei backend
```

## Screenshot

![Finestra principale FastZIP](../images/app.png)

## Compilazione dai sorgenti

Requisiti: toolchain Rust (edition 2024), Windows 10+.

```powershell
git clone https://github.com/cccccyccccc/fastZIP.git
cd fastZIP
cargo build --release
```

Artefatti di build (`target/release/`):
- `fastzip.exe` — GUI + CLI principale
- `fastzip-cli.exe` — solo CLI
- `sfx-stub.exe` — stub autoestraente

## Architettura

```
src/
  archive/
    mod.rs       — tipi condivisi, pipeline di compressione/estrazione
    native.rs    — backend nativo Rust
    rar.rs       — adattatore RAR (unrar.exe esterno)
    service.rs   — facade di routing dei backend
    sfx.rs       — builder di archivi autoestraenti
    test.rs      — test di integrità archivi
    iso.rs       — lettore ISO 9660
    wim.rs       — lettore metadati WIM
  bin/
    fastzip-cli.rs  — binario CLI leggero
    sfx-stub.rs     — binario stub SFX
  gui.rs          — GUI nativa egui/eframe
  amsi.rs         — integrazione Windows AMSI
  benchmark.rs    — suite di benchmark
  hash.rs         — SHA-256 / BLAKE3 / CRC32
  encoding.rs     — rilevamento codepage
  localization.rs — localizzazione 12 lingue
  settings.rs     — impostazioni INI + preimpostazioni
```

## Licenza

GPL-3.0
