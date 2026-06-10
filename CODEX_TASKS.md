# Codex 작업 지시서 — ivLyrics MusicXMatch Server 최적화/개선

> 작성일: 2026-06-10 · 기준 버전: 0.7.17 (commit 6539ddd) · 현재 작업 버전: 0.7.19
> 기존 [IMPROVEMENTS.md](IMPROVEMENTS.md)는 "백로그 메모"이고, 이 문서는 **실제 코드 위치 기준의 실행 지시서**입니다.
> 항목별로 독립적으로 작업·커밋 가능하며, 우선순위 순서대로 진행하세요.

---

## 공통 작업 규칙

1. **검증 명령** — 모든 작업 후 아래가 통과해야 함:
   ```bash
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   cargo test
   cargo build --release
   ```
2. **외부 동작 호환성 유지** — `/lyrics`, `/health`, `/config`, `/update/*` 응답 스키마(JSON 필드명·타입)는 변경 금지. 추가는 허용.
3. **새 의존성은 항목에 명시된 것만** 추가. 그 외에는 표준 라이브러리 + 기존 의존성으로 해결.
4. **민감정보 로그 금지** — ARL, usertoken은 절대 원문 출력하지 않음 (`mask_secret` 정책 유지).
5. 커밋 메시지는 기존 컨벤션(`fix:`, `feat:`, `perf:`, `refactor:`, `chore:` + 한국어 요약)을 따름.
6. **작업 시작 전 Git 기준 확인** — 각 작업 시작 전 `git status`, 현재 브랜치, `origin/main`과의 차이를 확인한다.
7. **PR 순차 진행** — 이전 PR이 `main`에 병합되고 `main`의 test/deploy 상태까지 확인된 뒤 다음 작업을 시작한다.
8. **Codex 리뷰 확인** — Codex 리뷰가 `eyes` 상태면 기다리고, `+1`이면 남은 댓글이나 스레드가 없는지 확인한다.
9. **리뷰/CI 대응** — 리뷰 코멘트나 CI 실패가 있으면 같은 PR에서 수정 커밋으로 대응하고, 관련 스레드에 답변한 뒤 resolved 처리한다.
10. **병합 차단 보고** — 외부 승인이나 권한 문제로 병합할 수 없으면 PR URL, 체크 상태, 막힌 이유를 보고하고 멈춘다.

---

## P1 — 체감 효과가 큰 항목

### T1. Auto 모드에서 동기화 가사 우선 선택 (동작 개선)

**문제**
README는 "auto는 provider를 순회하며 **가장 좋은 가사**를 자동 선택"이라고 약속하지만, 실제 구현(`src/main.rs:1673` `BackendMode::Auto` 분기)은 **첫 성공을 즉시 반환**합니다.
MusicXMatch가 unsynced(text-only) 가사만 반환해도 거기서 멈추기 때문에, Bugs/Genie에 synced(LRC) 가사가 있어도 사용자는 일반 가사만 보게 됩니다. 한국 음악에서 특히 자주 발생.

**지시**
- `fetch_payload`의 Auto 분기 수정: provider가 성공했지만 `payload.lrc.is_none()`(text만 있음)이면, 그 결과를 "보류 후보"로 들고 다음 provider를 계속 시도.
- 어떤 provider든 `lrc`가 있는 결과를 얻으면 즉시 그것을 반환.
- 모든 provider 시도 후 LRC가 없으면 보류해 둔 첫 text-only 결과 반환.
- 기존 에러 우선순위 로직(MxmError::NotFound 등 매핑)은 "전부 실패한 경우"에만 적용되도록 유지.
- 로그: 보류/승격 시 `"text-only 결과 보류, synced 가사 탐색 계속"` 류의 태그 로그 추가.

**완료 기준**
- mock 또는 단위 테스트로 "provider A가 text-only, provider B가 lrc 보유 → B 결과 채택" 시나리오 검증 (provider fetch 함수를 직접 호출하기 어려우면 선택 로직을 순수 함수로 분리해 테스트).
- 캐시에는 최종 선택 결과만 저장됨을 확인.

