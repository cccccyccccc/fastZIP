# FastZIP

[English](../README.md) | [简体中文](README_zh.md) | [日本語](README_ja.md) | [한국어](README_ko.md) | **Français** | [Deutsch](README_de.md) | [Español](README_es.md) | [Italiano](README_it.md) | [Português](README_pt-BR.md) | [Русский](README_ru.md) | [العربية](README_ar.md) | [Türkçe](README_tr.md)

<img src="../assets/fastzip-icon.png" width="64" height="64" align="right">

Outil d'archive natif en Rust avec GUI et CLI. Compressez, extrayez, testez et benchmarkez — un seul binaire, sans dépendances externes pour les formats courants.

## Fonctionnalités

- **Compression et extraction** — 12 formats de sortie avec 5 niveaux de compression
- **Test d'intégrité** — parcourez les entrées et vérifiez l'intégrité CRC32 sans extraire
- **Benchmark de performance** — suite de benchmark intégrée pour toutes les combinaisons format/niveau
- **Archives auto-extractibles** — transformez toute archive en .exe autonome (SFX)
- **Analyse AMSI** — intégration optionnelle avec l'interface antimalware Windows
- **Hachage/somme de contrôle** — SHA-256, BLAKE3, CRC32 pour tout fichier
- **Volumes fractionnés** — créez et lisez des archives multi-volumes (.zip, .7z)
- **Protection par mot de passe** — archives ZIP et 7z chiffrées en AES
- **Gestion des pages de code** — détection automatique de l'encodage des noms de fichiers non UTF-8 (Shift-JIS, GBK, etc.)
- **Support des pipes** — lecture/écriture d'archives depuis stdin/stdout (`-`)
- **Préréglages de compression** — sauvegardez et réutilisez les paramètres de compression
- **Lecture WIM / ISO** — listez les fichiers dans les images WIM et ISO 9660
- **Intégration Windows** — menu contextuel pour compresser/extraire
- **Localisation** — 12 langues

## Formats supportés

| Format | Extension | Compression | Extraction |
|--------|-----------|-------------|------------|
| ZIP | `.zip` | Oui | Oui |
| 7-Zip | `.7z` | Oui | Oui |
| Tar | `.tar` | Oui | Oui |
| Tar + Gzip | `.tar.gz` `.tgz` | Oui | Oui |
| Tar + Bzip2 | `.tar.bz2` `.tbz2` | Oui | Oui |
| Tar + XZ | `.tar.xz` `.txz` | Oui | Oui |
| Tar + Zstd | `.tar.zst` `.tzst` | Oui | Oui |
| Tar + LZ4 | `.tar.lz4` `.tlz4` | Oui | Oui |
| Gzip | `.gz` | Oui | Oui |
| Bzip2 | `.bz2` | Oui | Oui |
| XZ | `.xz` | Oui | Oui |
| Zstd | `.zst` | Oui | Oui |
| LZ4 | `.lz4` | Oui | Oui |
| RAR | `.rar` | Non | Oui (via `unrar.exe`) |
| WIM | `.wim` | Non | Liste seulement |
| ISO 9660 | `.iso` | Non | Oui |

## Utilisation CLI

Le binaire principal inclut à la fois l'interface graphique et la CLI.

### Inspecter les archives

```powershell
fastzip list archive.zip
fastzip list archive.rar --password secret
fastzip list - < archive.tar.gz --format tar.gz
```

### Extraire

```powershell
fastzip extract archive.zip -o ./output
fastzip extract archive.7z --flat --password secret
fastzip extract archive.zip --scan          # Analyse AMSI
fastzip extract archive.zip --codepage 932  # Noms en Shift-JIS
```

### Compresser

```powershell
fastzip compress ./folder -o output.zip
fastzip compress ./folder -o output.7z --level maximum
fastzip compress ./folder -o output.zip --sfx            # Auto-extractible
fastzip compress ./folder -o output.zip --volume 100M    # Volumes fractionnés
fastzip compress ./folder -o output.7z --password secret --encrypt-file-names
fastzip compress ./folder -o output.tar.zst --threads 4
```

### Tester l'intégrité

```powershell
fastzip test archive.zip
fastzip test archive.7z --password secret
```

### Hachage

```powershell
fastzip checksum file.dat --algo sha256
fastzip checksum file.dat --algo blake3
fastzip checksum file.dat --algo crc32
```

### Benchmark

```powershell
fastzip benchmark -o ./results
```

### Autres commandes

```powershell
fastzip formats              # Lister les extensions supportées
fastzip backends             # Afficher l'état des backends
```

## Captures d'écran

![Fenêtre principale FastZIP](../images/app.png)

## Compilation depuis les sources

Prérequis : chaîne d'outils Rust (edition 2024), Windows 10+.

```powershell
git clone https://github.com/cccccyccccc/fastZIP.git
cd fastZIP
cargo build --release
```

Produits de compilation (`target/release/`) :
- `fastzip.exe` — GUI + CLI principal
- `fastzip-cli.exe` — CLI uniquement
- `sfx-stub.exe` — stub auto-extractible

## Architecture

```
src/
  archive/
    mod.rs       — types partagés, pipeline de compression/extraction
    native.rs    — backend natif Rust
    rar.rs       — adaptateur RAR (unrar.exe externe)
    service.rs   — façade de routage des backends
    sfx.rs       — constructeur d'archives auto-extractibles
    test.rs      — test d'intégrité des archives
    iso.rs       — lecteur ISO 9660
    wim.rs       — lecteur de métadonnées WIM
  bin/
    fastzip-cli.rs  — binaire CLI léger
    sfx-stub.rs     — binaire stub SFX
  gui.rs          — GUI native egui/eframe
  amsi.rs         — intégration Windows AMSI
  benchmark.rs    — suite de benchmark de compression
  hash.rs         — SHA-256 / BLAKE3 / CRC32
  encoding.rs     — détection et conversion de pages de code
  localization.rs — localisation 12 langues
  settings.rs     — paramètres INI + préréglages
```

## Licence

GPL-3.0
