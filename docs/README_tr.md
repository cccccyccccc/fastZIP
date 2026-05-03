# FastZIP

[English](../README.md) | [简体中文](README_zh.md) | [日本語](README_ja.md) | [한국어](README_ko.md) | [Français](README_fr.md) | [Deutsch](README_de.md) | [Español](README_es.md) | [Italiano](README_it.md) | [Português](README_pt-BR.md) | [Русский](README_ru.md) | [العربية](README_ar.md) | **Türkçe**

<img src="../assets/fastzip-icon.png" width="64" height="64" align="right">

Rust ile yazılmış, GUI ve CLI sunan yerel bir arşiv aracı. Sıkıştırma, çıkarma, test ve benchmark — yaygın formatlar için harici bağımlılık olmadan tek bir binary'de.

## Özellikler

- **Sıkıştırma ve çıkarma** — 5 sıkıştırma seviyesi ile 12 çıktı formatı
- **Arşiv testi** — çıkarmadan girişleri tara ve CRC32 bütünlüğünü doğrula
- **Performans benchmark'ı** — tüm format/seviye kombinasyonları için dahili benchmark
- **Kendiliğinden çıkan arşivler** — herhangi bir arşivi bağımsız .exe'ye (SFX) dönüştür
- **AMSI kötü amaçlı yazılım taraması** — isteğe bağlı Windows Antimalware Tarama Arayüzü entegrasyonu
- **Hash/sağlama** — herhangi bir dosya için SHA-256, BLAKE3, CRC32
- **Bölünmüş birimler** — çok birimli arşivler oluştur ve oku (.zip, .7z)
- **Parola koruması** — AES şifreli ZIP ve 7z arşivleri
- **Kod sayfası yönetimi** — UTF-8 olmayan dosya adları için otomatik kodlama algılama (Shift-JIS, GBK vb.)
- **Boru desteği** — stdin/stdout'tan arşiv oku/yaz (`-`)
- **Sıkıştırma ön ayarları** — ayarları kaydet ve yeniden kullan
- **WIM / ISO okuma** — WIM ve ISO 9660 imajlarının içindeki dosyaları listele
- **Windows kabuk entegrasyonu** — sağ tık menüsü ile sıkıştır/çıkar
- **Yerelleştirme** — 12 dil

## Desteklenen formatlar

| Format | Uzantı | Sıkıştırma | Çıkarma |
|--------|--------|------------|---------|
| ZIP | `.zip` | Evet | Evet |
| 7-Zip | `.7z` | Evet | Evet |
| Tar | `.tar` | Evet | Evet |
| Tar + Gzip | `.tar.gz` `.tgz` | Evet | Evet |
| Tar + Bzip2 | `.tar.bz2` `.tbz2` | Evet | Evet |
| Tar + XZ | `.tar.xz` `.txz` | Evet | Evet |
| Tar + Zstd | `.tar.zst` `.tzst` | Evet | Evet |
| Tar + LZ4 | `.tar.lz4` `.tlz4` | Evet | Evet |
| Gzip | `.gz` | Evet | Evet |
| Bzip2 | `.bz2` | Evet | Evet |
| XZ | `.xz` | Evet | Evet |
| Zstd | `.zst` | Evet | Evet |
| LZ4 | `.lz4` | Evet | Evet |
| RAR | `.rar` | Hayır | Evet (`unrar.exe` ile) |
| WIM | `.wim` | Hayır | Sadece listeleme |
| ISO 9660 | `.iso` | Hayır | Evet |

## CLI kullanımı

Ana binary hem GUI hem CLI içerir.

### Arşivleri inceleme

```powershell
fastzip list archive.zip
fastzip list archive.rar --password secret
fastzip list - < archive.tar.gz --format tar.gz
```

### Çıkarma

```powershell
fastzip extract archive.zip -o ./output
fastzip extract archive.7z --flat --password secret
fastzip extract archive.zip --scan          # AMSI taraması
fastzip extract archive.zip --codepage 932  # Shift-JIS dosya adları
```

### Sıkıştırma

```powershell
fastzip compress ./folder -o output.zip
fastzip compress ./folder -o output.7z --level maximum
fastzip compress ./folder -o output.zip --sfx            # SFX
fastzip compress ./folder -o output.zip --volume 100M    # Bölünmüş
fastzip compress ./folder -o output.7z --password secret --encrypt-file-names
fastzip compress ./folder -o output.tar.zst --threads 4
```

### Bütünlük testi

```powershell
fastzip test archive.zip
fastzip test archive.7z --password secret
```

### Hash / sağlama

```powershell
fastzip checksum file.dat --algo sha256
fastzip checksum file.dat --algo blake3
fastzip checksum file.dat --algo crc32
```

### Benchmark

```powershell
fastzip benchmark -o ./results
```

### Diğer komutlar

```powershell
fastzip formats              # Desteklenen uzantıları listele
fastzip backends             # Arka uç durumunu göster
```

## Ekran görüntüleri

![FastZIP ana penceresi](../images/app.png)

## Kaynaktan derleme

Gereksinimler: Rust araç zinciri (edition 2024), Windows 10+.

```powershell
git clone https://github.com/cccccyccccc/fastZIP.git
cd fastZIP
cargo build --release
```

Derleme çıktıları (`target/release/`):
- `fastzip.exe` — ana GUI + CLI
- `fastzip-cli.exe` — sadece CLI
- `sfx-stub.exe` — SFX saplaması

## Mimari

```
src/
  archive/
    mod.rs       — ortak tipler, sıkıştırma/çıkarma hattı
    native.rs    — yerel Rust arka ucu
    rar.rs       — RAR bağdaştırıcısı (harici unrar.exe)
    service.rs   — arka uç yönlendirme
    sfx.rs       — SFX oluşturucu
    test.rs      — arşiv bütünlük testi
    iso.rs       — ISO 9660 okuyucu
    wim.rs       — WIM üst veri okuyucu
  bin/
    fastzip-cli.rs  — hafif CLI binary'si
    sfx-stub.rs     — SFX saplama binary'si
  gui.rs          — egui/eframe yerel GUI
  amsi.rs         — Windows AMSI entegrasyonu
  benchmark.rs    — sıkıştırma benchmark'ı
  hash.rs         — SHA-256 / BLAKE3 / CRC32
  encoding.rs     — kod sayfası algılama
  localization.rs — 12 dilli yerelleştirme
  settings.rs     — INI ayarları + ön ayarlar
```

## Lisans

GPL-3.0
