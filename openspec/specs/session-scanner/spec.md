# session-scanner Specification

## Purpose
TBD - created by archiving change add-sessions-tab. Update Purpose after archive.
## Requirements
### Requirement: Platform session discovery
The system SHALL discover which agent platforms have session data available on the local machine. For each platform, it SHALL report the platform id, display name, and number of sessions found.

#### Scenario: Claude Code sessions discovered
- **WHEN** the system scans for session platforms
- **THEN** it SHALL check `~/.claude/projects/` directory for JSONL files and report platform id "claude-code" with the total count of `.jsonl` files across all project subdirectories

#### Scenario: Codex CLI sessions discovered
- **WHEN** the system scans for session platforms
- **THEN** it SHALL check `~/.codex/state_5.sqlite` exists and query the `threads` table count, reporting platform id "codex-cli" with the thread count

#### Scenario: Platform with no sessions
- **WHEN** a platform has no session data (e.g. directory does not exist, SQLite file missing)
- **THEN** that platform SHALL be omitted from the results

### Requirement: Claude Code session listing
The system SHALL list all Claude Code sessions by scanning `~/.claude/projects/<encoded-path>/*.jsonl` files. Each session entry SHALL include: session id (filename without extension), title (from `custom-title` line or first user message truncated to 80 chars), project path (decoded from directory name), model (from first assistant message), started_at (from first line timestamp), updated_at (from file modification time), and message_count.

#### Scenario: Session with custom title
- **WHEN** a JSONL file contains a line with `type: "custom-title"`
- **THEN** the session title SHALL use the `customTitle` field value

#### Scenario: Session without custom title
- **WHEN** a JSONL file has no `custom-title` line but has at least one `type: "user"` line
- **THEN** the session title SHALL be the first user message `message.content` string truncated to 80 characters

#### Scenario: Project path decoding
- **WHEN** scanning directory names under `~/.claude/projects/`
- **THEN** each directory name (e.g. `-Users-liuyang-Documents-code`) SHALL be decoded to a file path by replacing dashes with path separators (e.g. `/Users/liuyang/Documents/code`)

### Requirement: Codex CLI session listing
The system SHALL list all Codex CLI sessions by querying `~/.codex/state_5.sqlite` `threads` table. Each session entry SHALL include: session id, title, project path (from `cwd` column), model, tokens_used, started_at (from `created_at` column), updated_at (from `updated_at` column), and first_user_message.

#### Scenario: Successful SQLite query
- **WHEN** the system queries the threads table
- **THEN** it SHALL return all threads ordered by `updated_at` DESC with columns: id, title, cwd, model, tokens_used, created_at, updated_at, first_user_message

#### Scenario: SQLite database locked
- **WHEN** the SQLite database is locked by Codex CLI
- **THEN** the system SHALL open the database in READONLY mode and retry once with a 100ms delay before returning an error

### Requirement: Session message pagination
The system SHALL support paginated reading of session messages. For Claude Code, it SHALL read the JSONL file line by line, filtering for `type: "user"` and `type: "assistant"` lines. For Codex CLI, it SHALL read the rollout JSONL file referenced by `threads.rollout_path`, filtering for `event_msg` with `payload.type: "user_message"` and `response_item` with `payload.type: "message"` lines.

#### Scenario: Paginated message fetch
- **WHEN** the frontend requests messages with offset=0 and limit=50
- **THEN** the system SHALL return the first 50 user/assistant message pairs from the session, each with role, content (text extracted), and timestamp

#### Scenario: Large file streaming
- **WHEN** reading a JSONL file larger than 10MB
- **THEN** the system SHALL use buffered line-by-line reading and SHALL NOT load the entire file into memory

### Requirement: Shared session data models
All session data SHALL use a unified `SessionSummary` struct with fields: id (String), title (String), project_path (String), model (Option<String>), started_at (i64 epoch ms), updated_at (i64 epoch ms), message_count (Option<u32>), tokens_used (Option<u64>), platform_id (String).

#### Scenario: Cross-platform unified response
- **WHEN** listing sessions from either Claude Code or Codex CLI
- **THEN** the response SHALL use the same `SessionSummary` structure regardless of platform

