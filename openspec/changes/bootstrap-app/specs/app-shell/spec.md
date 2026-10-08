## ADDED Requirements

### Requirement: Application launches to a home window
The application SHALL open a single main window titled "MergeIQ" with minimum size 1024x640 when launched with no arguments.

#### Scenario: Launch without arguments
- **WHEN** user starts the app with no CLI arguments
- **THEN** main window opens showing the home view with app name and version

### Requirement: Typed IPC between UI and backend
All frontend-to-backend calls SHALL go through Tauri commands whose TypeScript bindings are generated from Rust types. Hand-written `invoke` calls with string command names SHALL NOT be used in UI code.

#### Scenario: app_info round-trip
- **WHEN** UI calls generated `commands.appInfo()`
- **THEN** it receives `{ name: "MergeIQ", version: <Cargo package version>, platform: "macos" | "windows" | "linux" }`

#### Scenario: Bindings are up to date
- **WHEN** CI runs binding generation and `git diff --exit-code` on the bindings file
- **THEN** no diff exists

### Requirement: Theme follows OS with manual override
The UI SHALL support `light`, `dark`, and `system` theme modes. In `system` mode it SHALL follow OS `prefers-color-scheme` live. All colors SHALL come from CSS variables.

#### Scenario: System mode follows OS change
- **WHEN** theme is `system` and OS switches to dark
- **THEN** UI switches to dark tokens without restart

#### Scenario: Manual override persists
- **WHEN** user selects `dark` and restarts the app
- **THEN** app opens in dark theme

### Requirement: Settings persistence
The backend SHALL persist user settings as JSON at `<OS config dir>/MergeIQ/settings.json`, creating it with defaults when missing. Unknown keys SHALL be preserved; a corrupt file SHALL be backed up to `settings.json.bak` and replaced with defaults.

#### Scenario: Missing settings file
- **WHEN** app starts and no settings file exists
- **THEN** a settings file with defaults is created

#### Scenario: Corrupt settings file
- **WHEN** settings file contains invalid JSON
- **THEN** file is renamed to `settings.json.bak`, defaults are loaded, and a warning is logged

### Requirement: Logging
The backend SHALL write structured logs via `tracing` to `<OS log dir>/MergeIQ/` with daily rotation, keeping at most 7 files. Log level SHALL default to `info` and be overridable with env var `MERGEIQ_LOG`.

#### Scenario: Log file written
- **WHEN** app starts
- **THEN** a log line containing app version is written to today's log file

### Requirement: Continuous integration
CI SHALL run on push and pull request for macOS, Windows, and Linux: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo nextest run`, `pnpm lint`, `pnpm test`, `pnpm tauri build --no-bundle` (or debug build).

#### Scenario: Lint failure blocks CI
- **WHEN** a commit introduces a clippy warning
- **THEN** CI job fails
