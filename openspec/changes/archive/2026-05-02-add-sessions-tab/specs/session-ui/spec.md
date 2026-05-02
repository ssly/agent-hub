## ADDED Requirements

### Requirement: Sessions tab in tab bar
The system SHALL display a third tab labeled "Sessions" in the sidebar tab bar, alongside existing "Skills" and "MCP" tabs.

#### Scenario: Tab bar renders three tabs
- **WHEN** the app loads
- **THEN** the tab bar SHALL show three tabs: "Skills", "MCP", "Sessions"

#### Scenario: Switching to Sessions tab
- **WHEN** user clicks the Sessions tab
- **THEN** the sidebar SHALL show platforms that have session data, and the main view SHALL show the selected platform's session list

### Requirement: Session platform sidebar
When the Sessions tab is active, the sidebar SHALL list agent platforms that have session data, each showing the platform display name and session count. The search input SHALL be hidden in this tab.

#### Scenario: Multiple platforms with sessions
- **WHEN** both Claude Code and Codex CLI have session data
- **THEN** the sidebar SHALL show both platforms with their respective session counts

#### Scenario: No platforms have sessions
- **WHEN** no platforms have any session data
- **THEN** the sidebar SHALL show a localized empty state message

### Requirement: Session list view
The main view SHALL display a list of sessions for the selected platform. Each session card SHALL show: title, project path (relative or truncated), model name, start time (localized date format), token count (if available), and message count (if available). Sessions SHALL be sorted by most recently updated first.

#### Scenario: Sessions with full metadata
- **WHEN** a session has title, project path, model, and token count
- **THEN** the session card SHALL display all available fields

#### Scenario: Session with minimal metadata
- **WHEN** a session only has a title and timestamp (no model or tokens)
- **THEN** the session card SHALL display available fields and omit missing ones without showing empty placeholders

#### Scenario: No platform selected
- **WHEN** the Sessions tab is active but no platform is selected in the sidebar
- **THEN** the main view SHALL show a prompt to select a platform

### Requirement: Session detail modal
Clicking a session card SHALL open a modal displaying the session's conversation messages. Messages SHALL be loaded in pages of 50. User messages SHALL be styled differently from assistant messages.

#### Scenario: Opening session detail
- **WHEN** user clicks a session card
- **THEN** a modal SHALL open showing the first page of messages (up to 50) with user/assistant role distinction

#### Scenario: Loading more messages
- **WHEN** user scrolls to the bottom of the modal and more messages are available
- **THEN** the system SHALL load the next page of messages and append them

#### Scenario: Closing modal
- **WHEN** user clicks outside the modal or presses Escape
- **THEN** the modal SHALL close and return to the session list

### Requirement: Locale support for Sessions tab
All user-facing text in the Sessions tab SHALL support both English and Chinese locales via the existing i18n system.

#### Scenario: English locale
- **WHEN** the app language is set to English
- **THEN** all Sessions tab text (tab label, column headers, empty states, loading states) SHALL display in English

#### Scenario: Chinese locale
- **WHEN** the app language is set to Chinese
- **THEN** all Sessions tab text SHALL display in Chinese
