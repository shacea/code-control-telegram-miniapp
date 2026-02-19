# Telegram Remote Control

code-control-telegram의 AI 기능(`.` 단축키)을 텔레그램 Bot을 통해 원격으로 사용할 수 있도록 한다.

## 개요

- 텔레그램에서 `/start <path>` 로 세션을 시작
- 텍스트를 보내면 Claude AI에 전달하여 응답을 스트리밍으로 받아온다
- 기존 `~/.code-control-telegram/ai_sessions/` 세션 파일과 호환되어 TUI와 텔레그램 간 세션 공유가 가능

## CLI 명령어

### `code-control-telegram --ccserver <TOKEN> [TOKEN2] ...`
텔레그램 Bot 서버를 시작한다. 토큰을 인자로 직접 전달하며, Long Polling 방식으로 동작한다.
여러 토큰을 전달하면 각각의 봇이 동시에 구동된다.

## 텔레그램 명령어

### `/start <path>`
- 지정된 경로에서 AI 세션을 시작한다
- 기존 세션이 있으면 복원하고 마지막 5개 대화를 표시한다
- 경로가 유효하지 않으면 에러 메시지를 반환한다

### `/clear`
- 현재 세션을 초기화한다
- session_id와 대화 기록을 클리어한다

### 일반 텍스트
- Claude AI에 전달하여 응답을 받는다
- 응답은 스트리밍으로 수신되며, 텔레그램 메시지를 실시간 업데이트한다

## 스트리밍 브릿지

1. 사용자 텍스트 수신 -> "..." placeholder 메시지 전송
2. `tokio::task::spawn_blocking`으로 `claude::execute_command_streaming()` 실행
3. 300ms 간격 polling으로 mpsc channel에서 StreamMessage 수신
4. Text 변경 시 placeholder 메시지를 `edit_message_text`로 업데이트
5. 응답이 4096자 초과 시 여러 메시지로 분할 전송
6. Done/Error 수신 시 최종 메시지 전송 + 세션 저장

## 세션 호환

- TUI에서 만든 세션을 텔레그램에서 이어서 사용 가능
- 텔레그램에서 만든 세션을 TUI에서 이어서 사용 가능
- 동일한 `SessionData` 포맷과 `ai_sessions_dir` 경로 사용
