## ADDED Requirements

### Requirement: Universal core field extraction

When syncing an MCP server between platforms, the system SHALL extract only the universal core fields from the source config: `command`, `args`, and `env` (when non-empty). All other fields SHALL be discarded from the sync payload.

#### Scenario: Claude Code to Gemini strips platform-specific fields

- **WHEN** a server config from Claude Code contains `type: "stdio"`, `command: "npx"`, `args: ["-y", "@upstash/context7-mcp", "--api-key", "KEY"]`, and `env: {}`
- **AND** it is synced to Gemini
- **THEN** the written Gemini entry contains only `command` and `args`
- **AND** `type` and empty `env` are not present

#### Scenario: Claude Code to Codex strips platform-specific fields

- **WHEN** a server config from Claude Code contains `type: "stdio"`, `command`, `args`, and `env: {}`
- **AND** it is synced to Codex
- **THEN** the written Codex entry contains only `command` and `args`
- **AND** no unrecognized fields are written (preventing Codex's deny_unknown_fields from failing)

#### Scenario: Codex to Gemini strips platform-specific fields

- **WHEN** a server config from Codex contains `command`, `args`, `env`, and `startup_timeout_sec: 20`
- **AND** it is synced to Gemini
- **THEN** the written Gemini entry contains only `command`, `args`, and `env` (if non-empty)
- **AND** `startup_timeout_sec` is not present

#### Scenario: api-key in args is preserved

- **WHEN** a server's `args` array contains `["--api-key", "ctx7sk-..."]`
- **AND** it is synced to any target platform
- **THEN** the synced entry's `args` contains the identical api-key value

#### Scenario: api-key in env is preserved

- **WHEN** a server's `env` contains `{"API_KEY": "secret"}`
- **AND** it is synced to any target platform
- **THEN** the synced entry's `env` contains `{"API_KEY": "secret"}`

#### Scenario: Empty env is dropped

- **WHEN** the source config has `env: {}`
- **THEN** the synced entry does not include an `env` field

### Requirement: Preserve target platform-specific fields

When the target already has the server being synced, the system SHALL NOT delete or modify any fields outside the core set (`command`, `args`, `env`). Platform-specific fields in the target (such as Codex's `startup_timeout_sec` or Gemini's `timeout`) SHALL remain intact.

#### Scenario: Codex startup_timeout_sec preserved after sync

- **WHEN** Codex already has a server with `command`, `args`, and `startup_timeout_sec: 20`
- **AND** the server is synced from Claude Code with updated `args`
- **THEN** the Codex entry has the updated `args`
- **AND** `startup_timeout_sec: 20` remains unchanged

#### Scenario: Gemini timeout preserved after sync

- **WHEN** Gemini already has a server with `command`, `args`, and `timeout: 30000`
- **AND** the server is synced from Codex with updated `args`
- **THEN** the Gemini entry has the updated `args`
- **AND** `timeout: 30000` remains unchanged

### Requirement: Field-level minimal update for an existing server

When the target config already contains the server being synced, the system SHALL rewrite only the core sub-properties whose values differ, leaving unchanged sub-properties and their surrounding formatting intact.

#### Scenario: Only changed core fields are rewritten

- **WHEN** the target already has the server and only `args` differs from the source's core
- **THEN** the resulting file diff touches only the `args` value
- **AND** `command` and other fields remain byte-identical

#### Scenario: No-op sync produces an empty diff

- **WHEN** the target already has the server and its core fields equal the source's core fields
- **THEN** the sync produces no change to the target file

### Requirement: Preview matches the applied result

The sync preview SHALL display the diff computed from the same core-extracted config that the apply step writes, so the previewed diff is identical to the diff produced when the sync is applied.

#### Scenario: Preview diff equals applied diff

- **WHEN** a sync is previewed and then applied for the same source, target, and server
- **THEN** the file content shown as "after" in the preview equals the file content written on apply
