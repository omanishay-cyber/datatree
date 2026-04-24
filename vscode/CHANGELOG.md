# Changelog

All notable changes to the Mneme VS Code extension will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-04-24

- Initial release.
- Auto-registers the mneme MCP server with VS Code on activation.
- Adds 6 commands: Build, Doctor, Recall, Open Vision, Start/Stop daemon.
- Status bar item shows daemon health (polled every 30s).
- Honors `mneme.binaryPath` and `mneme.autoRegisterMCP` settings.
