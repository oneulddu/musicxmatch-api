# ivLyrics Lyrics Providers

Spotify용 ivLyrics 확장에 MusicXMatch, Deezer, Bugs, Genie 가사를 제공하는 로컬 서버

## 주요 기능

- 🎵 **4개 Provider 지원**: MusicXMatch, Deezer, Bugs, Genie
- ⚡ **동기화 가사**: LRC 형식 타임스탬프 가사 지원
- 🎯 **스마트 매칭**: Spotify Track ID 우선, 자동 fallback
- 💾 **캐싱**: 30분 자동 캐시로 빠른 응답
- 🦀 **Rust 기반**: 단일 바이너리, 낮은 메모리 사용
- 🔄 **자동 시작**: OS 부팅 시 백그라운드 실행

---

## 빠른 시작

### 자동 설치 (권장)

**Windows (PowerShell)**
```powershell
iwr -useb "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.ps1" | iex
```

관리자 권한은 보통 필요하지 않습니다. Windows에서는 기본적으로 `Startup` 폴더 방식으로 자동 시작을 등록합니다. 작업 스케줄러를 꼭 쓰고 싶으면 설치 전에 아래 환경 변수를 주면 됩니다.

```powershell
$env:IVLYRICS_WINDOWS_AUTOSTART = "scheduled-task"
iwr -useb "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.ps1" | iex
```

**macOS / Linux**
```bash
curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.sh | bash
```

설치 스크립트가 자동으로:
1. 로컬 서버 설치 및 자동 시작 설정 (`http://127.0.0.1:8092`)
2. Spicetify Extensions에 4개 애드온 파일 배치
3. 기존 extension 목록 유지하며 자동 등록
4. `spicetify apply` 실행
5. 서버 health check 및 CORS 헤더 검증

### 사용법

1. ivLyrics 설정에서 원하는 provider 활성화
2. Spotify에서 음악 재생
3. 가사가 자동으로 표시됨

**Deezer 사용 시**: ivLyrics 설정에서 Deezer ARL 쿠키 입력 필요 (아래 참조)

---

## Provider 설정

### MusicXMatch
- 별도 설정 불필요
- 자동으로 세션 토큰 관리

### Deezer
Deezer 가사를 사용하려면 ARL 쿠키 필요:

