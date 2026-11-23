# Development Guide

## Requirements

- Windows with WebView2 Runtime
- Node.js and npm
- Rust stable MSVC toolchain
- Microsoft C++ Build Tools and Windows SDK

The repository targets Windows because provider secrets use Windows Credential Manager. Cross-platform support is not currently promised.

## Commands

<pre><code>npm ci
npm run tauri dev
npm run check
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml
npm run tauri build -- --no-bundle</code></pre>

- npm ci installs the exact frontend dependency tree from package-lock.json.
- npm run tauri dev starts Vite and the Tauri desktop shell.
- npm run check type-checks the TypeScript interface.
- npm run build type-checks and creates the Vite production bundle.
- cargo test runs Rust unit and integration tests for the local backend.
- npm run tauri build -- --no-bundle produces an unpackaged Windows build.

## Code map

| Path | Responsibility |
| --- | --- |
| src/main.tsx | Main navigation and project list |
| src/workspace.tsx | Project workspace and panel selection |
| src/model-settings.tsx | Provider profiles, model selection, and credentials UI |
| src/draft-panel.tsx | Draft stages, progress, artifacts, and resume controls |
| src/draft-review.tsx | Findings, proposals, and blueprint version history |
| src/export-panel.tsx | ZIP export and handoff prompt |
| src-tauri/src/main.rs | Tauri setup, local database initialization, command registration |
| src-tauri/src/projects.rs | Project operations and local persistence |
| src-tauri/src/providers.rs | Provider configuration and model HTTP requests |
| src-tauri/src/providers/credentials.rs | Windows Credential Manager integration |
| src-tauri/src/drafts.rs | Draft orchestration and stage execution |
| src-tauri/src/drafts/ | Draft storage, reviews, revisions, versions, and export |
| src-tauri/migrations/ | Ordered SQLite schema migrations |
| src-tauri/schemas/ | Canonical JSON Schema for generated project specifications |
| assets/branding/ | Logo masters, variants, and usage guide |

## Application flow

The React frontend calls Rust commands through Tauri invoke. Rust coordinates local project storage, provider calls, staged generation, review and revision workflows, snapshots, and ZIP packaging. SQLite migrations evolve the local database. The canonical project specification is in src-tauri/schemas/project-specification.v1.json.

The AI provider is user-configured. The backend retrieves credentials from Windows Credential Manager when a request needs them; credentials are not part of the SQLite project profile. Remote endpoint URLs use HTTPS, with loopback HTTP allowed for a locally hosted model.

## Database and app data

Tauri's app-local data directory stores ngodik.sqlite3 and managed project files. Database schema changes belong in a new ordered migration under src-tauri/migrations. Do not rewrite a migration that may already have been applied to a user's database.

## Tests and pull requests

Run the frontend type check and Rust tests before submitting a change. Keep pull requests focused, describe user-visible changes, and include reproduction steps for bug fixes. Do not commit API keys, generated blueprint archives, local database files, build artifacts, or Windows credentials.
