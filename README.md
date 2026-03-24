# ivLyrics MusicXMatch Provider (Node.js)

MusicXMatch 가사 제공자 애드온 for ivLyrics

## 특징

- ✅ 동기화 가사 (richsync) 지원
- ✅ 일반 가사 지원
- ✅ 자동 트랙 매칭 (스코어링 시스템)
- ✅ 캐싱 (30분)
- ✅ 자동 시작 설정

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

## 사용법

1. 서버가 자동으로 시작됩니다 (http://localhost:8092)
2. ivLyrics 설정에서 MusicXMatch 애드온 활성화
3. 서버 URL 확인: `http://localhost:8092`

## 수동 설치

```bash
npm install
node server.js
```

---

## Python API (Legacy)

Python 버전은 `legacy/` 폴더에 있습니다.


