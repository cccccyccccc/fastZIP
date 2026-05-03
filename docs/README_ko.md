# FastZIP

[English](../README.md) | [简体中文](README_zh.md) | [日本語](README_ja.md) | **한국어** | [Français](README_fr.md) | [Deutsch](README_de.md) | [Español](README_es.md) | [Italiano](README_it.md) | [Português](README_pt-BR.md) | [Русский](README_ru.md) | [العربية](README_ar.md) | [Türkçe](README_tr.md)

<img src="../assets/fastzip-icon.png" width="64" height="64" align="right">

Rust로 작성된 네이티브 아카이브 도구. GUI와 CLI를 모두 제공하며, 압축, 해제, 테스트, 벤치마크를 단일 바이너리로 실행. 일반 형식은 외부 의존성 없이 작동.

## 기능

- **압축 및 해제** — 12가지 출력 형식, 5단계 압축 레벨
- **아카이브 테스트** — 해제 없이 항목을 순회하며 CRC32 무결성 검증
- **성능 벤치마크** — 모든 형식/레벨 조합에 대한 내장 벤치마크
- **자동 압축 해제** — 모든 아카이브를 독립 실행형 .exe(SFX)로 변환
- **AMSI 악성코드 검사** — Windows 안티멀웨어 검사 인터페이스 통합
- **해시/체크섬** — SHA-256, BLAKE3, CRC32
- **분할 볼륨** — 다중 볼륨 아카이브 생성 및 읽기(.zip, .7z)
- **비밀번호 보호** — AES 암호화 ZIP 및 7z
- **코드페이지 처리** — 비UTF-8 파일명 자동 인코딩 감지(Shift-JIS, GBK 등)
- **파이프 지원** — stdin/stdout으로 아카이브 읽기/쓰기(`-`)
- **압축 프리셋** — 압축 설정 저장 및 재사용
- **WIM / ISO 읽기** — WIM 및 ISO 9660 이미지 내 파일 목록 표시
- **Windows 셸 통합** — 우클릭 메뉴로 압축/해제
- **다국어 지원** — 12개 언어

## 지원 형식

| 형식 | 확장자 | 압축 | 해제 |
|------|--------|------|------|
| ZIP | `.zip` | 가능 | 가능 |
| 7-Zip | `.7z` | 가능 | 가능 |
| Tar | `.tar` | 가능 | 가능 |
| Tar + Gzip | `.tar.gz` `.tgz` | 가능 | 가능 |
| Tar + Bzip2 | `.tar.bz2` `.tbz2` | 가능 | 가능 |
| Tar + XZ | `.tar.xz` `.txz` | 가능 | 가능 |
| Tar + Zstd | `.tar.zst` `.tzst` | 가능 | 가능 |
| Tar + LZ4 | `.tar.lz4` `.tlz4` | 가능 | 가능 |
| Gzip | `.gz` | 가능 | 가능 |
| Bzip2 | `.bz2` | 가능 | 가능 |
| XZ | `.xz` | 가능 | 가능 |
| Zstd | `.zst` | 가능 | 가능 |
| LZ4 | `.lz4` | 가능 | 가능 |
| RAR | `.rar` | 불가 | 가능 (`unrar.exe` 필요) |
| WIM | `.wim` | 불가 | 목록만 |
| ISO 9660 | `.iso` | 불가 | 가능 |

## CLI 사용법

메인 바이너리는 GUI와 CLI를 모두 포함합니다.

### 아카이브 내용 보기

```powershell
fastzip list archive.zip
fastzip list archive.rar --password secret
fastzip list - < archive.tar.gz --format tar.gz
```

### 해제

```powershell
fastzip extract archive.zip -o ./output
fastzip extract archive.7z --flat --password secret
fastzip extract archive.zip --scan          # AMSI 검사
fastzip extract archive.zip --codepage 932  # Shift-JIS 파일명
```

### 압축

```powershell
fastzip compress ./folder -o output.zip
fastzip compress ./folder -o output.7z --level maximum
fastzip compress ./folder -o output.zip --sfx            # 자동 해제
fastzip compress ./folder -o output.zip --volume 100M    # 분할
fastzip compress ./folder -o output.7z --password secret --encrypt-file-names
fastzip compress ./folder -o output.tar.zst --threads 4
```

### 무결성 테스트

```powershell
fastzip test archive.zip
fastzip test archive.7z --password secret
```

### 해시/체크섬

```powershell
fastzip checksum file.dat --algo sha256
fastzip checksum file.dat --algo blake3
fastzip checksum file.dat --algo crc32
```

### 벤치마크

```powershell
fastzip benchmark -o ./results
```

### 기타

```powershell
fastzip formats              # 지원 확장자 목록
fastzip backends             # 백엔드 상태 표시
```

## 스크린샷

![FastZIP 메인 화면](../images/app.png)

## 소스에서 빌드

요구사항: Rust 툴체인(edition 2024), Windows 10+.

```powershell
git clone https://github.com/cccccyccccc/fastZIP.git
cd fastZIP
cargo build --release
```

빌드 결과물(`target/release/`):
- `fastzip.exe` — 메인 GUI + CLI
- `fastzip-cli.exe` — CLI 전용
- `sfx-stub.exe` — 자동 해제 스텁

## 아키텍처

```
src/
  archive/
    mod.rs       — 공유 타입, 압축/해제 파이프라인
    native.rs    — 네이티브 Rust 백엔드
    rar.rs       — RAR 어댑터(외부 unrar.exe)
    service.rs   — 백엔드 라우팅
    sfx.rs       — 자동 해제 빌더
    test.rs      — 아카이브 무결성 테스트
    iso.rs       — ISO 9660 리더
    wim.rs       — WIM 메타데이터 리더
  bin/
    fastzip-cli.rs  — CLI 전용 바이너리
    sfx-stub.rs     — SFX 스텁 바이너리
  gui.rs          — egui/eframe 네이티브 GUI
  amsi.rs         — Windows AMSI 통합
  benchmark.rs    — 압축 벤치마크
  hash.rs         — SHA-256 / BLAKE3 / CRC32
  encoding.rs     — 코드페이지 감지 및 변환
  localization.rs — 12개 언어 로컬라이제이션
  settings.rs     — INI 설정 + 압축 프리셋
```

## 라이선스

GPL-3.0
