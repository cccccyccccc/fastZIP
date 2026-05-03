# FastZIP

[English](../README.md) | [简体中文](README_zh.md) | [日本語](README_ja.md) | [한국어](README_ko.md) | [Français](README_fr.md) | [Deutsch](README_de.md) | [Español](README_es.md) | [Italiano](README_it.md) | **Português** | [Русский](README_ru.md) | [العربية](README_ar.md) | [Türkçe](README_tr.md)

<img src="../assets/fastzip-icon.png" width="64" height="64" align="right">

Ferramenta de arquivos nativa em Rust com GUI e CLI. Comprima, extraia, teste e faça benchmark — tudo em um único binário, sem dependências externas para formatos comuns.

## Funcionalidades

- **Compressão e extração** — 12 formatos de saída com 5 níveis de compressão
- **Teste de integridade** — percorre entradas e verifica o CRC32 sem extrair
- **Benchmark de desempenho** — suíte de benchmark integrada para todas as combinações de formato/nível
- **Arquivos autoextraíveis** — transforme qualquer arquivo em um .exe independente (SFX)
- **Verificação AMSI** — integração opcional com a interface antimalware do Windows
- **Hash/checksum** — SHA-256, BLAKE3, CRC32 para qualquer arquivo
- **Volumes divididos** — crie e leia arquivos em múltiplos volumes (.zip, .7z)
- **Proteção por senha** — arquivos ZIP e 7z com criptografia AES
- **Tratamento de codepage** — detecção automática de codificação para nomes de arquivo não UTF-8 (Shift-JIS, GBK, etc.)
- **Suporte a pipe** — leia/escreva arquivos de stdin/stdout (`-`)
- **Predefinições de compressão** — salve e reutilize configurações
- **Leitura WIM / ISO** — liste arquivos dentro de imagens WIM e ISO 9660
- **Integração com o Windows** — menu de contexto para comprimir/extrair
- **Localização** — 12 idiomas

## Formatos suportados

| Formato | Extensão | Comprimir | Extrair |
|---------|----------|-----------|---------|
| ZIP | `.zip` | Sim | Sim |
| 7-Zip | `.7z` | Sim | Sim |
| Tar | `.tar` | Sim | Sim |
| Tar + Gzip | `.tar.gz` `.tgz` | Sim | Sim |
| Tar + Bzip2 | `.tar.bz2` `.tbz2` | Sim | Sim |
| Tar + XZ | `.tar.xz` `.txz` | Sim | Sim |
| Tar + Zstd | `.tar.zst` `.tzst` | Sim | Sim |
| Tar + LZ4 | `.tar.lz4` `.tlz4` | Sim | Sim |
| Gzip | `.gz` | Sim | Sim |
| Bzip2 | `.bz2` | Sim | Sim |
| XZ | `.xz` | Sim | Sim |
| Zstd | `.zst` | Sim | Sim |
| LZ4 | `.lz4` | Sim | Sim |
| RAR | `.rar` | Não | Sim (via `unrar.exe`) |
| WIM | `.wim` | Não | Apenas lista |
| ISO 9660 | `.iso` | Não | Sim |

## Uso da CLI

O binário principal inclui tanto a GUI quanto a CLI.

### Inspecionar arquivos

```powershell
fastzip list archive.zip
fastzip list archive.rar --password secret
fastzip list - < archive.tar.gz --format tar.gz
```

### Extrair

```powershell
fastzip extract archive.zip -o ./output
fastzip extract archive.7z --flat --password secret
fastzip extract archive.zip --scan          # Verificação AMSI
fastzip extract archive.zip --codepage 932  # Nomes Shift-JIS
```

### Comprimir

```powershell
fastzip compress ./folder -o output.zip
fastzip compress ./folder -o output.7z --level maximum
fastzip compress ./folder -o output.zip --sfx            # Autoextraível
fastzip compress ./folder -o output.zip --volume 100M    # Volumes
fastzip compress ./folder -o output.7z --password secret --encrypt-file-names
fastzip compress ./folder -o output.tar.zst --threads 4
```

### Teste de integridade

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

### Outros comandos

```powershell
fastzip formats              # Listar extensões suportadas
fastzip backends             # Exibir status dos backends
```

## Capturas de tela

![Janela principal do FastZIP](../images/app.png)

## Compilar do código fonte

Requisitos: toolchain Rust (edition 2024), Windows 10+.

```powershell
git clone https://github.com/cccccyccccc/fastZIP.git
cd fastZIP
cargo build --release
```

Artefatos de build (`target/release/`):
- `fastzip.exe` — GUI + CLI principal
- `fastzip-cli.exe` — apenas CLI
- `sfx-stub.exe` — stub autoextraível

## Arquitetura

```
src/
  archive/
    mod.rs       — tipos compartilhados, pipeline de compressão/extração
    native.rs    — backend Rust nativo
    rar.rs       — adaptador RAR (unrar.exe externo)
    service.rs   — fachada de roteamento de backends
    sfx.rs       — construtor de SFX
    test.rs      — teste de integridade
    iso.rs       — leitor ISO 9660
    wim.rs       — leitor de metadados WIM
  bin/
    fastzip-cli.rs  — binário CLI leve
    sfx-stub.rs     — binário stub SFX
  gui.rs          — GUI nativa egui/eframe
  amsi.rs         — integração Windows AMSI
  benchmark.rs    — suíte de benchmark
  hash.rs         — SHA-256 / BLAKE3 / CRC32
  encoding.rs     — detecção de codepage
  localization.rs — localização em 12 idiomas
  settings.rs     — configurações INI + predefinições
```

## Licença

GPL-3.0
