# fireworks-vscode — Full Guide

> This file contains the full reference material previously embedded in SKILL.md.
> Loaded on demand when the skill needs deeper context.

## 5. Tasks Configuration

### Flutter Tasks

| Task | Command | Group | Shortcut |
|---|---|---|---|
| Analyze | `flutter analyze` | test | Ctrl+Shift+B |
| Test All | `flutter test` | test | -- |
| Test File | `flutter test ${file}` | test | -- |
| Build Runner | `dart run build_runner watch --delete-conflicting-outputs` | build | -- |
| Clean | `flutter clean && flutter pub get` | none | -- |
| Build APK | `flutter build apk --release` | build | -- |
| Build Web | `flutter build web --release` | build | -- |

### Electron / React / TypeScript Tasks

| Task | Command | Group | Shortcut |
|---|---|---|---|
| Type Check | `tsc --noEmit` | test | Ctrl+Shift+B |
| Vite Dev | `npx vite` | none | -- |
| Vite Build | `npx vite build` | build | -- |
| ESLint | `npx eslint src/ --ext .ts,.tsx` | test | -- |
| Electron Build | `npx electron-builder --config` | build | -- |
| Test | `npx vitest run` | test | -- |
| Test Watch | `npx vitest` | test | -- |

**Rule: `tsc --noEmit` MUST be a task in every TypeScript project.**

See `references/flutter-vscode-config.md` for full tasks.json examples.
See `references/electron-vscode-config.md` for full tasks.json examples.

---

## 6. Debugging Protocol

### Breakpoint Types

| Type | When to Use | How |
|---|---|---|
| Line breakpoint | Standard pause at line | Click gutter |
| Conditional breakpoint | Pause only when condition is true | Right-click gutter > Conditional |
| Logpoint | Log without pausing | Right-click gutter > Logpoint, use `{expression}` |
| Exception breakpoint | Pause on throw | Debug panel > Breakpoints > check exceptions |
| Hit count breakpoint | Pause after N hits | Right-click > Hit Count |
| Function breakpoint | Pause on function entry | Debug panel > + Function Breakpoint |

### Debug Panel Navigation

| Action | Shortcut | Purpose |
|---|---|---|
| Continue | `F5` | Resume execution |
| Step Over | `F10` | Execute current line, skip into functions |
| Step Into | `F11` | Enter function call |
| Step Out | `Shift+F11` | Return from current function |
| Restart | `Ctrl+Shift+F5` | Restart debug session |
| Stop | `Shift+F5` | End debug session |
| Toggle Breakpoint | `F9` | Add/remove breakpoint at cursor |

### Call Stack Navigation

1. Pause at breakpoint or exception
2. Use CALL STACK panel to see the full trace
3. Click any frame to jump to that source location
4. Use VARIABLES panel to inspect locals at each frame
5. Use WATCH panel to track expressions across frames
6. Use DEBUG CONSOLE to evaluate expressions in current scope

See `references/debugging-advanced.md` for compound configs, env vars, remote debugging.

---

## 7. Snippets

### Custom Snippet Quick-Reference

| Prefix | Stack | What It Generates |
|---|---|---|
| `rprov` | Flutter | Riverpod `@riverpod` provider with codegen |
| `rnotif` | Flutter | Riverpod `@riverpod` class notifier |
| `blocevent` | Flutter | BLoC event class with Freezed |
| `blocstate` | Flutter | BLoC state class with Freezed |
| `goroute` | Flutter | GoRouter route definition |
| `frzmodel` | Flutter | Freezed data model |
| `rfc` | React | React functional component with TypeScript |
| `rhook` | React | Custom React hook |
| `zustand` | React | Zustand store definition |
| `ipcmain` | Electron | IPC main handler with typed channel |
| `ipcrender` | Electron | IPC renderer invoke with typed channel |
| `ipcbridge` | Electron | Preload bridge expose |

**Rule: Never type boilerplate by hand. If a pattern repeats 3+ times, create a snippet.**

See `references/snippets-library.md` for complete snippet definitions.

---

## 8. Multi-Root Workspaces

### When to Use

| Scenario | Single-Root | Multi-Root |
|---|---|---|
| Single app | Yes | No |
| Monorepo (frontend + backend) | No | Yes |
| Electron (main + renderer separate) | No | Yes |
| Flutter + companion API | No | Yes |
| Multiple related packages | No | Yes |

### .code-workspace File Structure

```json
{
  "folders": [
    { "name": "Frontend", "path": "./packages/renderer" },
    { "name": "Backend", "path": "./packages/main" },
    { "name": "Shared", "path": "./packages/shared" }
  ],
  "settings": {
    "editor.formatOnSave": true
  },
  "extensions": {
    "recommendations": ["dbaeumer.vscode-eslint"]
  },
  "launch": {
    "compounds": []
  }
}
```

**Per-folder settings** override workspace settings for that folder only. Use for different formatters, linters, or SDK paths per package.

---

## 9. Keybindings

### Productivity Shortcuts (Defaults + Recommended Custom)

