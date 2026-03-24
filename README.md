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

이 명령은 Rust 기반 로컬 서버를 설치하고, `http://localhost:8092`로 자동 시작되도록 설정합니다.

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

1. 서버가 자동으로 시작됩니다 (http://localhost:8092)
2. ivLyrics가 `~/.config/spicetify/CustomApps/ivLyrics`에 설치되어 있어야 합니다
3. Spicetify extension으로 `Addon_Lyrics_MusicXMatch.js`가 등록되어 있어야 합니다
4. ivLyrics 설정에서 MusicXMatch 애드온 활성화
5. 서버 URL 확인: `http://localhost:8092`

## 수동 설치

Rust가 설치되어 있다면 서버만 직접 실행할 수 있습니다:

```bash
cargo run
```

서버 상태 확인:

```bash
curl http://127.0.0.1:8092/health
```

---

## Python API (Legacy)

Python 버전은 `legacy/` 폴더에 있습니다.
