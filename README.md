<div align="center">

# 🎵 ivLyrics Lyrics Providers

**Spotify용 ivLyrics 확장에 가사를 제공하는 로컬 서버**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![Version](https://img.shields.io/badge/version-0.7.13-green.svg)](https://github.com/oneulddu/musicxmatch-api)

[빠른 시작](#-빠른-시작) • [Provider 설정](#-provider-설정) • [API 문서](#-api-엔드포인트) • [문제 해결](#-문제-해결)

</div>

---

## ✨ 주요 기능

<table>
<tr>
<td width="50%">

### 🎯 스마트 매칭
- Spotify Track ID 우선 검색
- 자동 fallback으로 높은 성공률
- 4개 Provider 통합 지원

</td>
<td width="50%">

### ⚡ 빠른 응답
- 30분 자동 캐싱
- Rust 기반 단일 바이너리
- 낮은 메모리 사용량

</td>
</tr>
<tr>
<td width="50%">

### 🎵 동기화 가사
- LRC 형식 타임스탬프 지원
- 실시간 가사 동기화
- 다국어 가사 지원

</td>
<td width="50%">

### 🔄 자동 시작
- OS 부팅 시 백그라운드 실행
- 별도 설정 없이 즉시 사용
- 안정적인 서비스 제공

</td>
</tr>
</table>

### 지원 Provider

| Provider | 설정 필요 | 동기화 가사 | 특징 |
|----------|-----------|-------------|------|
| 🎵 **MusicXMatch** | ❌ | ✅ | 자동 세션 관리, 광범위한 DB |
| 🎧 **Deezer** | ✅ | ✅ | ARL 쿠키 필요, 고품질 가사 |
| 🐛 **Bugs** | ❌ | ✅ | 한국 음악 특화 |
| 🧞 **Genie** | ❌ | ✅ | 한국 음악 특화 |

---

## 🚀 빠른 시작

### 자동 설치 (권장)

<details open>
<summary><b>Windows (PowerShell)</b></summary>

```powershell
iwr -useb "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.ps1" | iex
```

**작업 스케줄러 사용 (선택사항)**
```powershell
$env:IVLYRICS_WINDOWS_AUTOSTART = "scheduled-task"
iwr -useb "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.ps1" | iex
```

</details>

<details>
<summary><b>macOS / Linux</b></summary>

```bash
curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.sh | bash
```

</details>

### 설치 스크립트 동작

설치 스크립트는 다음을 자동으로 수행합니다:

1. ✅ 로컬 서버 설치 (`http://127.0.0.1:8092`)
2. ✅ OS 부팅 시 자동 시작 설정
3. ✅ 서버 health check 및 CORS 검증
4. ✅ 4개 provider addon 자동 등록
5. ✅ `spicetify apply` 자동 적용

### 사용 방법

1. ivLyrics 설정에서 원하는 provider 활성화
2. Spotify에서 음악 재생
3. 가사 자동 표시 🎉

> **💡 Tip**: Deezer 사용 시 [ARL 쿠키 설정](#deezer) 필요

---

## ⚙️ Provider 설정

### MusicXMatch

별도 설정 불필요. 자동으로 세션 토큰을 관리합니다.

### Deezer

Deezer 가사를 사용하려면 ARL 쿠키가 필요합니다:

<details>
<summary><b>ARL 쿠키 얻는 방법</b></summary>

1. [Deezer 웹 플레이어](https://www.deezer.com) 로그인
2. 브라우저 개발자 도구 열기 (`F12`)
3. **Application** (Chrome) 또는 **Storage** (Firefox) 탭 선택
4. **Cookies** → `https://www.deezer.com` 선택
5. `arl` 값 복사
6. ivLyrics 설정 → Deezer Provider → Deezer cookie에 붙여넣기

</details>

**환경 변수로 설정** (선택사항):
```bash
export DEEZER_ARL=your_arl_cookie_value
```

### Bugs / Genie

별도 설정 불필요. 로그인 없이 공개 API를 사용합니다.

---

## 🔧 고급 설정

### 환경 변수

```bash
# 서버 포트 (기본: 8092)
PORT=8092

# Provider별 타임아웃 (초)
IVLYRICS_HTTP_TIMEOUT_SECS=10
IVLYRICS_MUSIXMATCH_TIMEOUT_SECS=15
IVLYRICS_DEEZER_TIMEOUT_SECS=10
IVLYRICS_BUGS_TIMEOUT_SECS=10
IVLYRICS_GENIE_TIMEOUT_SECS=10

# Deezer ARL
DEEZER_ARL=your_cookie

# 로그 파일 위치
IVLYRICS_MXM_LOG=/custom/path/server.log
```

### 애드온 수동 설치

설치 스크립트가 기본적으로 애드온을 등록합니다. 수동 재등록이 필요한 경우:

<details>
<summary><b>Windows</b></summary>

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
<summary><b>macOS / Linux</b></summary>

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

---

## 🔄 업데이트

### ivLyrics 설정에서 (권장)

- **Update server**: 서버만 업데이트
- **Update all**: 서버 + provider addon 모두 업데이트

### 수동 업데이트

<details>
<summary><b>서버 업데이트</b></summary>

**Windows**
```powershell
iwr -useb "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.ps1" | iex
```

**macOS / Linux**
```bash
curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.sh | bash
```

</details>

<details>
<summary><b>애드온 업데이트</b></summary>

**Windows**
```powershell
$base = "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main"
& ([scriptblock]::Create((iwr -useb "$base/addon-manager-compat.ps1").Content)) `
  "$base/Addon_Lyrics_MusicXMatch.js" `
  "$base/Addon_Lyrics_Deezer.js" `
  "$base/Addon_Lyrics_Bugs.js" `
  "$base/Addon_Lyrics_Genie.js"
```

**macOS / Linux**
```bash
base="https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main"
curl -fsSL "$base/addon-manager-compat.sh" | sh -s -- \
  "$base/Addon_Lyrics_MusicXMatch.js" \
  "$base/Addon_Lyrics_Deezer.js" \
  "$base/Addon_Lyrics_Bugs.js" \
  "$base/Addon_Lyrics_Genie.js"
```

</details>

---

## 🗑️ 제거

<details>
<summary><b>Windows</b></summary>

```powershell
iwr -useb "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/uninstall.ps1" | iex
```

</details>

<details>
<summary><b>macOS / Linux</b></summary>

```bash
curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/uninstall.sh | bash
```

</details>

> **참고**: 애드온 제거는 ivLyrics addon tracking에서 별도로 관리하세요.

---

## 📡 API 엔드포인트

### `GET /health`

서버 상태 확인

```bash
curl http://127.0.0.1:8092/health
```

<details>
<summary><b>응답 예시</b></summary>

```json
{
  "status": "ok",
  "version": "0.7.13",
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

### `GET /lyrics`

가사 조회

```bash
curl "http://127.0.0.1:8092/lyrics?title=Love%20Love%20Love&artist=에픽하이"
```

**파라미터**:

| 파라미터 | 타입 | 필수 | 설명 |
|----------|------|------|------|
| `title` | string | ✅* | 곡 제목 |
| `artist` | string | ✅* | 아티스트 |
| `spotifyId` | string | ❌ | Spotify Track ID (있으면 title/artist 불필요) |
| `durationMs` | number | ❌ | 곡 길이 (밀리초) |
| `backend` | string | ❌ | provider 지정 (auto/musicxmatch/deezer/bugs/genie) |
| `debug` | number | ❌ | 디버그 정보 포함 (0/1) |

*spotifyId가 있으면 title/artist 생략 가능

### `GET /config`

Deezer 설정 확인

```bash
curl http://127.0.0.1:8092/config
```

### `POST /config`

Deezer ARL 저장

```bash
curl -X POST http://127.0.0.1:8092/config \
  -H 'Content-Type: application/json' \
  -d '{"deezerArl":"your_arl_cookie"}'
```

### `DELETE /cache`

캐시 초기화

```bash
curl -X DELETE http://127.0.0.1:8092/cache
```

---

## 📋 로그 확인

<details>
<summary><b>Windows</b></summary>

```powershell
Get-Content "$env:USERPROFILE\.ivlyrics-musicxmatch\server.log" -Wait
```

</details>

<details>
<summary><b>macOS / Linux</b></summary>

```bash
tail -f ~/.ivlyrics-musicxmatch/server.log
```

</details>

---

## 🛠️ 개발

### 빌드

```bash
cargo build --release
```

### 실행

```bash
cargo run
```

### 테스트

```bash
cargo test
```

### 애드온 생성

```bash
node scripts/generate_addons.js
```

### 버전 업데이트

```bash
./scripts/bump_version.sh 0.7.14
```

---

## 🔍 문제 해결

<details>
<summary><b>가사가 표시되지 않음</b></summary>

1. 서버 상태 확인:
   ```bash
   curl http://127.0.0.1:8092/health
   ```

2. 로그 확인:
   ```bash
   tail -f ~/.ivlyrics-musicxmatch/server.log
   ```

3. ivLyrics 설정에서 provider 활성화 확인

4. Spicetify extension 등록 확인:
   ```bash
   spicetify config
   ```

</details>

<details>
<summary><b>Deezer 가사가 안 나옴</b></summary>

1. ARL 쿠키 설정 확인
2. `/config` 엔드포인트로 설정 상태 확인
3. Deezer 로그인 상태 확인 (쿠키 만료 가능)

</details>

<details>
<summary><b>서버가 시작되지 않음</b></summary>

**Windows**: 작업 스케줄러에서 "ivLyrics-MusicXMatch" 작업 확인

**macOS**:
```bash
launchctl list | grep ivlyrics
```

**Linux**:
```bash
systemctl --user status ivlyrics-musicxmatch
```

</details>

---

## ⚠️ 면책 조항

본 프로젝트는 **교육 목적**으로만 제공됩니다. 무단 배포, 복제, 변경, 또는 불법 활동에 사용하는 등 본 목적에 반하는 정보의 오용이나 남용은 엄격히 금지되며, 관련 법률 및 규정 위반에 해당할 수 있습니다.

---

## 📄 라이선스

[MIT License](LICENSE)

---

## 🤝 기여

이슈 및 PR을 환영합니다!

- 🐛 [버그 리포트](https://github.com/oneulddu/musicxmatch-api/issues)
- 💡 [기능 제안](https://github.com/oneulddu/musicxmatch-api/issues)
- 🔧 [Pull Request](https://github.com/oneulddu/musicxmatch-api/pulls)

---

<div align="center">

**Made with ❤️ for Spotify & ivLyrics users**

[⬆ 맨 위로](#-ivlyrics-lyrics-providers)

</div>
