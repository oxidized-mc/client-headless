# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| `main` (latest) | ✅ |
| older releases | ❌ |

## Reporting a Vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

Report security issues by using
[GitHub's private vulnerability reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing/privately-reporting-a-security-vulnerability)
on this repository.

Include:
- A description of the vulnerability and its impact
- Steps to reproduce (proof-of-concept if possible)
- Affected versions/commits

We will acknowledge your report within **72 hours** and aim to release a fix
within **7 days** for critical issues.

## Scope

This project is a **client-side** library/framework. Relevant vulnerability classes:

- Credential exposure (Mojang auth tokens, session secrets)
- Remote code execution via malformed server packets
- Denial of service via malicious server responses
- Memory safety issues in protocol parsing
- Path traversal in file handling

Out of scope: server-side rendering bugs, vanilla client exploits.
