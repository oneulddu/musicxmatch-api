# ivLyrics MusicXMatch Server - 개선 백로그

## 목적
이 문서는 현재 코드베이스 기준으로 실제로 검토할 가치가 있는 개선 항목만 정리합니다.
이미 반영된 내용, 현재 방향과 맞지 않는 제안, 리스크에 비해 이득이 작은 제안은 제외하거나 보수적으로 다룹니다.

---

## 1. 바로 가치가 큰 항목

### 1.1 캐시 만료 정리 백그라운드 작업
**현재 상태**
- 반영 완료: 10분 간격 백그라운드 정리 작업이 추가되었습니다.

**메모**
- 현재는 expired entry 수와 remaining count를 로그로 남깁니다.
- 추가 메트릭은 아직 없습니다.

### 1.2 검색 variant 조기 종료
**현재 상태**
- 반영 완료: exact title+artist 후보가 나오면 provider별 variant 검색을 조기 종료합니다.

**메모**
- 현재는 과도한 API 호출을 줄이되, `title-only` fallback 자체를 제거하지는 않습니다.

### 1.3 점수 계산 상수화
**현재 상태**
- 반영 완료: 공통 가중치/보너스/패널티가 상수로 분리되었습니다.

**메모**
- provider별 차이는 유지하고, 공통 숫자만 정리한 상태입니다.

---

## 2. 중기 개선 항목

### 2.1 scoring 중복 축소
**현재 상태**
- `MusicXMatch`, `Deezer`, `Bugs`, `Genie` scoring 함수는 비슷하지만 완전히 같지는 않습니다.

**판단**
- 완전한 제네릭 trait 통합은 과할 수 있습니다.
- 다만 공통 골격을 작은 helper로 추출하는 건 가치가 있습니다.

**제안**
- 제목/아티스트 기본 점수 계산
- duration 보정
- noise penalty
를 공통 helper로 분리

**우선순위**
- 중간

### 2.2 provider별 재시도 정책 정리
**현재 상태**
- `MusicXMatch`는 토큰 만료 시 1회 재시도
- `Deezer`는 auth refresh 흐름 있음
- `Bugs`, `Genie`는 별도 재시도 없음

**제안**
- 네트워크 일시 오류에 한해 provider별 1회 재시도 검토
- 모든 오류에 재시도하지 말고 timeout/5xx에만 제한

**우선순위**
- 중간

### 2.3 timeout 설정 외부화
**현재 상태**
- provider 타임아웃은 코드에 고정돼 있습니다.

**제안**
- 환경 변수 하나로 공통 timeout을 주고, 필요하면 provider별 override 허용

**우선순위**
- 중간

### 2.4 health 응답에 provider 상태 확장
**현재 상태**
- 부분 반영: `/health`는 기존 필드 외에 `providerStatuses`를 반환합니다.

**메모**
- 현재 값은 `musicxmatch`, `bugs`, `genie`는 `ready`, `deezer`는 `configured`/`not-configured`입니다.
- 더 세밀한 실시간 provider health는 아직 없습니다.

---

## 3. 보안 및 운영

### 3.1 설정 파일 권한 점검
**현재 상태**
- 반영 완료: `config.json`과 `musixmatch_session.json` 저장 후 Unix 계열에서 `600` 권한 적용을 시도합니다.

**메모**
- 실패해도 기능은 유지하고 로그만 남깁니다.

### 3.2 ARL 및 민감정보 로그 정책 유지
**현재 상태**
- Deezer ARL은 마스킹해서만 노출합니다.

**판단**
- 현재 방향은 적절합니다.
- 환경 변수 기반 설정도 그대로 마스킹 또는 비노출 유지가 맞습니다.

**제안**
- 새 로그 추가 시에도 민감값 직접 출력 금지 원칙 유지

**우선순위**
- 중간

### 3.3 CORS는 현재 방식 유지
**현재 상태**
- `allow_origin(Any)` 사용 중

**판단**
- Spotify 웹뷰 origin이 고정적이지 않아, 섣부른 제한은 실제 동작을 깨뜨릴 가능성이 큽니다.
- 현재는 실용성이 더 중요합니다.

**제안**
- 보수적으로 유지
- 제한은 실제 허용 origin 목록을 충분히 수집한 뒤 검토

**우선순위**
- 낮음

---

## 4. 테스트 및 문서

### 4.1 통합 테스트 확대
**현재 상태**
- 단위 테스트는 충분히 늘어났지만, 네트워크 경계 테스트는 없습니다.

**제안**
- mock 응답 기반 provider 통합 테스트
- 캐시 동작 테스트
- `/config`, `/health`, `/lyrics` handler 테스트

**우선순위**
- 중간

### 4.2 실제 fixture 기반 파서 테스트
**현재 상태**
- `Bugs`, `Genie`는 샘플 문자열 테스트가 있습니다.

**제안**
- 실제 HTML/JSONP/lyrics payload fixture 파일을 둬서 회귀 테스트 강화

**우선순위**
- 중간

### 4.3 문서화
**제안**
- `/lyrics`, `/config`, `/health`, `/update/*` 응답 예시 보강
- provider별 특성
  - `MusicXMatch`: session token
  - `Deezer`: ARL 필요
  - `Bugs`: 무설정
  - `Genie`: 무설정
정리

**우선순위**
- 낮음

---

## 5. 의존성 및 빌드

### 5.1 현재 의존성 방향 유지
**현재 상태**
- `musixmatch-inofficial` 제거 완료
- 현재 의존성은 목적 대비 과하지 않은 편입니다.

**판단**
- 추가적인 “최소화 리팩터링”은 가능하지만 체감 이득이 작고 코드 복잡도만 높일 수 있습니다.

**제안**
- 지금은 유지
- 새 의존성 추가는 명확한 실익이 있을 때만

### 5.2 release profile 최적화 검토
**제안**
- 바이너리 크기와 설치 시간을 줄이기 위해 release profile 최적화 검토

예:
```toml
[profile.release]
lto = true
codegen-units = 1
strip = true
```

**우선순위**
- 낮음

---

## 우선순위 요약

### 중간
1. scoring 중복 축소
2. provider별 재시도 정책 정리
3. timeout 설정 외부화
4. 통합 테스트 확대
5. fixture 기반 파서 테스트

### 중간-낮음
1. health 응답 provider 상태 세분화
2. session 파일 권한 점검 확장

### 낮음
1. CORS 제한 재검토
2. 문서화 보강
3. release profile 최적화

---

## 메모
- `thiserror`, `tracing` 같은 새 의존성 도입은 가능하지만 현재 우선순위는 아닙니다.
- `Bugs`, `Genie`는 지금 구조를 유지하되, 파서 회귀 테스트 강화가 더 중요합니다.
- `Deezer`는 인증/설정 검증 쪽 안정화가 이미 들어가 있으므로, 다음 개선은 재시도/timeout 정책 쪽이 더 자연스럽습니다.
