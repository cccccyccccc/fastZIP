# FastZIP

[English](../README.md) | [简体中文](README_zh.md) | **日本語** | [한국어](README_ko.md) | [Français](README_fr.md) | [Deutsch](README_de.md) | [Español](README_es.md) | [Italiano](README_it.md) | [Português](README_pt-BR.md) | [Русский](README_ru.md) | [العربية](README_ar.md) | [Türkçe](README_tr.md)

<img src="../assets/fastzip-icon.png" width="64" height="64" align="right">

Rust 製のネイティブアーカイブツール。GUI と CLI を両方備え、圧縮、解凍、テスト、ベンチマークを単一バイナリで実行。一般的な形式では外部依存なし。

## 機能

- **圧縮と解凍** — 12 種類の出力形式、5 段階の圧縮レベル
- **アーカイブテスト** — 解凍せずにエントリを走査し CRC32 の整合性を検証
- **パフォーマンスベンチマーク** — 全形式・全レベルを網羅した内蔵ベンチマーク
- **自己解凍アーカイブ** — 任意のアーカイブをスタンドアロン .exe（SFX）に変換
- **AMSI マルウェアスキャン** — Windows マルウェア対策スキャンインターフェースと統合
- **ハッシュ/チェックサム** — SHA-256、BLAKE3、CRC32
- **分割ボリューム** — マルチボリュームアーカイブの作成と読み取り（.zip, .7z）
- **パスワード保護** — AES 暗号化 ZIP および 7z
- **コードページ処理** — 非 UTF-8 ファイル名の自動エンコーディング検出（Shift-JIS, GBK 等）
- **パイプ対応** — stdin/stdout からの読み書き（`-`）
- **圧縮プリセット** — 圧縮設定の保存と再利用
- **WIM / ISO 読み取り** — WIM および ISO 9660 イメージ内のファイル一覧表示
- **Windows シェル統合** — 右クリックメニューで圧縮/解凍
- **多言語対応** — 12 言語

## 対応形式

| 形式 | 拡張子 | 圧縮 | 解凍 |
|------|--------|------|------|
| ZIP | `.zip` | 可 | 可 |
| 7-Zip | `.7z` | 可 | 可 |
| Tar | `.tar` | 可 | 可 |
| Tar + Gzip | `.tar.gz` `.tgz` | 可 | 可 |
| Tar + Bzip2 | `.tar.bz2` `.tbz2` | 可 | 可 |
| Tar + XZ | `.tar.xz` `.txz` | 可 | 可 |
| Tar + Zstd | `.tar.zst` `.tzst` | 可 | 可 |
| Tar + LZ4 | `.tar.lz4` `.tlz4` | 可 | 可 |
| Gzip | `.gz` | 可 | 可 |
| Bzip2 | `.bz2` | 可 | 可 |
| XZ | `.xz` | 可 | 可 |
| Zstd | `.zst` | 可 | 可 |
| LZ4 | `.lz4` | 可 | 可 |
| RAR | `.rar` | 不可 | 可（要 `unrar.exe`） |
| WIM | `.wim` | 不可 | 一覧のみ |
| ISO 9660 | `.iso` | 不可 | 可 |

## CLI の使い方

メインバイナリは GUI と CLI の両方を含みます。

### アーカイブの内容表示

```powershell
fastzip list archive.zip
fastzip list archive.rar --password secret
fastzip list - < archive.tar.gz --format tar.gz
```

### 解凍

```powershell
fastzip extract archive.zip -o ./output
fastzip extract archive.7z --flat --password secret
fastzip extract archive.zip --scan          # AMSI スキャン
fastzip extract archive.zip --codepage 932  # Shift-JIS ファイル名
```

### 圧縮

```powershell
fastzip compress ./folder -o output.zip
fastzip compress ./folder -o output.7z --level maximum
fastzip compress ./folder -o output.zip --sfx            # 自己解凍
fastzip compress ./folder -o output.zip --volume 100M    # 分割
fastzip compress ./folder -o output.7z --password secret --encrypt-file-names
fastzip compress ./folder -o output.tar.zst --threads 4
```

### 整合性テスト

```powershell
fastzip test archive.zip
fastzip test archive.7z --password secret
```

### ハッシュ/チェックサム

```powershell
fastzip checksum file.dat --algo sha256
fastzip checksum file.dat --algo blake3
fastzip checksum file.dat --algo crc32
```

### ベンチマーク

```powershell
fastzip benchmark -o ./results
```

### その他

```powershell
fastzip formats              # 対応拡張子の一覧
fastzip backends             # バックエンドの状態表示
```

## スクリーンショット

![FastZIP メイン画面](../images/app.png)

## ソースからビルド

要件: Rust ツールチェーン（edition 2024）、Windows 10+。

```powershell
git clone https://github.com/cccccyccccc/fastZIP.git
cd fastZIP
cargo build --release
```

ビルド成果物（`target/release/`）:
- `fastzip.exe` — メイン GUI + CLI
- `fastzip-cli.exe` — CLI 専用
- `sfx-stub.exe` — 自己解凍スタブ

## アーキテクチャ

```
src/
  archive/
    mod.rs       — 共有型、圧縮/解凍パイプライン
    native.rs    — ネイティブ Rust バックエンド
    rar.rs       — RAR アダプター（外部 unrar.exe）
    service.rs   — バックエンドルーティング
    sfx.rs       — 自己解凍ビルダー
    test.rs      — アーカイブ整合性テスト
    iso.rs       — ISO 9660 リーダー
    wim.rs       — WIM メタデータリーダー
  bin/
    fastzip-cli.rs  — CLI 専用バイナリ
    sfx-stub.rs     — SFX スタブバイナリ
  gui.rs          — egui/eframe ネイティブ GUI
  amsi.rs         — Windows AMSI 統合
  benchmark.rs    — 圧縮ベンチマーク
  hash.rs         — SHA-256 / BLAKE3 / CRC32
  encoding.rs     — コードページ検出と変換
  localization.rs — 12 言語ローカリゼーション
  settings.rs     — INI 設定 + 圧縮プリセット
```

## ライセンス

GPL-3.0