### T2. Auto 모드 fallback 병렬화 (지연시간 단축)

**문제**
`src/main.rs:1673-1745` — MusicXMatch 실패 후 Deezer → Bugs → Genie를 **순차** 시도합니다. 각 provider는 timeout 10초 + 내부 variant 검색(최대 수십 회 호출)이므로 최악의 경우 응답까지 수십 초가 걸립니다. 가사 표시는 곡 재생 시작 직후가 중요하므로 체감이 큽니다.

**지시**
- MusicXMatch 실패(또는 T1 적용 후 text-only 보류) 시, Deezer(설정된 경우)·Bugs·Genie를 `tokio::join!` 또는 `futures::future::join_all` 등으로 **동시에** 실행.
  - `futures` crate가 필요하면 추가 허용 (`futures = "0.3"` 또는 `tokio` util만으로 해결 가능하면 그쪽 우선).
- 결과 선택 우선순위: ① lrc 보유 결과 중 provider 우선순위(deezer > bugs > genie 순서는 현 fallback 순서 유지), ② text-only 결과.
- 에러 매핑 로직은 기존 `map_error` 의미 유지: 전부 실패 시 반환할 대표 에러는 현행 우선순위 규칙(`deezer_error` > `bugs_error` ...)을 보존.
- T1과 함께 작업하면 자연스럽게 합쳐지므로 **T1 → T2 순서로 같은 PR**에서 진행 권장.

**완료 기준**
- `backend=auto` 동작이 순차 대비 빨라지는 구조 확인 (코드 리뷰 수준으로 충분, 실측 불요).
- 단일 backend 지정(`backend=bugs` 등) 경로는 변경 없음.

### T3. async 컨텍스트의 블로킹 호출 제거

**문제**
`spawn_addon_restore_task`(`src/main.rs:1196`)가 5분마다 도는 async 태스크 안에서 다음 블로킹 호출을 실행합니다:
- `apply_spicetify_changes` → `stop_spotify_if_running`의 `std::thread::sleep(Duration::from_secs(2))` (`src/main.rs:1295`, `1341`)
- `Command::new(...).output()` 동기 실행 (spicetify apply, pgrep, osascript 등)

tokio 워커 스레드를 수 초간 점유하여, 그 동안 `/lyrics` 요청 처리가 지연될 수 있습니다.

**지시**
- `apply_spicetify_changes()` 호출 전체를 `tokio::task::spawn_blocking`으로 감싸기 (내부 함수들은 동기 그대로 유지해도 됨).
- `restore_known_provider_addons` 내 `std::fs::write`/`read_to_string` 등은 빈도가 낮아 그대로 둬도 무방 (선택).

**완료 기준**
- async fn 또는 tokio 태스크 본문에서 `std::thread::sleep` / 동기 `Command::output()` 직접 호출이 사라짐.
- `cargo clippy` 통과.

### T4. release 프로필 최적화 (바이너리 크기·설치 시간)

**문제**
`Cargo.toml`에 `[profile.release]` 설정이 없습니다. 이 서버는 사용자가 설치 스크립트로 빌드/다운로드하는 단일 바이너리라 크기 절감 효과가 직접적입니다. (IMPROVEMENTS.md 5.2에서 이미 승인된 방향)

**지시**
`Cargo.toml`에 추가:
```toml
[profile.release]
lto = true
codegen-units = 1
strip = true
panic = "abort"
opt-level = "z"   # 크기 우선. 빌드해보고 문제 없으면 유지, 성능 우려 시 "s"
```
- `panic = "abort"`는 `expect()` 기반 부팅 실패 메시지가 그대로 동작하는지 확인 후 적용. 문제가 되면 제외.

**완료 기준**
- `cargo build --release` 성공, `cargo test`는 dev 프로필이므로 영향 없음.
- 적용 전/후 바이너리 크기를 커밋 메시지에 기록.

---

## P2 — 구조 정리 및 안정성

### T5. resolve_* / is_acceptable_* 4중복 제거

