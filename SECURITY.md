# Security Policy

ngodik is an alpha Windows desktop application. Please avoid sharing real API keys, private project documents, local database files, or sensitive provider URLs in bug reports.

## Reporting

For a suspected vulnerability, contact the maintainer through the [Doojyaan GitHub profile](https://github.com/Doojyaan) and request a private reporting channel. Do not publish a working exploit or credentials in a public issue.

## Security-relevant design

- Provider keys are stored through Windows Credential Manager.
- Project metadata and generated artifacts are stored locally in the app data directory.
- Remote provider calls should use HTTPS.
- Review exported documents before sharing them. ngodik includes checks for known credential-like text, but no automated check can guarantee that every secret has been removed.
