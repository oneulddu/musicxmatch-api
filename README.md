# ivLyrics Lyrics Providers

MusicXMatch / Deezer / Bugs 가사 제공자 애드온 for ivLyrics

## 특징

- ✅ 동기화 가사 (richsync) 지원
- ✅ 일반 가사 지원
- ✅ Spotify Track ID 우선 매칭
- ✅ 자동 트랙 매칭 (Android API 기반)
- ✅ MusicXMatch / Deezer / Bugs provider 분리 지원
- ✅ 캐싱 (30분)
- ✅ Rust 단일 바이너리 서버

## 개발 메모

`Addon_Lyrics_MusicXMatch.js`, `Addon_Lyrics_Deezer.js`, `Addon_Lyrics_Bugs.js`, `manifest.json`은 수동 편집 대상이 아닙니다.
소스는 `scripts/addon_definitions.json`과 `scripts/generate_addons.js`에 두고, 변경 후 아래 명령으로 다시 생성합니다.

```bash
node scripts/generate_addons.js
```

버전을 올릴 때는 아래 스크립트를 사용하면 정의 파일 갱신과 addon/manifest 재생성이 같이 처리됩니다.

```bash
./scripts/bump_version.sh 0.6.2
```

## 설치

### Windows

PowerShell에서 실행:

```powershell
iwr -useb "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.ps1" | iex
```

### macOS / Linux

터미널에서 실행:

```bash
curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.sh | bash
```

이 명령은 Rust 기반 로컬 서버를 설치하고, `http://127.0.0.1:8092`로 자동 시작되도록 설정합니다.
또한 `Addon_Lyrics_MusicXMatch.js`, `Addon_Lyrics_Deezer.js`, `Addon_Lyrics_Bugs.js`를 Spicetify Extensions 폴더에 배치하고, 기존 extension 목록을 유지한 채 자동으로 등록한 뒤 `spicetify apply`까지 실행합니다.
설치 스크립트는 마지막 단계에서 `/health` 응답과 `Access-Control-Allow-Origin: *` 헤더까지 검사합니다.
Windows에서는 작업 스케줄러가 직접 exe를 실행하지 않고 `run-server.ps1` 래퍼를 통해 로그를 남기면서 서버를 실행합니다.

## ivLyrics 애드온 추가

기본 설치 스크립트를 썼다면 이 단계는 자동으로 끝납니다. 아래 내용은 수동으로 다시 등록하거나, 애드온만 따로 갱신하고 싶을 때 사용하면 됩니다.

`Addon_Lyrics_MusicXMatch.js`, `Addon_Lyrics_Deezer.js`, `Addon_Lyrics_Bugs.js`는 ivLyrics 내부 파일이 아니라 Spicetify extension으로 등록해야 합니다.

### Windows

```powershell
New-Item -ItemType Directory -Force "$env:APPDATA\spicetify\Extensions" | Out-Null
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/Addon_Lyrics_MusicXMatch.js" -OutFile "$env:APPDATA\spicetify\Extensions\Addon_Lyrics_MusicXMatch.js"
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/Addon_Lyrics_Deezer.js" -OutFile "$env:APPDATA\spicetify\Extensions\Addon_Lyrics_Deezer.js"
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/Addon_Lyrics_Bugs.js" -OutFile "$env:APPDATA\spicetify\Extensions\Addon_Lyrics_Bugs.js"
spicetify config extensions Addon_Lyrics_MusicXMatch.js
spicetify config extensions Addon_Lyrics_Deezer.js
spicetify config extensions Addon_Lyrics_Bugs.js
spicetify apply
```

### macOS / Linux