**문제**
- `src/main.rs:2126-2315` — `resolve_deezer_tracks`, `resolve_bugs_tracks`, `resolve_genie_tracks`가 사실상 동일한 ~60줄 로직의 복붙 (variant 이중 루프 → exact 후보 조기 종료 → title-only fallback → 점수 정렬 → 필터).
- `src/matching.rs:268-377` — `is_acceptable_deezer_match` / `is_acceptable_bugs_match` / `is_acceptable_genie_match`도 거의 동일 (Deezer만 duration 조건이 빠짐).

**지시**
- 공통 트랙 표현을 위한 경량 trait 도입. 예:
  ```rust
  trait SearchableTrack {
      fn id(&self) -> u64;
      fn name(&self) -> &str;
      fn artist(&self) -> &str;
      fn duration_ms(&self) -> Option<u64>;
  }
  ```
  `DeezerTrack`/`BugsTrack`/`GenieTrack`에 구현 (필드가 동일하므로 단순).
- 제네릭 `resolve_provider_tracks<T: SearchableTrack>(search_fn, score_fn, accept_fn, ...)` 형태로 main.rs의 3개 resolve 함수를 1개로 통합. `resolve_track`(MusicXMatch)은 matcher fallback 단계가 달라 별도 유지 가능 — 단, variant 루프 부분은 공용 helper 재사용.
- `is_acceptable_*` 3개를 `is_acceptable_provider_match(track_name, artist_name, title, artist, matched_by, duration_pair)` 하나로 통합. Deezer는 duration `None` 전달로 현행 동작 유지.
- **점수/임계값 숫자는 1도 바꾸지 말 것.** 순수 리팩터링.

**완료 기준**
- `cargo test` 전체 통과 (기존 매칭 테스트가 회귀 검증 역할).
- main.rs 줄 수 의미 있게 감소 (~150줄 이상).

### T6. provider 모듈 공통 유틸 통합

**문제**
`src/bugs.rs`와 `src/genie.rs`에 byte-단위로 거의 동일한 함수가 중복:
- `send_with_retry`, `is_retryable_status`, `is_retryable_error` (bugs.rs:168-203 ≒ genie.rs:139-174)
- `parse_duration_ms` (bugs.rs:242 ≒ genie.rs:360)
- `format_lrc_timestamp` 계열이 deezer/bugs/genie 3곳에 각각 존재 (반올림 방식만 미세하게 다름)

**지시**
- `src/provider_util.rs` (또는 `src/util.rs`) 신설:
  - `send_with_retry`를 에러 타입 제네릭(`Fn(reqwest::Error|String) -> E` 콜백 또는 `From<String>` 바운드)으로 한 벌만 유지.
  - `parse_duration_ms` 1벌.
  - LRC 타임스탬프 포맷터는 **ms 입력 기준 1벌**(`format_lrc_timestamp_ms`)로 통일하되, 출력이 기존과 동일해야 함 — bugs는 초 단위 입력이므로 호출부에서 ms 변환. 기존 단위 테스트의 기대값이 그대로 통과해야 함.
- 각 provider 파일에서 중복 제거 후 re-use.

**완료 기준**
- `cargo test` 통과 (특히 `parse_synced_bugs_lyrics_into_lrc`, `format_genie_lrc_uses_millisecond_timestamps` 기대값 불변).

### T7. 실패 결과 negative cache (provider 부하·반복 지연 감소)

**문제**
캐시는 성공 결과만 저장합니다(`store_cache`, `src/main.rs:1161`). 가사가 없는 곡은 ivLyrics가 재요청할 때마다 4개 provider × variant 검색을 전부 다시 수행 → 불필요한 외부 호출 + 매번 느린 실패.

**지시**
- `CacheEntry`를 성공/실패 구분 가능하게 확장 (예: `payload: Result<LyricsPayload, (StatusCode, String)>` 또는 enum).
- 실패 TTL은 짧게: `NEGATIVE_CACHE_TTL = 5분` 상수 추가 (성공 30분과 별도).
- **단, 캐시할 실패는 `404 NotFound`/`NotAvailable` 계열만.** 네트워크 오류·429·5xx는 캐시하지 않음 (일시 장애가 5분간 고착되면 안 됨).
- 캐시 히트 시 동일 status/detail로 응답. 로그에 `"실패 캐시 사용"` 표기.

