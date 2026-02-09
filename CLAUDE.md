# madang-compiler 개발 지침

## 그라운드 룰

1. **커먼마크 스펙을 구현한다.**
2. **커먼마크 스펙에 적혀있는 Example을 빠짐없이 TC로 만든다.**
3. **Example TC를 통과하는데 필요한 코드 외에는 작성하지 않는다.**

## 작업 목적

이 프로젝트는 단순히 파서를 만드는 것이 아니라, **학습**이 목적이다:
- 러스트 지식
- 파서 구현 지식
- CommonMark 명세에 대한 이해

따라서 빠른 완성보다 각 단계를 깊이 이해하는 것이 중요하다.

## 작업 방식

1. **한 단계씩 진행**: 코드를 한번에 많이 작성하지 않고, 아주 작은 단위로 나눠서 진행
2. **단계 시작 전 설명**: 각 단계를 시작하기 전에 다음을 설명
   - 무엇을 할 것인가
   - 왜 이 단계를 다음으로 선정했는가
3. **확인 후 진행**: 설명 후 사용자의 질문을 받고, 확인 후에 코드 작성
4. **다음 단계 진행 시 AskUserQuestion 도구 사용**: 다음 단계로 넘어갈 때 반드시 AskUserQuestion 도구로 확인
5. **TDD 방식 준수**: 구현 과정은 철저히 TDD 방식을 따른다
   - 테스트 먼저 작성
   - 실패 확인
   - 최소한의 구현으로 테스트 통과
   - **항상 rstest를 사용한 parametrized 테스트로 작성**
     ```rust
     #[rstest]
     #[case("input1", expected1)]
     #[case("input2", expected2)]
     fn test_something(#[case] input: &str, #[case] expected: Type) { ... }
     ```
   - 개별 `#[test]` 함수 대신 `#[rstest]` + `#[case]`로 케이스 통합
   - **CommonMark 명세의 Example은 반드시 테스트 케이스로 구현**
     - 각 케이스에 `// Example NNN: 설명` 주석 추가
     - 예: `#[case("> # Foo", Some((1, "Foo")), false)]  // Example 228`
     - 명세의 모든 관련 Example을 누락 없이 포함
6. **새 스펙 구현 전 명세 확인**: 새로운 마크다운 요소를 구현하기 전에
   - CommonMark 명세(https://spec.commonmark.org/)에서 해당 요소 정의 확인
   - 엣지 케이스와 공식 규칙을 먼저 파악
   - 명세를 바탕으로 테스트 케이스 설계
7. **학습 종료 시 요약 저장**: "학습 종료" 선택 시 다음을 수행
   - 현재까지 학습한 내용을 `progress/PROGRESS-{시퀀스}.md`에 저장 (예: `PROGRESS-001.md`, `PROGRESS-002.md`)
   - 기존 파일을 덮어쓰지 않고 새 시퀀스 번호로 생성
   - 다음 학습 시작 시 가장 최신 시퀀스 파일을 읽고 시작 지점을 상기

## 코드 스타일

1. **`use` 문은 파일 상단에**: 함수나 impl 블록 내부가 아닌, 파일 최상단에 모아서 선언
2. **함수/메서드 선언 위치 고려**: 새 함수를 추가할 때 OOP 원칙에 따라 적절한 위치를 고민
   - 특정 타입을 변환하는 함수 → 해당 타입의 메서드로 구현
   - 예: `ListMarker` → `ListType` 변환은 `ListMarker::to_list_type()`
   - 예: `Node` 생성 로직은 `Node::build_list()`
3. **불변 스타일 선호**: `let mut` 대신 메서드 체이닝이나 새 인스턴스 반환 방식 사용
   - 예: `start.with_content_from(line)` 패턴

## 버저닝

- 형식: `YYYY.M.D` (연도.월.일)
- 예: `2025.1.14`

## 프로젝트 목표

- CommonMark → GFM → OFM 순서로 확장하는 마크다운 파서
- 러스트로 구현
