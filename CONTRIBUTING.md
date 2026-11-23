# Contributing

Thanks for your interest in ngodik. The project is in alpha and currently targets Windows desktop.

## Before opening an issue

- Search existing issues for the same bug or request.
- Include the app version, Windows version, and a short reproduction path.
- Remove provider API keys, personal project data, database files, and private endpoints from screenshots and logs.
- Use a minimal sample idea that does not contain confidential information.

## Pull requests

1. Fork the repository and create a focused branch.
2. Explain the problem and intended behavior in the pull request.
3. Include steps to reproduce and verify user-facing changes.
4. Run the checks below.

<pre><code>npm ci
npm run check
cargo test --locked --manifest-path src-tauri/Cargo.toml</code></pre>

Do not commit node_modules, dist, target directories, local databases, generated blueprint ZIP files, secrets, or API credentials. Add database changes as a new migration. Keep the project specification JSON Schema and its validation behavior aligned.