**완료 기준**
- 단위 테스트: NotFound 응답 후 동일 키 재요청이 캐시에서 404 반환. 429는 캐시되지 않음.

### T8. Logger I/O 효율화

**문제**
`src/logging.rs:39-50` — 매 로그 라인마다:
1. `reopen_if_log_too_large`가 **쓰기 전후로 2회** 호출되고, 각 호출이 `file.metadata()` syscall 수행 → 라인당 메타데이터 조회 2회.
2. `flush()`를 매 라인 수행.

요청당 로그가 3~5줄이므로 syscall이 불필요하게 많습니다.

**지시**
- Logger 내부에 누적 바이트 카운터(`AtomicU64` 또는 Mutex 내 u64)를 두고, 회전 체크는 카운터 기준으로 수행. 파일 open/rotate 시점에만 `metadata()` 호출해 카운터 초기화.
- 쓰기 후 1회만 회전 체크 (현재의 이중 호출 제거).
- `flush()`는 유지 (크래시 시 로그 보존이 디버깅에 중요하므로).

**완료 기준**
- 정상 경로에서 라인당 metadata syscall 0회.
- 2MB 초과 시 `.log.1` 회전 동작 유지 (수동 확인 또는 테스트).

### T9. update/버전 체크용 HTTP 클라이언트 재사용

**문제**
`latest_version_info`(`src/main.rs:831`)가 호출될 때마다 `reqwest::Client`를 새로 빌드합니다 (TLS 컨텍스트 초기화 비용). `/health`와 ivLyrics 설정 화면에서 반복 호출되는 경로입니다.

**지시**
- `AppState`에 update용 `reqwest::Client` 1개를 추가하거나 `std::sync::OnceLock<reqwest::Client>` 전역으로 재사용.
- `spawn_addon_restore_task`의 client와 통합 가능하면 통합.

**완료 기준**
- `latest_version_info` 호출 경로에서 Client 빌드가 1회(프로세스 수명 기준)로 줄어듦.

---

## P3 — 정확성 버그 및 소소한 정리

### T10. `strip_featured` 멀티바이트 인덱스 버그 수정 (잠재 panic)

**문제**
`src/matching.rs:420-428`:
```rust
let lower = value.to_lowercase();
if let Some(index) = lower.find(marker) {
    return value[..index].trim().to_string();  // lower의 인덱스를 value에 적용
}
```
`to_lowercase()`는 일부 유니코드(예: `İ` U+0130 → `i̇` 2 chars)에서 **바이트 길이가 달라질 수 있어**, `lower`에서 얻은 인덱스를 `value`에 적용하면 char boundary panic 또는 잘못된 절단이 발생할 수 있습니다. 터키어 등 일부 제목에서 요청 핸들러 panic 가능.

**지시**
- 인덱스를 원본 문자열 기준으로 찾도록 수정. 가장 간단한 방법: marker 비교를 char 단위 case-insensitive 매칭으로 구현하거나, `value`를 char_indices로 순회하며 ASCII lowercase 비교 (marker는 전부 ASCII이므로 `eq_ignore_ascii_case`로 충분).
- 같은 파일의 `first_artist`는 원본에 `find`를 쓰므로 문제 없음 — 변경 불요.

**완료 기준**
- 회귀 테스트 추가: `strip_featured("İstanbul feat. Someone")`이 panic 없이 동작.
- 기존 `artist_variants_strip_featured_and_split_collaborators` 테스트 통과.

### T11. 캐시 엔트리 수 상한

**문제**
`cache: HashMap` (`src/main.rs:73`)은 TTL 정리만 있고 최대 크기 제한이 없습니다. duration·backend별로 키가 분화되므로 장시간 구동 시 무한 증가 가능 (로컬 서버라 위험은 낮지만 상주 데몬이므로 방어 필요).

