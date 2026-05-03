# FastZIP

[English](../README.md) | [简体中文](README_zh.md) | [日本語](README_ja.md) | [한국어](README_ko.md) | [Français](README_fr.md) | [Deutsch](README_de.md) | [Español](README_es.md) | [Italiano](README_it.md) | [Português](README_pt-BR.md) | **Русский** | [العربية](README_ar.md) | [Türkçe](README_tr.md)

<img src="../assets/fastzip-icon.png" width="64" height="64" align="right">

Нативное средство работы с архивами на Rust с GUI и CLI. Сжатие, распаковка, проверка и бенчмарк — всё в одном бинарном файле, без внешних зависимостей для распространённых форматов.

## Возможности

- **Сжатие и распаковка** — 12 форматов вывода с 5 уровнями сжатия
- **Проверка целостности** — обход записей и проверка CRC32 без распаковки
- **Бенчмарк производительности** — встроенный набор тестов для всех комбинаций форматов и уровней
- **Самораспаковывающиеся архивы** — упаковка любого архива в автономный .exe (SFX)
- **AMSI-сканирование** — опциональная интеграция с антивирусным интерфейсом Windows
- **Хеш/контрольная сумма** — SHA-256, BLAKE3, CRC32 для любого файла
- **Разделение на тома** — создание и чтение многотомных архивов (.zip, .7z)
- **Защита паролем** — ZIP и 7z с AES-шифрованием
- **Обработка кодировок** — автоматическое определение кодировки для не-UTF-8 имён файлов (Shift-JIS, GBK и др.)
- **Поддержка каналов** — чтение/запись архивов из stdin/stdout (`-`)
- **Предустановки сжатия** — сохранение и повторное использование настроек
- **Чтение WIM / ISO** — просмотр файлов в образах WIM и ISO 9660
- **Интеграция с Windows** — контекстное меню для сжатия/распаковки
- **Локализация** — 12 языков

## Поддерживаемые форматы

| Формат | Расширение | Сжатие | Распаковка |
|--------|------------|--------|------------|
| ZIP | `.zip` | Да | Да |
| 7-Zip | `.7z` | Да | Да |
| Tar | `.tar` | Да | Да |
| Tar + Gzip | `.tar.gz` `.tgz` | Да | Да |
| Tar + Bzip2 | `.tar.bz2` `.tbz2` | Да | Да |
| Tar + XZ | `.tar.xz` `.txz` | Да | Да |
| Tar + Zstd | `.tar.zst` `.tzst` | Да | Да |
| Tar + LZ4 | `.tar.lz4` `.tlz4` | Да | Да |
| Gzip | `.gz` | Да | Да |
| Bzip2 | `.bz2` | Да | Да |
| XZ | `.xz` | Да | Да |
| Zstd | `.zst` | Да | Да |
| LZ4 | `.lz4` | Да | Да |
| RAR | `.rar` | Нет | Да (через `unrar.exe`) |
| WIM | `.wim` | Нет | Только список |
| ISO 9660 | `.iso` | Нет | Да |

## Использование CLI

Основной бинарный файл включает и GUI, и CLI.

### Просмотр архива

```powershell
fastzip list archive.zip
fastzip list archive.rar --password secret
fastzip list - < archive.tar.gz --format tar.gz
```

### Распаковка

```powershell
fastzip extract archive.zip -o ./output
fastzip extract archive.7z --flat --password secret
fastzip extract archive.zip --scan          # AMSI-сканирование
fastzip extract archive.zip --codepage 932  # Имена в Shift-JIS
```

### Сжатие

```powershell
fastzip compress ./folder -o output.zip
fastzip compress ./folder -o output.7z --level maximum
fastzip compress ./folder -o output.zip --sfx            # SFX
fastzip compress ./folder -o output.zip --volume 100M    # Тома
fastzip compress ./folder -o output.7z --password secret --encrypt-file-names
fastzip compress ./folder -o output.tar.zst --threads 4
```

### Проверка целостности

```powershell
fastzip test archive.zip
fastzip test archive.7z --password secret
```

### Хеш / контрольная сумма

```powershell
fastzip checksum file.dat --algo sha256
fastzip checksum file.dat --algo blake3
fastzip checksum file.dat --algo crc32
```

### Бенчмарк

```powershell
fastzip benchmark -o ./results
```

### Прочие команды

```powershell
fastzip formats              # Список поддерживаемых расширений
fastzip backends             # Статус бэкендов
```

## Скриншоты

![Главное окно FastZIP](../images/app.png)

## Сборка из исходников

Требования: инструментарий Rust (edition 2024), Windows 10+.

```powershell
git clone https://github.com/cccccyccccc/fastZIP.git
cd fastZIP
cargo build --release
```

Результаты сборки (`target/release/`):
- `fastzip.exe` — основной GUI + CLI
- `fastzip-cli.exe` — только CLI
- `sfx-stub.exe` — SFX-заглушка

## Архитектура

```
src/
  archive/
    mod.rs       — общие типы, конвейер сжатия/распаковки
    native.rs    — нативный Rust-бэкенд
    rar.rs       — адаптер RAR (внешний unrar.exe)
    service.rs   — маршрутизация бэкендов
    sfx.rs       — построитель SFX
    test.rs      — проверка целостности
    iso.rs       — читатель ISO 9660
    wim.rs       — читатель метаданных WIM
  bin/
    fastzip-cli.rs  — облегчённый CLI
    sfx-stub.rs     — SFX-заглушка
  gui.rs          — нативный GUI на egui/eframe
  amsi.rs         — интеграция Windows AMSI
  benchmark.rs    — набор бенчмарков
  hash.rs         — SHA-256 / BLAKE3 / CRC32
  encoding.rs     — определение кодировок
  localization.rs — локализация на 12 языков
  settings.rs     — настройки INI + предустановки
```

## Лицензия

GPL-3.0
