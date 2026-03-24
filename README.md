# ivLyrics MusicXMatch Provider

MusicXMatch 가사 제공자 애드온 for ivLyrics

## 특징

- ✅ 동기화 가사 (richsync) 지원
- ✅ 일반 가사 지원
- ✅ Spotify Track ID 우선 매칭
- ✅ 자동 트랙 매칭 (Android API 기반)
- ✅ 캐싱 (30분)
- ✅ Rust 단일 바이너리 서버

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

## ivLyrics 애드온 추가

`Addon_Lyrics_MusicXMatch.js`는 ivLyrics 내부 파일이 아니라 Spicetify extension으로 등록해야 합니다.

### Windows

```powershell
New-Item -ItemType Directory -Force "$env:APPDATA\spicetify\Extensions" | Out-Null
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/Addon_Lyrics_MusicXMatch.js" -OutFile "$env:APPDATA\spicetify\Extensions\Addon_Lyrics_MusicXMatch.js"
spicetify config extensions Addon_Lyrics_MusicXMatch.js
spicetify apply
```

### macOS / Linux

```bash
mkdir -p ~/.config/spicetify/Extensions
curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/Addon_Lyrics_MusicXMatch.js \
  -o ~/.config/spicetify/Extensions/Addon_Lyrics_MusicXMatch.js
spicetify config extensions Addon_Lyrics_MusicXMatch.js
spicetify apply
```

애드온 파일 위치:

```text
Windows: %AppData%\spicetify\Extensions\Addon_Lyrics_MusicXMatch.js
~/.config/spicetify/Extensions/Addon_Lyrics_MusicXMatch.js
```

ivLyrics 앱 폴더 위치:

```text
Windows: %LocalAppData%\spicetify\CustomApps\ivLyrics
~/.config/spicetify/CustomApps/ivLyrics
```

이미 다른 extension을 사용 중이면 `spicetify config extensions` 값을 덮어쓰지 않도록 기존 목록에 `Addon_Lyrics_MusicXMatch.js`를 추가하세요.

## 사용법

1. 서버가 자동으로 시작됩니다 (`http://127.0.0.1:8092`)
2. ivLyrics가 `~/.config/spicetify/CustomApps/ivLyrics`에 설치되어 있어야 합니다
3. Spicetify extension으로 `Addon_Lyrics_MusicXMatch.js`가 등록되어 있어야 합니다
4. ivLyrics 설정에서 MusicXMatch 애드온 활성화
5. 서버 URL 확인: `http://127.0.0.1:8092`

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
spicetify config extensions Addon_Lyrics_MusicXMatch.js-
spicetify apply
```

제거되는 주요 위치:

```text
%USERPROFILE%\.ivlyrics-musicxmatch
%USERPROFILE%\.cargo\bin\ivlyrics-musicxmatch-server.exe
%APPDATA%\spicetify\Extensions\Addon_Lyrics_MusicXMatch.js
```

### macOS / Linux

서버 제거:

```bash
curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/uninstall.sh | bash
```

애드온 제거:

```bash
rm -f ~/.config/spicetify/Extensions/Addon_Lyrics_MusicXMatch.js
spicetify config extensions Addon_Lyrics_MusicXMatch.js-
spicetify apply
```

제거되는 주요 위치:

```text
~/.ivlyrics-musicxmatch
~/.cargo/bin/ivlyrics-musicxmatch-server
~/.config/spicetify/Extensions/Addon_Lyrics_MusicXMatch.js
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

가사 요청도 직접 확인할 수 있습니다.

```bash
curl "http://127.0.0.1:8092/lyrics?title=Love%20Love%20Love&artist=%EC%97%90%ED%94%BD%ED%95%98%EC%9D%B4"
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

macOS/Linux:

```bash
tail -f ~/.ivlyrics-musicxmatch/server.log
```