| Action | Default | Custom Recommendation |
|---|---|---|
| Quick Open file | `Ctrl+P` | -- |
| Command Palette | `Ctrl+Shift+P` | -- |
| Toggle Terminal | `` Ctrl+` `` | -- |
| Go to Definition | `F12` | -- |
| Peek Definition | `Alt+F12` | -- |
| Find All References | `Shift+F12` | -- |
| Rename Symbol | `F2` | -- |
| Toggle Sidebar | `Ctrl+B` | -- |
| Split Editor | `Ctrl+\` | -- |
| Focus Terminal | -- | `Ctrl+Shift+T` |
| Run Build Task | `Ctrl+Shift+B` | -- |
| Run Task | -- | `Ctrl+Shift+R` (custom) |
| Flutter Hot Reload | -- | `Ctrl+Shift+F5` (custom) |
| Flutter Hot Restart | -- | `Ctrl+Shift+F6` (custom) |
| Git: Stage File | -- | `Ctrl+Shift+S` (custom) |
| Toggle Zen Mode | `Ctrl+K Z` | -- |
| Multi-cursor | `Ctrl+Alt+Down` | -- |
| Select All Occurrences | `Ctrl+Shift+L` | -- |

### Custom Keybinding Example (keybindings.json)

```json
[
  {
    "key": "ctrl+shift+r",
    "command": "workbench.action.tasks.runTask"
  },
  {
    "key": "ctrl+shift+f5",
    "command": "flutter.hotReload",
    "when": "dart.flutterProjectLoaded"
  }
]
```

---

## 10. Verification Gates

Before declaring ANY VS Code setup complete, ALL must pass:

- [ ] All required extensions installed and active (show Extensions panel)
- [ ] `settings.json` present in `.vscode/` with stack-appropriate config
- [ ] `launch.json` has at least one working debug config (launch it, hit a breakpoint)
- [ ] `tasks.json` has build/test tasks that run cleanly (execute each one)
- [ ] `extensions.json` has `recommendations` array for team consistency
- [ ] Formatting on save works (edit a file, save, confirm format applied)
- [ ] Linting shows errors inline (introduce a deliberate error, confirm red squiggly)
- [ ] Snippets work (type prefix, confirm expansion in correct language)
- [ ] Keybindings respond correctly (test each custom binding)
- [ ] Multi-root workspace loads all folders (if applicable)

---

## 11. Wrong vs Right Patterns

| Context | WRONG | RIGHT |
|---|---|---|
| Settings location | Putting project settings in User settings | Use `.vscode/settings.json` for workspace |
| Formatter | Multiple formatters fighting | Set `editor.defaultFormatter` per language |
| Launch config | Hardcoded absolute paths | Use `${workspaceFolder}`, `${file}` variables |
| Extensions | Installing globally, not sharing | Use `extensions.json` recommendations |
| Tasks | Running commands manually in terminal | Define in `tasks.json` for one-click execution |
| Debugging | Using `console.log` / `print()` everywhere | Use breakpoints, logpoints, watch expressions |
| Snippets | Copy-pasting boilerplate from docs | Create project snippets, share via `.vscode/` |
| Workspace | Opening each folder separately | Use `.code-workspace` for multi-root |
| Git | Committing `.vscode/settings.json` with local paths | Use `${workspaceFolder}` vars, gitignore local overrides |
| TypeScript | No type checking task | Add `tsc --noEmit` as default build task |

---

## 12. Compound Skill Chaining

This skill auto-chains to other fireworks skills based on context:

| When You're Doing | Chain To | Why |
|---|---|---|
| Flutter VS Code setup | `fireworks-flutter` | Flutter-specific architecture, testing, packages |
| Debugging any stack | `fireworks-debug` | Scientific 10-step debug protocol |
| Setting up test tasks | `fireworks-test` | TDD methodology, test runners, coverage |
| Electron debugging | `fireworks-debug` + Electron patterns | Main/renderer process debugging |
| Performance profiling config | `fireworks-performance` | Profiling launch configs, DevTools |
| Security scanning tasks | `fireworks-security` | Scanner scripts as VS Code tasks |
| Code review setup | `fireworks-review` | Linter configs, review checklists |

**Chaining is NOT optional.** If a VS Code task involves Flutter project setup, you MUST also load `fireworks-flutter`. If it involves debugging configuration, you MUST also load `fireworks-debug`.

---

## 13. Cross-References to Related Skills

| Need | Skill |
|---|---|
| Flutter development | `fireworks-flutter` |
| Scientific debugging | `fireworks-debug` |
| Testing methodology | `fireworks-test` |
| Code review config | `fireworks-review` |
| Security scanning | `fireworks-security` |
| Performance profiling | `fireworks-performance` |
| System architecture | `fireworks-architect` |
| Premium UI design | `fireworks-design` |
| DevOps pipelines | `fireworks-devops` |
| Task decomposition | `fireworks-taskmaster` |

---

## 14. Reference Files Index

| Reference | Contents |
|---|---|
| `references/flutter-vscode-config.md` | Complete `.vscode/` setup for Flutter -- launch.json, settings.json, extensions.json, tasks.json with full JSON |
| `references/electron-vscode-config.md` | Complete `.vscode/` setup for Electron + React + TypeScript projects |
| `references/snippets-library.md` | Full custom snippet definitions for Flutter (Riverpod, BLoC, GoRouter, Freezed) and React (components, hooks, IPC) |
| `references/debugging-advanced.md` | Advanced debugging -- compound launch configs, preLaunchTask, postDebugTask, env vars, source maps, remote debugging |
