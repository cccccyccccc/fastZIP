# FastZIP

[English](../README.md) | [简体中文](README_zh.md)

<img src="../assets/fastzip-icon.png" width="64" height="64" align="right">

原生 Rust 压缩工具，同时提供 GUI 和 CLI。压缩、解压、测试、基准测试 — 一个二进制文件搞定，常见格式无需任何外部依赖。

## 功能

- **压缩与解压** — 12 种输出格式，5 级压缩强度
- **完整性测试** — 无需解压即可遍历条目并验证 CRC32 完整性
- **性能基准测试** — 内置基准测试套件，覆盖所有格式与级别组合
- **自解压程序** — 将任意压缩包打包为独立的 .exe（SFX）
- **AMSI 恶意软件扫描** — 可选集成 Windows 反恶意软件扫描接口，解压时自动扫描
- **哈希校验** — 支持 SHA-256、BLAKE3、CRC32
- **分卷压缩** — 创建和读取分卷压缩包（.zip, .7z）
- **密码保护** — AES 加密的 ZIP 和 7z 文件
- **编码页处理** — 自动检测非 UTF-8 文件名编码（Shift-JIS、GBK 等）
- **管道支持** — 从 stdin/stdout 读写压缩包（`-`）
- **压缩预设** — 保存和复用压缩参数配置
- **WIM / ISO 只读** — 列出 WIM 和 ISO 9660 镜像内的文件
- **Windows 右键菜单** — 右键菜单集成压缩/解压
- **多语言** — 12 种语言（英文、中文、日文、韩文、法文、德文、西班牙文、意大利文、葡萄牙文、俄文、阿拉伯文、土耳其文）

## 格式支持

| 格式 | 扩展名 | 压缩 | 解压 |
|------|--------|------|------|
| ZIP | `.zip` | 是 | 是 |
| 7-Zip | `.7z` | 是 | 是 |
| Tar | `.tar` | 是 | 是 |
| Tar + Gzip | `.tar.gz` `.tgz` | 是 | 是 |
| Tar + Bzip2 | `.tar.bz2` `.tbz2` | 是 | 是 |
| Tar + XZ | `.tar.xz` `.txz` | 是 | 是 |
| Tar + Zstd | `.tar.zst` `.tzst` | 是 | 是 |
| Tar + LZ4 | `.tar.lz4` `.tlz4` | 是 | 是 |
| Gzip | `.gz` | 是 | 是 |
| Bzip2 | `.bz2` | 是 | 是 |
| XZ | `.xz` | 是 | 是 |
| Zstd | `.zst` | 是 | 是 |
| LZ4 | `.lz4` | 是 | 是 |
| RAR | `.rar` | 否 | 是（需安装 `unrar.exe`） |
| WIM | `.wim` | 否 | 仅列出 |
| ISO 9660 | `.iso` | 否 | 是 |

## CLI 使用方法

主程序包含 GUI 和 CLI，通过子命令访问 CLI 功能。

### 查看压缩包

```powershell
fastzip list archive.zip
fastzip list archive.rar --password secret
fastzip list - < archive.tar.gz --format tar.gz
```

### 解压

```powershell
fastzip extract archive.zip -o ./output
fastzip extract archive.7z --flat --password secret
fastzip extract archive.zip --scan          # AMSI 扫描解压文件
fastzip extract archive.zip --codepage 932  # Shift-JIS 文件名
```

### 压缩

```powershell
fastzip compress ./folder -o output.zip
fastzip compress ./folder -o output.7z --level maximum
fastzip compress ./folder -o output.zip --sfx            # 自解压
fastzip compress ./folder -o output.zip --volume 100M    # 分卷
fastzip compress ./folder -o output.7z --password secret --encrypt-file-names
fastzip compress ./folder -o output.tar.zst --threads 4
```

### 完整性测试

```powershell
fastzip test archive.zip
fastzip test archive.7z --password secret
```

### 哈希校验

```powershell
fastzip checksum file.dat --algo sha256
fastzip checksum file.dat --algo blake3
fastzip checksum file.dat --algo crc32
```

### 基准测试

```powershell
fastzip benchmark -o ./results
```

### 其他命令

```powershell
fastzip formats              # 列出支持的扩展名
fastzip backends             # 显示后端可用状态
```

同时还提供一个精简版 CLI（`fastzip-cli.exe`），跳过 GUI 启动开销，适合脚本调用。

## 性能基准测试

FastZIP 内置基准测试功能，测试所有格式（ZIP, 7z, TarGz, TarBz2, TarXz, TarZst, TarLz4, Gz, Bz2, Xz, Zst, LZ4）在三种压缩级别（Fastest, Normal, Maximum）下，分别对可压缩和不可压缩的 1MB 数据集进行测试。结果包含压缩率和吞吐量（MB/s）。

可以通过 CLI 或 GUI 设置页面运行。

## 截图

![FastZIP 主窗口](../images/app.png)

## 从源码构建

需求：Rust 工具链（edition 2024），Windows 10+。

```powershell
git clone https://github.com/cccccyccccc/fastZIP.git
cd fastZIP
cargo build --release
```

构建产物在 `target/release/`：
- `fastzip.exe` — 主程序（GUI + CLI）
- `fastzip-cli.exe` — 精简 CLI
- `sfx-stub.exe` — 自解压引导程序

## 架构

```
src/
  archive/
    mod.rs       — 共享类型，压缩/解压核心管线
    native.rs    — 原生 Rust 后端（ZIP, 7z, tar.*, gz, bz2, xz, zst, lz4）
    rar.rs       — RAR 适配器（调用外部 unrar.exe）
    service.rs   — 后端路由门面
    sfx.rs       — 自解压构建器
    test.rs      — 压缩包完整性测试
    iso.rs       — ISO 9660 读取器
    wim.rs       — WIM 元数据读取器
  bin/
    fastzip-cli.rs  — 精简 CLI 程序
    sfx-stub.rs     — SFX 引导程序
  gui.rs          — egui/eframe 原生 GUI
  amsi.rs         — Windows AMSI 集成
  benchmark.rs    — 压缩基准测试套件
  hash.rs         — SHA-256 / BLAKE3 / CRC32
  encoding.rs     — 编码检测与转换
  localization.rs — 12 语言本地化
  settings.rs     — INI 设置 + 压缩预设
```

## 许可证

GPL-3.0