```bash
mkdir -p ~/.config/spicetify/Extensions
curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/Addon_Lyrics_MusicXMatch.js \
  -o ~/.config/spicetify/Extensions/Addon_Lyrics_MusicXMatch.js
curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/Addon_Lyrics_Deezer.js \
  -o ~/.config/spicetify/Extensions/Addon_Lyrics_Deezer.js
curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/Addon_Lyrics_Bugs.js \
  -o ~/.config/spicetify/Extensions/Addon_Lyrics_Bugs.js
spicetify config extensions Addon_Lyrics_MusicXMatch.js
spicetify config extensions Addon_Lyrics_Deezer.js
spicetify config extensions Addon_Lyrics_Bugs.js
spicetify apply
```

애드온 파일 위치:

```text
Windows: %AppData%\spicetify\Extensions\Addon_Lyrics_MusicXMatch.js
Windows: %AppData%\spicetify\Extensions\Addon_Lyrics_Deezer.js
Windows: %AppData%\spicetify\Extensions\Addon_Lyrics_Bugs.js
~/.config/spicetify/Extensions/Addon_Lyrics_MusicXMatch.js
~/.config/spicetify/Extensions/Addon_Lyrics_Deezer.js
~/.config/spicetify/Extensions/Addon_Lyrics_Bugs.js
```

ivLyrics 앱 폴더 위치:

```text
Windows: %LocalAppData%\spicetify\CustomApps\ivLyrics
~/.config/spicetify/CustomApps/ivLyrics
```

이미 다른 extension을 사용 중이면 `spicetify config extensions` 값을 덮어쓰지 않도록 기존 목록에 세 파일을 각각 추가하세요.

## 사용법

1. 서버가 자동으로 시작됩니다 (`http://127.0.0.1:8092`)
2. ivLyrics가 `~/.config/spicetify/CustomApps/ivLyrics`에 설치되어 있어야 합니다
3. Spicetify extension으로 `Addon_Lyrics_MusicXMatch.js`, `Addon_Lyrics_Deezer.js`, `Addon_Lyrics_Bugs.js`가 등록되어 있어야 합니다
4. ivLyrics 설정에서 원하는 provider를 각각 활성화
5. 서버 URL 확인: `http://127.0.0.1:8092`

## Deezer fallback 설정

Deezer provider를 사용하려면 인증용 `arl` 쿠키가 필요합니다.

설정 방법:
1. Deezer 웹 플레이어에 로그인
2. 브라우저 개발자 도구의 쿠키에서 `arl` 값을 복사
3. ivLyrics의 `Deezer Provider` 설정 화면에서 `Deezer cookie` 입력칸에 붙여넣고 저장

저장 위치:

```text
Windows: %USERPROFILE%\.ivlyrics-musicxmatch\config.json
macOS/Linux: ~/.ivlyrics-musicxmatch/config.json
```

원하면 환경 변수로도 줄 수 있습니다.

```text
DEEZER_ARL=<your_cookie_value>
```

환경 변수 `DEEZER_ARL`이 있으면 설정 파일보다 우선합니다.

## Bugs provider 동작 방식

Bugs provider는 별도 로그인 없이 아래 흐름으로 동작합니다.
1. Spotify에서 받은 `title`, `artist`, `duration`으로 Bugs 모바일 검색 API에서 후보를 찾음
2. 제목/아티스트/길이 점수로 가장 적절한 `track_id`를 고름
3. `https://music.bugs.co.kr/player/lyrics/T/{track_id}` 에서 synced lyrics 조회
4. synced가 없으면 `https://music.bugs.co.kr/player/lyrics/N/{track_id}` 에서 plain lyrics 조회

즉 `track_id`를 미리 알고 있을 필요는 없습니다.

## 수동 설치

Rust가 설치되어 있다면 서버만 직접 실행할 수 있습니다:

```bash
cargo run
```

서버 상태 확인:

```bash
curl http://127.0.0.1:8092/health
```

## 업데이트

최신 버전으로 올리려면 설치 스크립트를 다시 실행한 뒤 서버를 재시작하세요.

### Windows

```powershell
iwr -useb "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.ps1" | iex
spicetify apply
```

서버 재시작:

