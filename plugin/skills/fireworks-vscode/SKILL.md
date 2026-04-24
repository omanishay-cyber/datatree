---
name: fireworks-vscode
version: 1.0.0
author: mneme
description: Use when configuring VS Code for Flutter, Electron, React, or TypeScript projects. Covers launch configs, debugging profiles, tasks, snippets, extensions, settings optimization, multi-root workspaces, and keybindings. Triggers on .vscode, launch.json, settings.json, extensions.json, tasks.json, or VS Code workflow questions.
triggers:
  - vscode
  - vs code
  - launch.json
  - settings.json
  - extensions.json
  - tasks.json
  - debugging
  - breakpoint
  - snippet
  - workspace
---

# Fireworks VS Code v1.0 — Master Skill Hub

> Enterprise-grade VS Code productivity for Flutter, Electron, React, and TypeScript.
> This hub provides setup protocols, decision trees, verification gates, and cross-references.
> All detailed JSON configs and code examples live in `references/` files.
> **Compound skill** — auto-chains to `fireworks-flutter` for Flutter configs, `fireworks-debug` for debugging, `fireworks-test` for test tasks.

---

## 1. Development Protocol

Every VS Code setup task follows this pipeline:

1. **Identify Stack** — Determine project type (Flutter, Electron, React, TypeScript, hybrid)
2. **Extensions** — Install required + recommended extensions for the stack
3. **Settings** — Configure workspace settings.json optimized for the stack
4. **Launch Configs** — Set up all debug/run launch configurations
5. **Tasks** — Define build, test, lint, and deploy tasks
6. **Snippets** — Install or create project-specific code snippets
7. **Keybindings** — Configure productivity shortcuts for the stack
8. **Verify** — Test every launch config, run every task, confirm extensions active

---

## 2. Extension Recommendations by Stack

### Flutter

| Extension | ID | Purpose |
|---|---|---|
| Dart | `dart-code.dart-code` | Dart language support, analysis, formatting |
| Flutter | `dart-code.flutter` | Flutter commands, hot reload, device management |
| Flutter Riverpod Snippets | `robert-brunhage.flutter-riverpod-snippets` | Riverpod codegen snippets |
| Awesome Flutter Snippets | `nash.awesome-flutter-snippets` | Widget and pattern snippets |
| Flutter Widget Snippets | `alexisvt.flutter-snippets` | Widget boilerplate snippets |
| Error Lens | `usernamehw.errorlens` | Inline error/warning display |
| Dart Data Class Generator | `bendixma.dart-data-class-generator` | Generate fromJson, toJson, copyWith |

### Electron / React / TypeScript

| Extension | ID | Purpose |
|---|---|---|
| ESLint | `dbaeumer.vscode-eslint` | Linting for JS/TS |
| Prettier | `esbenp.prettier-vscode` | Code formatting |
| Tailwind CSS IntelliSense | `bradlc.vscode-tailwindcss` | Class autocomplete, hover preview |
| Error Lens | `usernamehw.errorlens` | Inline diagnostics |
| GitLens | `eamodio.gitlens` | Git blame, history, compare |
| Thunder Client | `rangav.vscode-thunder-client` | REST API testing |
| Auto Rename Tag | `formulahendry.auto-rename-tag` | Sync HTML/JSX tag renames |
| ES7+ Snippets | `dsznajder.es7-react-js-snippets` | React/Redux snippets |

### Universal (All Projects)

| Extension | ID | Purpose |
|---|---|---|
| GitHub Copilot | `github.copilot` | AI code completion |
| Material Icon Theme | `pkief.material-icon-theme` | File/folder icons |
| Project Manager | `alefragnani.project-manager` | Quick project switching |
| Todo Tree | `gruntfuggly.todo-tree` | TODO/FIXME/HACK aggregation |
| Better Comments | `aaron-bond.better-comments` | Color-coded comment annotations |
| Path Intellisense | `christian-kohler.path-intellisense` | File path autocomplete |
| EditorConfig | `editorconfig.editorconfig` | Cross-editor formatting rules |