**지시**
- `CACHE_MAX_ENTRIES: usize = 2_000` 상수 추가.
- `store_cache`에서 초과 시 만료 임박 순(또는 단순히 만료된 것 우선 + 임의) 제거. LRU crate 도입은 과하므로 금지 — 간단한 정리로 충분.

**완료 기준**
- 단위 테스트: 상한 초과 삽입 시 len이 상한 이하 유지.

### T12. `html_entity_decode` 단일 패스화

**문제**
`src/genie.rs:383-399` — `.replace()` 14연쇄로 호출당 최대 14회 String 재할당. 가사 라인마다 호출되므로 곡당 수백 회 할당.

**지시**
- `&`로 split하거나 char 순회 기반 단일 패스 디코더로 교체. 지원 엔티티 목록은 현행 14개 그대로.

**완료 기준**
- 기존 genie 테스트(특히 fixture 기반) 전부 통과.

### T13. 문서·메타데이터 현행화

**문제**
1. `Cargo.toml:6` — `description = "... powered by musixmatch-inofficial"` → 해당 의존성은 제거된 지 오래됨 (IMPROVEMENTS.md 5.1).
2. `.env.example`이 있지만 서버는 dotenv 로딩을 하지 않음 — 환경변수는 실행 환경에서 직접 설정해야 한다는 안내가 없어 오해 소지.
3. README 환경 변수 표에 provider별 timeout 변수(`IVLYRICS_MUSIXMATCH_TIMEOUT_SECS` 등)와 `IVLYRICS_MXM_UPDATE_LOG`가 누락.

**지시**
- Cargo.toml description을 현재 구조에 맞게 수정 (예: "Local lyrics bridge server for ivLyrics (MusicXMatch/Deezer/Bugs/Genie)").
- `.env.example` 상단에 주석 추가: 이 파일은 직접 로드되지 않으며 shell/launchd/systemd 환경 변수로 설정해야 함 (또는 install 스크립트가 사용하는 방식 명시).
- README 환경 변수 표 보강.

**완료 기준**
- 문서 변경만, 코드 변경 없음.

### T14. (선택) `similarity` 품질 개선 검토

**문제**
`src/matching.rs:77-88`의 `similarity`는 **같은 위치의 문자 일치율**만 봅니다. 단어 순서가 다르거나 접두사가 추가된 경우("A Team" vs "The A Team") 점수가 비정상적으로 낮아져 매칭 실패 가능.

**지시 (탐색적 — 동작 변경이므로 신중히)**
- 기존 함수는 유지하고, 토큰 집합 기반 보조 점수(단어 단위 Jaccard 유사도)를 추가해 `max(positional, token_jaccard)`로 사용하는 방안을 구현.
- **반드시 기존 테스트 + 신규 케이스(어순 변경, 관사 추가)로 회귀 확인.** 기존 임계값(0.8/0.45/0.35)과의 상호작용 때문에 매칭이 과도하게 느슨해지면 안 됨.
- 확신이 없으면 이 항목은 PR을 분리하고 변경 영향 분석을 PR 설명에 포함.

**완료 기준**
- 기존 매칭 테스트 전부 통과 + 신규 테스트 4개 이상.

---

## 권장 진행 순서

| 순서 | 항목 | 종류 | 예상 규모 |
|------|------|------|----------|
| 1 | T4 release 프로필 | chore | 소 |
| 2 | T13 문서 현행화 | docs | 소 |
| 3 | T10 strip_featured 버그 | fix | 소 |
| 4 | T1 + T2 Auto 모드 개선 | feat/perf | **대** (한 PR) |
| 5 | T3 블로킹 호출 제거 | fix | 소 |
| 6 | T7 negative cache | feat | 중 |
| 7 | T5 resolve 중복 제거 | refactor | 대 |
| 8 | T6 provider 유틸 통합 | refactor | 중 |
| 9 | T8 Logger 효율화 | perf | 소 |
| 10 | T9 클라이언트 재사용 | perf | 소 |
| 11 | T11, T12 | perf/chore | 소 |
| 12 | T14 (선택) | feat | 중 |

> T5/T6 리팩터링은 T1/T2 동작 변경이 안정화된 **이후**에 진행할 것 (충돌 최소화).
