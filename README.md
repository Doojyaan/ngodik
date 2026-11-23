# ngodik

> A local-first Windows desktop app for turning an idea into a structured project blueprint.

![ngodik logo](assets/branding/ngodik-horizontal.svg)

**Status: Alpha** · [User guide in Indonesian](docs/USER_GUIDE_ID.md) · [Development guide](docs/DEVELOPMENT.md) · [Architecture decisions](ARCHITECTURE_DECISIONS.md) · [Brand assets](docs/BRANDING.md)

ngodik turns an initial idea into structured, editable project documents. It uses a React and TypeScript interface, a Rust and Tauri desktop backend, and local SQLite storage. AI requests go only to the provider endpoint configured by the user.

## Features

- Project workspace with autosave and idea revision checkpoints.
- OpenAI Chat Completions-compatible provider profiles and model selection per project.
- API keys managed through Windows Credential Manager.
- Four-stage blueprint generation: product and requirements, system analysis, UX, and solution architecture.
- Draft resume after interruption, structural checks, semantic review, revision proposals, and version recovery.
- ZIP export with blueprint documents and coding-agent handoff instructions.

## Status and scope

The app is in alpha and targets Windows. Review AI-generated assumptions and QA findings before implementation. Remote provider endpoints must use HTTPS. Localhost/loopback endpoints may use HTTP. Hosted accounts, team collaboration, cross-device sync, automatic updates, installer packaging, and code signing are not configured.

## Quick start

### Requirements

Node.js and npm, Rust stable with the MSVC target, Microsoft C++ Build Tools with Windows SDK, and WebView2 Runtime.

### Run from source

<pre><code>git clone https://github.com/Doojyaan/ngodik.git
cd ngodik
npm ci
npm run tauri dev</code></pre>

### Checks and Windows build

<pre><code>npm run check
cargo test --locked --manifest-path src-tauri/Cargo.toml
npm run tauri build -- --no-bundle</code></pre>

The build command creates an unpackaged Windows executable.

## Technology

| Area | Stack |
| --- | --- |
| Desktop shell | Tauri 2 |
| Frontend | React 19, TypeScript 7, Vite 8 |
| Motion | GSAP 3 and @gsap/react |
| Backend | Rust 2024 |
| Local data | SQLite through rusqlite |
| Model API | OpenAI Chat Completions-compatible HTTP |
| Secret storage | Windows Credential Manager |
| Specification validation | JSON Schema |

See [third-party dependency notices](THIRD_PARTY_NOTICES.md) for direct dependency versions and declared licenses.

## Repository structure

<pre><code>src/                  React and TypeScript UI
src-tauri/src/         Rust commands and application services
src-tauri/migrations/  SQLite migrations
src-tauri/schemas/     Canonical project specification
src-tauri/icons/       Windows application icon
assets/branding/       Logo masters, exports, and brand usage notes
docs/                  User and developer documentation</code></pre>

See the [development guide](docs/DEVELOPMENT.md) for module details and [brand guide](docs/BRANDING.md) for logo usage.

## Data and security

Project database and application-managed files live under the Windows local app-data directory, normally %LOCALAPPDATA%\id.ngodik.desktop\. The SQLite database is ngodik.sqlite3. Provider API keys are stored separately through Windows Credential Manager and are not part of the project database. Do not share keys in issues, logs, screenshots, or generated blueprint packages.

Read [SECURITY.md](SECURITY.md) before reporting a security issue. User walkthrough: [docs/USER_GUIDE_ID.md](docs/USER_GUIDE_ID.md).

## License

ngodik is released under the [Apache License 2.0](LICENSE). Third-party packages remain subject to their own terms; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) before opening an issue or pull request.
