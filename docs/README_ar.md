# FastZIP

[English](../README.md) | [简体中文](README_zh.md) | [日本語](README_ja.md) | [한국어](README_ko.md) | [Français](README_fr.md) | [Deutsch](README_de.md) | [Español](README_es.md) | [Italiano](README_it.md) | [Português](README_pt-BR.md) | [Русский](README_ru.md) | **العربية** | [Türkçe](README_tr.md)

<img src="../assets/fastzip-icon.png" width="64" height="64" align="right">

أداة أرشفة مبنية بلغة Rust مع واجهة رسومية وسطر أوامر. ضغط وفك ضغط واختبار وقياس أداء — كل ذلك في برنامج واحد دون اعتماديات خارجية للصيغ الشائعة.

## الميزات

- **الضغط وفك الضغط** — 12 صيغة إخراج مع 5 مستويات ضغط
- **اختبار الأرشيف** — فحص الإدخالات والتحقق من سلامة CRC32 دون فك الضغط
- **قياس الأداء** — مجموعة اختبارات أداء مدمجة لجميع تركيبات الصيغ والمستويات
- **أرشيفات ذاتية الاستخراج** — تحويل أي أرشيف إلى ملف .exe مستقل (SFX)
- **فحص AMSI** — تكامل اختياري مع واجهة فحص البرمجيات الخبيثة في Windows
- **التجزئة/المجموع الاختباري** — SHA-256 و BLAKE3 و CRC32 لأي ملف
- **الأحجام المقسمة** — إنشاء وقراءة أرشيفات متعددة الأجزاء (.zip, .7z)
- **الحماية بكلمة مرور** — أرشيفات ZIP و 7z بتشفير AES
- **معالجة الترميز** — اكتشاف تلقائي لترميز أسماء الملفات غير UTF-8 (Shift-JIS, GBK وغيرها)
- **دعم الأنابيب** — قراءة/كتابة الأرشيفات من stdin/stdout (`-`)
- **إعدادات الضغط المسبقة** — حفظ وإعادة استخدام إعدادات الضغط
- **قراءة WIM / ISO** — عرض الملفات داخل صور WIM و ISO 9660
- **تكامل مع Windows** — قائمة سياق للضغط وفك الضغط
- **التعريب** — 12 لغة

## الصيغ المدعومة

| الصيغة | الامتداد | الضغط | فك الضغط |
|--------|----------|-------|----------|
| ZIP | `.zip` | نعم | نعم |
| 7-Zip | `.7z` | نعم | نعم |
| Tar | `.tar` | نعم | نعم |
| Tar + Gzip | `.tar.gz` `.tgz` | نعم | نعم |
| Tar + Bzip2 | `.tar.bz2` `.tbz2` | نعم | نعم |
| Tar + XZ | `.tar.xz` `.txz` | نعم | نعم |
| Tar + Zstd | `.tar.zst` `.tzst` | نعم | نعم |
| Tar + LZ4 | `.tar.lz4` `.tlz4` | نعم | نعم |
| Gzip | `.gz` | نعم | نعم |
| Bzip2 | `.bz2` | نعم | نعم |
| XZ | `.xz` | نعم | نعم |
| Zstd | `.zst` | نعم | نعم |
| LZ4 | `.lz4` | نعم | نعم |
| RAR | `.rar` | لا | نعم (بواسطة `unrar.exe`) |
| WIM | `.wim` | لا | قائمة فقط |
| ISO 9660 | `.iso` | لا | نعم |

## استخدام سطر الأوامر

البرنامج الرئيسي يتضمن الواجهة الرسومية وسطر الأوامر معاً.

### عرض محتويات الأرشيف

```powershell
fastzip list archive.zip
fastzip list archive.rar --password secret
fastzip list - < archive.tar.gz --format tar.gz
```

### فك الضغط

```powershell
fastzip extract archive.zip -o ./output
fastzip extract archive.7z --flat --password secret
fastzip extract archive.zip --scan          # فحص AMSI
fastzip extract archive.zip --codepage 932  # أسماء Shift-JIS
```

### الضغط

```powershell
fastzip compress ./folder -o output.zip
fastzip compress ./folder -o output.7z --level maximum
fastzip compress ./folder -o output.zip --sfx            # ذاتي الاستخراج
fastzip compress ./folder -o output.zip --volume 100M    # مقسم
fastzip compress ./folder -o output.7z --password secret --encrypt-file-names
fastzip compress ./folder -o output.tar.zst --threads 4
```

### اختبار السلامة

```powershell
fastzip test archive.zip
fastzip test archive.7z --password secret
```

### التجزئة / المجموع الاختباري

```powershell
fastzip checksum file.dat --algo sha256
fastzip checksum file.dat --algo blake3
fastzip checksum file.dat --algo crc32
```

### قياس الأداء

```powershell
fastzip benchmark -o ./results
```

### أوامر أخرى

```powershell
fastzip formats              # عرض الامتدادات المدعومة
fastzip backends             # عرض حالة الخلفيات
```

## لقطات الشاشة

![نافذة FastZIP الرئيسية](../images/app.png)

## البناء من المصدر

المتطلبات: سلسلة أدوات Rust (edition 2024)، Windows 10+.

```powershell
git clone https://github.com/cccccyccccc/fastZIP.git
cd fastZIP
cargo build --release
```

مخرجات البناء (`target/release/`):
- `fastzip.exe` — البرنامج الرئيسي (واجهة + CLI)
- `fastzip-cli.exe` — CLI فقط
- `sfx-stub.exe` — كعب ذاتي الاستخراج

## الهندسة المعمارية

```
src/
  archive/
    mod.rs       — الأنواع المشتركة وخط أنابيب الضغط/فك الضغط
    native.rs    — الخلفية الأصلية بلغة Rust
    rar.rs       — مهايئ RAR (unrar.exe خارجي)
    service.rs   — واجهة توجيه الخلفيات
    sfx.rs       — منشئ الأرشيفات ذاتية الاستخراج
    test.rs      — اختبار سلامة الأرشيف
    iso.rs       — قارئ ISO 9660
    wim.rs       — قارئ بيانات WIM الوصفية
  bin/
    fastzip-cli.rs  — برنامج CLI خفيف
    sfx-stub.rs     — كعب SFX
  gui.rs          — واجهة رسومية بـ egui/eframe
  amsi.rs         — تكامل Windows AMSI
  benchmark.rs    — مجموعة اختبارات الأداء
  hash.rs         — SHA-256 / BLAKE3 / CRC32
  encoding.rs     — اكتشاف وتحويل الترميز
  localization.rs — تعريب 12 لغة
  settings.rs     — إعدادات INI + الإعدادات المسبقة
```

## الرخصة

GPL-3.0