See `references/flutter-vscode-config.md` for Flutter extension config.
See `references/electron-vscode-config.md` for Electron/React extension config.

---

## 3. Launch Configurations Decision Tree

### Flutter

| Scenario | Config Name | Key Properties |
|---|---|---|
| Debug on Chrome | `Flutter Web (Chrome)` | `"deviceId": "chrome"`, `--web-port=5000` |
| Debug on iOS Simulator | `Flutter iOS` | `"deviceId": "iPhone 16 Pro"` |
| Debug on Android Emulator | `Flutter Android` | `"deviceId": "emulator-5554"` |
| Debug on macOS desktop | `Flutter macOS` | `"deviceId": "macos"` |
| Debug on Windows desktop | `Flutter Windows` | `"deviceId": "windows"` |
| Attach to running app | `Flutter Attach` | `"request": "attach"`, requires `--observatory-port` |
| Profile mode | `Flutter Profile` | `"flutterMode": "profile"` |
| Release mode test | `Flutter Release` | `"flutterMode": "release"` |

### Electron

| Scenario | Config Name | Key Properties |
|---|---|---|
| Main process only | `Electron Main` | `"runtimeExecutable": "${workspaceFolder}/node_modules/.bin/electron"` |
| Renderer process only | `Electron Renderer` | Attach to Chrome DevTools port |
| Combined (main + renderer) | `Electron Full` | Compound config launching both |
| Main with args | `Electron Dev` | `"args": [".", "--inspect=5858"]` |

### React / Vite

| Scenario | Config Name | Key Properties |
|---|---|---|
| Chrome debug | `React Chrome` | `"url": "http://localhost:5173"`, `"webRoot": "${workspaceFolder}/src"` |
| Edge debug | `React Edge` | `"type": "msedge"`, same webRoot |
| Attach to running | `React Attach` | `"request": "attach"`, `"port": 9222` |

**Rule: Every project MUST have at least one working launch.json config before development begins.**

See `references/flutter-vscode-config.md` for full Flutter launch.json.
See `references/electron-vscode-config.md` for full Electron launch.json.
See `references/debugging-advanced.md` for compound configs and advanced debugging.

---

## 4. Settings Optimization

### Workspace vs User Settings

| Setting Level | When to Use | File Location |
|---|---|---|
| User (global) | Editor preferences, theme, font | `%APPDATA%/Code/User/settings.json` |
| Workspace | Project-specific lint, format, paths | `.vscode/settings.json` |
| Folder (multi-root) | Per-folder overrides | `.code-workspace` file |

### Quick-Reference: Critical Settings by Stack

| Stack | Setting | Value | Why |
|---|---|---|---|
| Flutter | `dart.flutterSdkPath` | SDK path | Ensures correct SDK |
| Flutter | `dart.lineLength` | `80` | Dart standard |
| Flutter | `[dart].editor.formatOnSave` | `true` | Auto-format on save |
| Flutter | `dart.previewFlutterUiGuides` | `true` | Widget tree visualization |
| React/TS | `editor.defaultFormatter` | `esbenp.prettier-vscode` | Consistent formatting |
| React/TS | `editor.formatOnSave` | `true` | Auto-format |
| React/TS | `typescript.preferences.importModuleSpecifier` | `"non-relative"` | Clean imports |
| React/TS | `tailwindCSS.experimental.classRegex` | `["cn\\(([^)]*)\\)"]` | cn() utility support |
| Electron | `debug.javascript.autoAttachFilter` | `"smart"` | Auto-attach debugger |
| All | `editor.bracketPairColorization.enabled` | `true` | Visual bracket matching |
| All | `editor.guides.bracketPairs` | `"active"` | Active bracket guides |
| All | `files.autoSave` | `"onFocusChange"` | Save on tab switch |
| All | `editor.minimap.enabled` | `false` | Reclaim screen space |

See `references/flutter-vscode-config.md` for complete Flutter settings.json.
See `references/electron-vscode-config.md` for complete Electron/React settings.json.

---

---

## Full Reference

For complete patterns, examples, and advanced usage, see [`references/full-guide.md`](./references/full-guide.md).
Read that file when you need deeper context than the summary above.