```powershell
Start-ScheduledTask -TaskName "ivLyrics-MusicXMatch"
```

### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.sh | bash
spicetify apply
```

서버 재시작:

```bash
launchctl kickstart -k gui/$(id -u)/com.ivlyrics.musicxmatch
```

Linux(systemd user)에서는:

```bash
systemctl --user restart ivlyrics-musicxmatch
```

설정 화면에는 두 가지 버튼이 있습니다.
- `Update now`: 서버만 다시 설치하고 재시작합니다. 애드온 파일과 `spicetify apply`는 건너뜁니다.
- `Update all`: 설치 스크립트를 전체 모드로 다시 실행해서 서버, 애드온 파일, `spicetify apply`를 한 번에 처리합니다.

`Update all`은 Spotify UI가 다시 적용되므로 실행 시 잠깐 재로딩될 수 있습니다.
두 버튼 모두 백그라운드 작업으로 예약되며, macOS/Linux에서는 `~/.ivlyrics-musicxmatch/update.log`에서 진행 상태를 바로 확인할 수 있습니다.

## 제거

서버 제거와 애드온 제거는 별개입니다. 둘 다 지우려면 아래 순서로 진행하세요.

### Windows

서버 제거:

```powershell
iwr -useb "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/uninstall.ps1" | iex
```

애드온 제거:

```powershell
Remove-Item "$env:APPDATA\spicetify\Extensions\Addon_Lyrics_MusicXMatch.js" -Force -ErrorAction SilentlyContinue
Remove-Item "$env:APPDATA\spicetify\Extensions\Addon_Lyrics_Deezer.js" -Force -ErrorAction SilentlyContinue
Remove-Item "$env:APPDATA\spicetify\Extensions\Addon_Lyrics_Bugs.js" -Force -ErrorAction SilentlyContinue
spicetify config extensions Addon_Lyrics_MusicXMatch.js-
spicetify config extensions Addon_Lyrics_Deezer.js-
spicetify config extensions Addon_Lyrics_Bugs.js-
spicetify apply
```

제거되는 주요 위치:

```text
%USERPROFILE%\.ivlyrics-musicxmatch
%USERPROFILE%\.cargo\bin\ivlyrics-musicxmatch-server.exe
%APPDATA%\spicetify\Extensions\Addon_Lyrics_MusicXMatch.js
%APPDATA%\spicetify\Extensions\Addon_Lyrics_Deezer.js
%APPDATA%\spicetify\Extensions\Addon_Lyrics_Bugs.js
```

### macOS / Linux

서버 제거:

```bash
curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/uninstall.sh | bash
```

애드온 제거:

```bash
rm -f ~/.config/spicetify/Extensions/Addon_Lyrics_MusicXMatch.js
rm -f ~/.config/spicetify/Extensions/Addon_Lyrics_Deezer.js
rm -f ~/.config/spicetify/Extensions/Addon_Lyrics_Bugs.js
spicetify config extensions Addon_Lyrics_MusicXMatch.js-
spicetify config extensions Addon_Lyrics_Deezer.js-
spicetify config extensions Addon_Lyrics_Bugs.js-
spicetify apply
```

제거되는 주요 위치:

```text
~/.ivlyrics-musicxmatch
~/.cargo/bin/ivlyrics-musicxmatch-server
~/.config/spicetify/Extensions/Addon_Lyrics_MusicXMatch.js
~/.config/spicetify/Extensions/Addon_Lyrics_Deezer.js
~/.config/spicetify/Extensions/Addon_Lyrics_Bugs.js
```

## CORS / Health 확인

Spotify 웹뷰에서 로컬 서버를 읽으려면 CORS 헤더가 있어야 합니다. 아래 응답에 `access-control-allow-origin: *`가 보여야 정상입니다.

```bash
curl -i http://127.0.0.1:8092/health
```

정상 응답 예시:

```text
HTTP/1.1 200 OK
access-control-allow-origin: *
content-type: application/json
```

응답 JSON에는 아래 정보도 포함됩니다.

```text
status
version
provider
backend
cors
deezerConfigured
cacheEntries
sessionFile
logFile
```

가사 요청도 직접 확인할 수 있습니다.

```bash
curl "http://127.0.0.1:8092/lyrics?title=Love%20Love%20Love&artist=%EC%97%90%ED%94%BD%ED%95%98%EC%9D%B4"
```

매칭 과정을 같이 보고 싶으면 `debug=1`을 붙이세요.

```bash
curl "http://127.0.0.1:8092/lyrics?title=Love%20Love%20Love&artist=%EC%97%90%ED%94%BD%ED%95%98%EC%9D%B4&debug=1"
```

이 경우 응답에 아래 같은 디버그 정보가 추가됩니다.

```text
debug.source
debug.matchedBy
debug.durationMs
debug.selectedTrackId
debug.selectedTrackDurationMs
debug.searchVariants
```

업데이트 상태도 확인할 수 있습니다.

```bash
curl http://127.0.0.1:8092/update/check
```

업데이트를 예약하려면:

```bash
curl -X POST http://127.0.0.1:8092/update/apply
```

서버 + 애드온 + `spicetify apply`까지 한 번에 예약하려면:

```bash
curl -X POST http://127.0.0.1:8092/update/apply-all
```

Deezer 설정 상태를 확인하거나 저장하려면:

```bash
curl http://127.0.0.1:8092/config
curl -X POST http://127.0.0.1:8092/config \
  -H 'Content-Type: application/json' \
  -d '{"deezerArl":"YOUR_ARL_COOKIE"}'
