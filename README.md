<div align="center">

# 🎵 ivLyrics Lyrics Bridge

**Spotify용 ivLyrics 확장에 가사를 제공하는 로컬 브릿지 서버**

Rust로 작성된 단일 바이너리 서버가 MusicXMatch · Deezer · Bugs · Genie의 가사를
하나의 로컬 API로 묶어 ivLyrics에 동기화 가사를 공급합니다.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![Version](https://img.shields.io/badge/version-0.7.17-green.svg)](https://github.com/oneulddu/musicxmatch-api)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)](#-빠른-시작)

[빠른 시작](#-빠른-시작) • [Provider 설정](#️-provider-설정) • [API](#-api-엔드포인트) • [고급 설정](#-고급-설정) • [문제 해결](#-문제-해결)

</div>

---

## ✨ 주요 기능

<table>
<tr>
<td width="50%" valign="top">

### 🎯 스마트 매칭
- Spotify Track ID 우선 검색
- 제목·아티스트·재생시간 기반 fallback
- 4개 Provider 통합으로 높은 성공률

</td>
<td width="50%" valign="top">

### ⚡ 빠른 응답
- Rust 기반 단일 바이너리
- 결과 자동 캐싱
- 낮은 메모리·CPU 사용량

</td>
</tr>
<tr>
<td width="50%" valign="top">

### 🎤 최적 가사 선택
- `karaoke > synced > unsynced` 우선순위
- LRC 타임스탬프 동기화 가사
- 다국어·한국 음악 모두 지원

</td>
<td width="50%" valign="top">

### 🔄 손쉬운 운영
- OS 부팅 시 백그라운드 자동 시작
- ivLyrics 설정에서 원클릭 업데이트
- provider 자동 복구 루프

</td>
</tr>
</table>

### 지원 Provider

| Provider | 설정 필요 | 동기화 가사 | 특징 |
|----------|:--------:|:----------:|------|
| 🎵 **MusicXMatch** | ❌ | ✅ | 자동 세션 토큰 관리, 광범위한 글로벌 DB |
| 🎧 **Deezer** | ✅ ARL | ✅ | ARL 쿠키 필요, 고품질 가사 |
| 🐛 **Bugs** | ❌ | ✅ | 한국 음악 특화, 로그인 불필요 |
| 🧞 **Genie** | ❌ | ✅ | 한국 음악 특화, 로그인 불필요 |

> `backend=auto`(기본값)는 위 provider를 순회하며 가장 좋은 가사를 자동 선택합니다.

---

## 🚀 빠른 시작

### 자동 설치 (권장)

<details open>
<summary><b>🪟 Windows (PowerShell)</b></summary>

```powershell
iwr -useb "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.ps1" | iex
```

**작업 스케줄러로 자동 시작 (선택)**
```powershell
$env:IVLYRICS_WINDOWS_AUTOSTART = "scheduled-task"
iwr -useb "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.ps1" | iex
```

</details>

<details>
<summary><b>🍎 macOS / 🐧 Linux</b></summary>

```bash
curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.sh | bash
```

</details>

### 설치 스크립트가 하는 일

1. ✅ 로컬 서버 설치 및 실행 (`http://127.0.0.1:8092`)
2. ✅ OS 부팅 시 백그라운드 자동 시작 등록
3. ✅ 서버 health check 및 CORS 검증
4. ✅ 4개 provider addon 자동 등록
5. ✅ ivLyrics 가사 선택 우선순위 패치 적용
6. ✅ `spicetify apply` 자동 실행

### 사용 방법

1. ivLyrics 설정에서 원하는 provider 활성화
2. Spotify에서 음악 재생
3. 가사 자동 표시 🎉

> 💡 **Tip** — Deezer를 쓰려면 [ARL 쿠키 설정](#deezer)이 필요합니다.

---

## ⚙️ Provider 설정

### MusicXMatch
별도 설정이 필요 없습니다. 서버가 세션 토큰을 자동으로 발급·갱신합니다.

### Deezer

Deezer 가사를 사용하려면 ARL 쿠키가 필요합니다.

<details>
<summary><b>ARL 쿠키 얻는 방법</b></summary>

1. [Deezer 웹 플레이어](https://www.deezer.com) 로그인
2. 브라우저 개발자 도구 열기 (`F12`)
3. **Application**(Chrome) 또는 **Storage**(Firefox) 탭 선택
4. **Cookies → `https://www.deezer.com`** 선택
5. `arl` 값 복사
6. ivLyrics 설정 → Deezer Provider → *Deezer cookie* 에 붙여넣기

</details>

환경 변수로도 설정할 수 있습니다:
```bash
export DEEZER_ARL=your_arl_cookie_value
```

### Bugs / Genie
별도 설정이 필요 없습니다. 로그인 없이 공개 API를 사용합니다.

---

## 🧩 ivLyrics 가사 선택 우선순위

ivLyrics 기본 동작은 unsynced(일반) 가사를 먼저 채택할 때가 있습니다.
이 프로젝트의 패치 스크립트는 선택 로직을 **`karaoke > synced > unsynced`** 순으로 바꿔
항상 가장 좋은 형식의 가사가 우선되도록 합니다. (설치 스크립트가 자동 적용)

수동으로 다시 적용하거나 동작만 확인하려면:

<details>
<summary><b>🍎 macOS / 🐧 Linux</b></summary>

```bash
# 적용
scripts/patch-ivlyrics-selection.sh

# 변경 없이 패치 필요 여부만 확인
scripts/patch-ivlyrics-selection.sh --dry-run

# 패치만 하고 spicetify apply / Spotify 재시작은 건너뛰기
scripts/patch-ivlyrics-selection.sh --no-apply
```

</details>

<details>
<summary><b>🪟 Windows (PowerShell)</b></summary>

```powershell
scripts\patch-ivlyrics-selection.ps1
scripts\patch-ivlyrics-selection.ps1 -DryRun
scripts\patch-ivlyrics-selection.ps1 -NoApply
```

</details>

---

## 🔧 고급 설정

### 환경 변수

| 변수 | 기본값 | 설명 |
|------|--------|------|
| `PORT` | `8092` | 서버 포트 |
| `IVLYRICS_BIND_HOST` | `127.0.0.1` | 바인딩 호스트 |
| `IVLYRICS_ALLOWED_ORIGINS` | `spicetify://ivlyrics,https://xpui.app.spotify.com` | CORS 허용 origin |
| `IVLYRICS_HTTP_TIMEOUT_SECS` | `10` | provider HTTP 타임아웃(초) |
| `IVLYRICS_UPDATE_TIMEOUT_SECS` | `5` | 업데이트 확인 타임아웃(초) |
| `DEEZER_ARL` | – | Deezer ARL 쿠키 |
| `IVLYRICS_MXM_LOG` | `~/.ivlyrics-musicxmatch/server.log` | 서버 로그 경로 |
| `IVLYRICS_MXM_CONFIG` | `~/.ivlyrics-musicxmatch/config.json` | 설정 파일 경로 |
| `MXM_SESSION_FILE` | – | MusicXMatch 세션 캐시 경로 |

`.env.example`을 복사해 시작할 수 있습니다:
```bash
cp .env.example .env
```

### 애드온 수동 설치 / 재등록

설치 스크립트가 기본적으로 애드온을 등록하지만, 수동 재등록이 필요할 때:

<details>
<summary><b>🪟 Windows</b></summary>

```powershell
$base = "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main"
& ([scriptblock]::Create((iwr -useb "$base/addon-manager-compat.ps1").Content)) `
  "$base/Addon_Lyrics_MusicXMatch.js" `
  "$base/Addon_Lyrics_Deezer.js" `
  "$base/Addon_Lyrics_Bugs.js" `
  "$base/Addon_Lyrics_Genie.js"
```

</details>

<details>
<summary><b>🍎 macOS / 🐧 Linux</b></summary>

```bash
base="https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main"
curl -fsSL "$base/addon-manager-compat.sh" | sh -s -- \
  "$base/Addon_Lyrics_MusicXMatch.js" \
  "$base/Addon_Lyrics_Deezer.js" \
  "$base/Addon_Lyrics_Bugs.js" \
  "$base/Addon_Lyrics_Genie.js"
```

</details>

**특정 provider만 설치**:
```bash
curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/addon-manager-compat.sh | sh -s -- \
  "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/Addon_Lyrics_Genie.js"
```

### ivLyrics 업데이트 후 provider 복구

ivLyrics 업데이트가 `manifest.json`을 새로 쓰면 커스텀 provider 등록이 빠질 수 있습니다.
등록 기록은 `addon_sources.json`에 남기 때문에 아래 명령으로 복구할 수 있습니다.
(서버의 자동 복구 루프도 provider를 되살린 뒤 `spicetify apply`를 실행합니다.)

**Windows**
```powershell
& ([scriptblock]::Create((iwr -useb "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/addon-manager-compat.ps1").Content)) --restore
```

**macOS / Linux**
```bash
curl -fsSL "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/addon-manager-compat.sh" | sh -s -- --restore
```

---

## 🔄 업데이트

### ivLyrics 설정에서 (권장)
- **Update server** — 서버만 업데이트
- **Update all** — 서버 + provider addon 모두 업데이트

### 수동 업데이트

<details>
<summary><b>서버 재설치 (= 업데이트)</b></summary>

**Windows**
```powershell
iwr -useb "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.ps1" | iex
```

**macOS / Linux**
```bash
curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.sh | bash
```

</details>

---

## 🗑️ 제거

<details>
<summary><b>🪟 Windows</b></summary>

```powershell
iwr -useb "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/uninstall.ps1" | iex
```

</details>

<details>
<summary><b>🍎 macOS / 🐧 Linux</b></summary>

```bash
curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/uninstall.sh | bash
```

</details>

> 참고: 애드온 제거는 ivLyrics addon tracking에서 별도로 관리하세요.

---

## 📡 API 엔드포인트

기본 주소: `http://127.0.0.1:8092`

| 메서드 | 경로 | 설명 |
|--------|------|------|
| `GET` | `/health` | 서버 상태·provider 상태 확인 |
| `GET` | `/lyrics` | 가사 조회 |
| `GET` | `/config` | Deezer 설정 확인 |
| `POST` | `/config` | Deezer ARL 저장 |
| `DELETE` | `/cache` | 캐시 초기화 |
| `GET` | `/update/check` | 업데이트 확인 |
| `POST` | `/update/apply` | 서버 업데이트 적용 |
| `POST` | `/update/apply-all` | 서버 + 애드온 전체 업데이트 |

### `GET /lyrics`

```bash
curl "http://127.0.0.1:8092/lyrics?title=Love%20Love%20Love&artist=에픽하이"
```

| 파라미터 | 타입 | 필수 | 설명 |
|----------|------|:----:|------|
| `title` | string | ✅\* | 곡 제목 |
| `artist` | string | ✅\* | 아티스트 |
| `spotifyId` | string | ❌ | Spotify Track ID (있으면 title/artist 생략 가능) |
| `durationMs` | number | ❌ | 곡 길이(밀리초) — 매칭 정확도 향상 |
| `backend` | string | ❌ | `auto`(기본)·`musicxmatch`·`deezer`·`bugs`·`genie` |
| `debug` | number | ❌ | 디버그 정보 포함 (0/1) |

<sub>\* `spotifyId`가 있으면 `title`/`artist`는 생략 가능합니다.</sub>

### `GET /health`

```bash
curl http://127.0.0.1:8092/health
```

<details>
<summary><b>응답 예시</b></summary>

```json
{
  "status": "ok",
  "version": "0.7.17",
  "provider": "musicxmatch",
  "backend": "musixmatch + deezer(optional) + bugs + genie",
  "cors": true,
  "deezerConfigured": false,
  "cacheEntries": 5,
  "providerStatuses": {
    "musicxmatch": "ready",
    "deezer": "not-configured",
    "bugs": "ready",
    "genie": "ready"
  }
}
```

</details>

### `POST /config` — Deezer ARL 저장

```bash
curl -X POST http://127.0.0.1:8092/config \
  -H 'Content-Type: application/json' \
  -d '{"deezerArl":"your_arl_cookie"}'
```

---

## 🗂️ 프로젝트 구조

```
musicxmatch-api/
├── src/
│   ├── main.rs            # 서버 진입점 · 라우팅 · 캐시 · 업데이트 · 자동 복구
│   ├── matching.rs        # 곡 매칭 / 가사 우선순위 선택 로직
│   ├── musixmatch.rs      # MusicXMatch provider (세션 토큰 자동 관리)
│   ├── deezer.rs          # Deezer provider (ARL 기반)
│   ├── bugs.rs            # Bugs provider
│   ├── genie.rs           # Genie provider
│   └── logging.rs         # 파일 로깅
├── scripts/
│   ├── generate_addons.js          # addon_definitions.json → Addon_*.js 생성
│   ├── addon_definitions.json      # provider 메타데이터 (단일 소스)
│   ├── bump_version.sh             # 버전 일괄 갱신
│   └── patch-ivlyrics-selection.*  # 가사 선택 우선순위 패치
├── Addon_Lyrics_*.js      # ivLyrics용 provider 애드온 (생성물)
├── install.{sh,ps1}       # 설치 스크립트
├── uninstall.{sh,ps1}     # 제거 스크립트
├── addon-manager-compat.* # 애드온 등록/복구 도우미
└── manifest.json          # 애드온 매니페스트
```

---

## 🛠️ 개발

```bash
# 빌드
cargo build --release

# 실행 (개발)
cargo run

# 테스트
cargo test

# 애드온 재생성 (addon_definitions.json 수정 후)
node scripts/generate_addons.js

# 버전 일괄 업데이트
./scripts/bump_version.sh 0.7.18
```

### 로그 확인

**Windows**
```powershell
Get-Content "$env:USERPROFILE\.ivlyrics-musicxmatch\server.log" -Wait
```

**macOS / Linux**
```bash
tail -f ~/.ivlyrics-musicxmatch/server.log
```

---

## 🔍 문제 해결

<details>
<summary><b>가사가 표시되지 않음</b></summary>

1. 서버 상태 확인: `curl http://127.0.0.1:8092/health`
2. 로그 확인: `tail -f ~/.ivlyrics-musicxmatch/server.log`
3. ivLyrics 설정에서 provider가 활성화돼 있는지 확인
4. Spicetify extension 등록 확인: `spicetify config`

</details>

<details>
<summary><b>Deezer 가사가 안 나옴</b></summary>

1. ARL 쿠키 설정 확인
2. `/config` 엔드포인트로 설정 상태 확인
3. Deezer 로그인 / 쿠키 만료 여부 확인

</details>

<details>
<summary><b>일반 가사만 나오고 동기화가 안 됨</b></summary>

ivLyrics 선택 우선순위 패치가 빠졌을 수 있습니다.
[가사 선택 우선순위 패치](#-ivlyrics-가사-선택-우선순위)를 다시 적용하세요.

</details>

<details>
<summary><b>서버가 시작되지 않음</b></summary>

**Windows** — 작업 스케줄러에서 `ivLyrics-MusicXMatch` 작업 확인

**macOS**
```bash
launchctl list | grep ivlyrics
```

**Linux**
```bash
systemctl --user status ivlyrics-musicxmatch
```

</details>

---

## ⚠️ 면책 조항

본 프로젝트는 **교육 목적**으로만 제공됩니다. 무단 배포·복제·변경, 또는 불법 활동에
사용하는 등 본 목적에 반하는 오용·남용은 엄격히 금지되며, 관련 법률 및 규정 위반에
해당할 수 있습니다.

---

## 📄 라이선스

[MIT License](LICENSE)

## 🤝 기여

이슈와 PR을 환영합니다!

- 🐛 [버그 리포트](https://github.com/oneulddu/musicxmatch-api/issues)
- 💡 [기능 제안](https://github.com/oneulddu/musicxmatch-api/issues)
- 🔧 [Pull Request](https://github.com/oneulddu/musicxmatch-api/pulls)

---

<div align="center">

**Made with ❤️ for Spotify & ivLyrics users**

[⬆ 맨 위로](#-ivlyrics-lyrics-bridge)

</div>