1. [Deezer 웹 플레이어](https://www.deezer.com) 로그인
2. 브라우저 개발자 도구 → Application/Storage → Cookies
3. `arl` 값 복사
4. ivLyrics 설정 → Deezer Provider → Deezer cookie에 붙여넣기

**환경 변수로 설정** (선택사항):
```bash
export DEEZER_ARL=your_arl_cookie_value
```

### Bugs / Genie
- 별도 설정 불필요
- 로그인 없이 공개 API 사용

---

## 고급 설정

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

### 수동 애드온 설치

자동 설치 스크립트를 사용하지 않는 경우:

**Windows**
```powershell
$ext = "$env:APPDATA\spicetify\Extensions"
New-Item -ItemType Directory -Force $ext | Out-Null
$base = "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main"
@("MusicXMatch", "Deezer", "Bugs", "Genie") | ForEach-Object {
    Invoke-WebRequest "$base/Addon_Lyrics_$_.js" -OutFile "$ext\Addon_Lyrics_$_.js"
    spicetify config extensions "Addon_Lyrics_$_.js"
}
spicetify apply
```

**macOS / Linux**
```bash
mkdir -p ~/.config/spicetify/Extensions
base="https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main"
for p in MusicXMatch Deezer Bugs Genie; do
    curl -fsSL "$base/Addon_Lyrics_$p.js" -o ~/.config/spicetify/Extensions/Addon_Lyrics_$p.js
    spicetify config extensions "Addon_Lyrics_$p.js"
done
spicetify apply
```

---

## 업데이트

### ivLyrics 설정에서 (권장)
- **Update server**: 서버만 업데이트
- **Update all**: 서버 + 애드온 + spicetify apply

### 수동 업데이트
설치 스크립트 재실행:

**Windows**
```powershell
iwr -useb "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.ps1" | iex
```

**macOS / Linux**
```bash
curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.sh | bash
```

---

## 제거

**Windows**
```powershell
# 서버 제거
iwr -useb "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/uninstall.ps1" | iex

# 애드온 제거
@("MusicXMatch", "Deezer", "Bugs", "Genie") | ForEach-Object {
    Remove-Item "$env:APPDATA\spicetify\Extensions\Addon_Lyrics_$_.js" -Force -ErrorAction SilentlyContinue
    spicetify config extensions "Addon_Lyrics_$_.js-"
}
spicetify apply
```

**macOS / Linux**
```bash
# 서버 제거
curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/uninstall.sh | bash

# 애드온 제거
for p in MusicXMatch Deezer Bugs Genie; do
    rm -f ~/.config/spicetify/Extensions/Addon_Lyrics_$p.js
    spicetify config extensions "Addon_Lyrics_$p.js-"
done
spicetify apply
```

---

## API 엔드포인트

### GET /health
서버 상태 확인
```bash
curl http://127.0.0.1:8092/health
```

응답:
```json
{
  "status": "ok",
  "version": "0.7.3",
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

### GET /lyrics
가사 조회
```bash
curl "http://127.0.0.1:8092/lyrics?title=Love%20Love%20Love&artist=에픽하이"
```

파라미터:
- `title`: 곡 제목 (필수*)
- `artist`: 아티스트 (필수*)
- `spotifyId`: Spotify Track ID (선택, 있으면 title/artist 불필요)
- `durationMs`: 곡 길이 (밀리초, 선택)
- `backend`: provider 지정 (auto/musicxmatch/deezer/bugs/genie, 기본: auto)
- `debug`: 디버그 정보 포함 (0/1, 기본: 0)

*spotifyId가 있으면 title/artist 생략 가능

### GET /config
Deezer 설정 확인
```bash
curl http://127.0.0.1:8092/config
```

### POST /config
Deezer ARL 저장
```bash
curl -X POST http://127.0.0.1:8092/config \
  -H 'Content-Type: application/json' \
  -d '{"deezerArl":"your_arl_cookie"}'
```

### DELETE /cache
캐시 초기화
```bash
curl -X DELETE http://127.0.0.1:8092/cache
```

---

## 로그 확인

**Windows**
```powershell
Get-Content "$env:USERPROFILE\.ivlyrics-musicxmatch\server.log" -Wait
```

**macOS / Linux**
```bash
tail -f ~/.ivlyrics-musicxmatch/server.log
```

---

## 개발

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
./scripts/bump_version.sh 0.7.4
```

---

## 문제 해결

### 가사가 표시되지 않음
1. 서버 상태 확인: `curl http://127.0.0.1:8092/health`
2. 로그 확인: `~/.ivlyrics-musicxmatch/server.log`
3. ivLyrics 설정에서 provider 활성화 확인
4. Spicetify extension 등록 확인: `spicetify config`

### Deezer 가사가 안 나옴
1. ARL 쿠키 설정 확인
2. `/config` 엔드포인트로 설정 상태 확인
3. Deezer 로그인 상태 확인 (쿠키 만료 가능)

### 서버가 시작되지 않음
**Windows**: 작업 스케줄러에서 "ivLyrics-MusicXMatch" 작업 확인
**macOS**: `launchctl list | grep ivlyrics`
**Linux**: `systemctl --user status ivlyrics-musicxmatch`

---

## 면책 조항

본 문서에 제공된 내용은 순수하게 교육 목적으로만 제공됩니다. 무단 배포, 복제, 변경, 또는 불법 활동에 사용하는 등 본 목적에 반하는 정보의 오용이나 남용은 엄격히 금지되며, 관련 법률 및 규정 위반에 해당할 수 있습니다. 이는 법적 조치를 포함한 심각한 결과를 초래할 수 있습니다. 교육 자료는 책임감 있게, 윤리적으로, 그리고 성실하게 사용되어야 합니다. 본 약관을 위반하는 것으로 확인된 사용자에 대해서는 해당 자료에 대한 접근을 제한할 권리를 보유합니다.

---

## 라이선스

MIT License

---

## 기여

이슈 및 PR 환영합니다: https://github.com/oneulddu/musicxmatch-api