```

## 로그

기본 로그 파일 위치:

```text
Windows: %USERPROFILE%\.ivlyrics-musicxmatch\server.log
macOS/Linux: ~/.ivlyrics-musicxmatch/server.log
```

Windows에서 로그를 실시간으로 보려면:

```powershell
Get-Content "$env:USERPROFILE\.ivlyrics-musicxmatch\server.log" -Wait
```

stdout/stderr까지 같이 보려면:

```powershell
Get-Content "$env:USERPROFILE\.ivlyrics-musicxmatch\server.stdout.log" -Wait
```

macOS/Linux:

```bash
tail -f ~/.ivlyrics-musicxmatch/server.log
```

`Update now` / `Update all` 백그라운드 작업 로그:

```bash
tail -f ~/.ivlyrics-musicxmatch/update.log
```

## ivLyrics 로컬 패치 메모

현재 `ivLyrics` 원본은 두 가지 이유로 로컬 패치를 해두는 편이 안정적입니다.
- 빈 가사 배열 `[]`도 성공처럼 처리할 수 있어서, 앞선 provider가 빈 결과를 반환하면 다음 provider까지 내려오지 못하는 경우가 있습니다.
- 여러 provider가 켜져 있을 때 먼저 나온 unsynced 결과를 바로 써서, 뒤에 있는 synced 결과를 놓칠 수 있습니다.

로컬에서 아래 파일을 패치해 두면 안정적입니다.
- [LyricsAddonManager.js](/Users/oneul/Desktop/Workspace/ivLyrics/LyricsAddonManager.js)

핵심 내용:
- `karaoke`, `synced`, `unsynced`를 단순 truthy가 아니라 `배열 길이 > 0`으로 판단
- 빈 결과면 다음 provider로 계속 진행
- 여러 provider 결과를 비교해서 `karaoke > synced > unsynced` 순으로 더 좋은 결과를 선택
- 앞 provider가 unsynced이고 뒤 provider가 synced면, 뒤 provider의 synced를 우선 선택

이 수정은 `ivLyrics` 업데이트 시 덮어써질 수 있으니, 업데이트 후 가사가 다시 안 뜨면 가장 먼저 이 부분을 확인하는 편이 좋습니다.
