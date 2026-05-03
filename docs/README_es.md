# FastZIP

[English](../README.md) | [简体中文](README_zh.md) | [日本語](README_ja.md) | [한국어](README_ko.md) | [Français](README_fr.md) | [Deutsch](README_de.md) | **Español** | [Italiano](README_it.md) | [Português](README_pt-BR.md) | [Русский](README_ru.md) | [العربية](README_ar.md) | [Türkçe](README_tr.md)

<img src="../assets/fastzip-icon.png" width="64" height="64" align="right">

Herramienta de archivos nativa en Rust con GUI y CLI. Comprime, extrae, prueba y evalúa — todo en un solo binario sin dependencias externas para formatos comunes.

## Funciones

- **Compresión y extracción** — 12 formatos de salida con 5 niveles de compresión
- **Prueba de integridad** — recorre entradas y verifica CRC32 sin extraer
- **Benchmark de rendimiento** — suite de benchmark integrada para todas las combinaciones formato/nivel
- **Archivos autoextraíbles** — convierte cualquier archivo en .exe independiente (SFX)
- **Análisis AMSI** — integración opcional con la interfaz antimalware de Windows
- **Hash/checksum** — SHA-256, BLAKE3, CRC32 para cualquier archivo
- **Volúmenes divididos** — crea y lee archivos multivolumen (.zip, .7z)
- **Protección por contraseña** — archivos ZIP y 7z con cifrado AES
- **Manejo de codificación** — detección automática de nombres de archivo no UTF-8 (Shift-JIS, GBK, etc.)
- **Soporte de tuberías** — leer/escribir archivos desde stdin/stdout (`-`)
- **Preajustes de compresión** — guarda y reutiliza configuraciones
- **Lectura WIM / ISO** — lista archivos dentro de imágenes WIM e ISO 9660
- **Integración con Windows** — menú contextual para comprimir/extraer
- **Localización** — 12 idiomas

## Formatos soportados

| Formato | Extensión | Comprimir | Extraer |
|---------|-----------|-----------|---------|
| ZIP | `.zip` | Sí | Sí |
| 7-Zip | `.7z` | Sí | Sí |
| Tar | `.tar` | Sí | Sí |
| Tar + Gzip | `.tar.gz` `.tgz` | Sí | Sí |
| Tar + Bzip2 | `.tar.bz2` `.tbz2` | Sí | Sí |
| Tar + XZ | `.tar.xz` `.txz` | Sí | Sí |
| Tar + Zstd | `.tar.zst` `.tzst` | Sí | Sí |
| Tar + LZ4 | `.tar.lz4` `.tlz4` | Sí | Sí |
| Gzip | `.gz` | Sí | Sí |
| Bzip2 | `.bz2` | Sí | Sí |
| XZ | `.xz` | Sí | Sí |
| Zstd | `.zst` | Sí | Sí |
| LZ4 | `.lz4` | Sí | Sí |
| RAR | `.rar` | No | Sí (via `unrar.exe`) |
| WIM | `.wim` | No | Solo lista |
| ISO 9660 | `.iso` | No | Sí |

## Uso de CLI

El binario principal incluye tanto GUI como CLI.

### Inspeccionar archivos

```powershell
fastzip list archive.zip
fastzip list archive.rar --password secret
fastzip list - < archive.tar.gz --format tar.gz
```

### Extraer

```powershell
fastzip extract archive.zip -o ./output
fastzip extract archive.7z --flat --password secret
fastzip extract archive.zip --scan          # Análisis AMSI
fastzip extract archive.zip --codepage 932  # Nombres Shift-JIS
```

### Comprimir

```powershell
fastzip compress ./folder -o output.zip
fastzip compress ./folder -o output.7z --level maximum
fastzip compress ./folder -o output.zip --sfx            # Autoextraíble
fastzip compress ./folder -o output.zip --volume 100M    # Volúmenes
fastzip compress ./folder -o output.7z --password secret --encrypt-file-names
fastzip compress ./folder -o output.tar.zst --threads 4
```

### Prueba de integridad

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

### Otros comandos

```powershell
fastzip formats              # Listar extensiones soportadas
fastzip backends             # Mostrar estado de backends
```

## Capturas de pantalla

![Ventana principal de FastZIP](../images/app.png)

## Compilar desde el código fuente

Requisitos: cadena de herramientas Rust (edition 2024), Windows 10+.

```powershell
git clone https://github.com/cccccyccccc/fastZIP.git
cd fastZIP
cargo build --release
```

Artefactos de compilación (`target/release/`):
- `fastzip.exe` — GUI + CLI principal
- `fastzip-cli.exe` — solo CLI
- `sfx-stub.exe` — stub autoextraíble

## Arquitectura

```
src/
  archive/
    mod.rs       — tipos compartidos, pipeline de compresión/extracción
    native.rs    — backend nativo Rust
    rar.rs       — adaptador RAR (unrar.exe externo)
    service.rs   — fachada de enrutamiento de backends
    sfx.rs       — constructor de SFX
    test.rs      — prueba de integridad de archivos
    iso.rs       — lector ISO 9660
    wim.rs       — lector de metadatos WIM
  bin/
    fastzip-cli.rs  — binario CLI ligero
    sfx-stub.rs     — binario stub SFX
  gui.rs          — GUI nativa egui/eframe
  amsi.rs         — integración Windows AMSI
  benchmark.rs    — suite de benchmark
  hash.rs         — SHA-256 / BLAKE3 / CRC32
  encoding.rs     — detección de codificación
  localization.rs — localización en 12 idiomas
  settings.rs     — configuración INI + preajustes
```

## Licencia

GPL-3.0
