# Flutter VS Code Configuration — Complete Reference

> Full `.vscode/` setup for Flutter projects. Copy these configs as starting points and customize per project.

---

## extensions.json

```json
{
  "recommendations": [
    "dart-code.dart-code",
    "dart-code.flutter",
    "robert-brunhage.flutter-riverpod-snippets",
    "nash.awesome-flutter-snippets",
    "alexisvt.flutter-snippets",
    "usernamehw.errorlens",
    "bendixma.dart-data-class-generator",
    "github.copilot",
    "pkief.material-icon-theme",
    "gruntfuggly.todo-tree",
    "aaron-bond.better-comments"
  ],
  "unwantedRecommendations": []
}
```

---

## settings.json

```json
{
  // Dart / Flutter
  "dart.flutterSdkPath": "C:/flutter",
  "dart.lineLength": 80,
  "dart.previewFlutterUiGuides": true,
  "dart.previewFlutterUiGuidesCustomTracking": true,
  "dart.openDevTools": "flutter",
  "dart.debugExternalPackageLibraries": false,
  "dart.debugSdkLibraries": false,
  "dart.showTodos": true,
  "dart.enableCompletionCommitCharacters": true,
  "dart.closingLabels": true,
  "dart.warnWhenEditingFilesOutsideWorkspace": true,

  // Editor
  "[dart]": {
    "editor.formatOnSave": true,
    "editor.formatOnType": true,
    "editor.selectionHighlight": false,
    "editor.suggestSelection": "first",
    "editor.tabCompletion": "onlySnippets",
    "editor.wordBasedSuggestions": "off",
    "editor.defaultFormatter": "Dart-Code.dart-code",
    "editor.rulers": [80],
    "editor.codeActionsOnSave": {
      "source.fixAll": "explicit",
      "source.organizeImports": "explicit"
    }
  },

  // General
  "editor.bracketPairColorization.enabled": true,
  "editor.guides.bracketPairs": "active",
  "editor.minimap.enabled": false,
  "files.autoSave": "onFocusChange",
  "explorer.fileNesting.enabled": true,
  "explorer.fileNesting.patterns": {
    "pubspec.yaml": "pubspec.lock, .packages, .flutter-plugins, .flutter-plugins-dependencies, .metadata, analysis_options.yaml",
    "*.dart": "${capture}.g.dart, ${capture}.freezed.dart, ${capture}.gr.dart"
  },

  // Search exclusions
  "search.exclude": {
    "**/.dart_tool": true,
    "**/.fvm": true,
    "**/build": true,
    "**/*.g.dart": true,
    "**/*.freezed.dart": true,
    "**/*.gr.dart": true
  },
  "files.watcherExclude": {
    "**/.dart_tool/**": true,
    "**/.fvm/**": true,
    "**/build/**": true
  },

  // Todo Tree
  "todo-tree.general.tags": ["TODO", "FIXME", "HACK", "BUG", "PERF"],
  "todo-tree.highlights.defaultHighlight": {
    "type": "text-and-comment"
  }
}
```

---

## launch.json

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "Flutter Debug (Chrome)",
      "type": "dart",
      "request": "launch",
      "program": "lib/main.dart",
      "deviceId": "chrome",
      "args": ["--web-port=5000"],
      "flutterMode": "debug"
    },
    {
      "name": "Flutter Debug (iOS Simulator)",
      "type": "dart",
      "request": "launch",
      "program": "lib/main.dart",
      "deviceId": "iPhone 16 Pro"
    },
    {
      "name": "Flutter Debug (Android Emulator)",
      "type": "dart",
      "request": "launch",
      "program": "lib/main.dart",
      "deviceId": "emulator-5554"
    },
    {
      "name": "Flutter Debug (macOS)",
      "type": "dart",
      "request": "launch",
      "program": "lib/main.dart",
      "deviceId": "macos"
    },
    {
      "name": "Flutter Debug (Windows)",
      "type": "dart",
      "request": "launch",
      "program": "lib/main.dart",
      "deviceId": "windows"
    },
    {
      "name": "Flutter Profile",
      "type": "dart",
      "request": "launch",
      "program": "lib/main.dart",
      "flutterMode": "profile"
    },
    {
      "name": "Flutter Release",
      "type": "dart",
      "request": "launch",
      "program": "lib/main.dart",
      "flutterMode": "release"
    },
    {
      "name": "Flutter Attach",
      "type": "dart",
      "request": "attach"
    },
    {
      "name": "Flutter Debug (Staging)",
      "type": "dart",
      "request": "launch",
      "program": "lib/main_staging.dart",
      "args": ["--dart-define=ENV=staging"]
    },
    {
      "name": "Flutter Debug (Production)",
      "type": "dart",
      "request": "launch",
      "program": "lib/main_production.dart",
      "args": ["--dart-define=ENV=production"]
    }
  ]
}
```

---

## tasks.json

```json
{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "Flutter Analyze",
      "type": "shell",
      "command": "flutter analyze",
      "group": {
        "kind": "test",
        "isDefault": true
      },
      "problemMatcher": "$dart-analyze",
      "presentation": {
        "reveal": "always",
        "panel": "shared"
      }
    },
    {
      "label": "Flutter Test (All)",
      "type": "shell",
      "command": "flutter test",
      "group": "test",
      "problemMatcher": [],
      "presentation": {
        "reveal": "always",
        "panel": "dedicated"
      }
    },
    {
      "label": "Flutter Test (Current File)",
      "type": "shell",
      "command": "flutter test ${file}",
      "group": "test",
      "problemMatcher": [],
      "presentation": {
        "reveal": "always",
        "panel": "dedicated"
      }
    },
    {
      "label": "Flutter Test (Coverage)",
      "type": "shell",
      "command": "flutter test --coverage && genhtml coverage/lcov.info -o coverage/html",
      "group": "test",
      "problemMatcher": []
    },
    {
      "label": "Build Runner Watch",
      "type": "shell",
      "command": "dart run build_runner watch --delete-conflicting-outputs",
      "group": "build",
      "isBackground": true,
      "problemMatcher": [],
      "presentation": {
        "reveal": "always",
        "panel": "dedicated"
      }
    },
    {
      "label": "Build Runner Build",
      "type": "shell",
      "command": "dart run build_runner build --delete-conflicting-outputs",
      "group": "build",
      "problemMatcher": []
    },
    {
      "label": "Flutter Clean",
      "type": "shell",
      "command": "flutter clean && flutter pub get",
      "problemMatcher": []
    },
    {
      "label": "Flutter Build APK (Release)",
      "type": "shell",
      "command": "flutter build apk --release",
      "group": "build",
      "problemMatcher": []
    },
    {
      "label": "Flutter Build Web (Release)",
      "type": "shell",
      "command": "flutter build web --release",
      "group": "build",
      "problemMatcher": []
    },
    {
      "label": "Flutter Build iOS (Release)",
      "type": "shell",
      "command": "flutter build ios --release",
      "group": "build",
      "problemMatcher": []
    },
    {
      "label": "Pub Get",
      "type": "shell",
      "command": "flutter pub get",
      "problemMatcher": []
    }
  ]
}
```

---

## .gitignore Additions for .vscode/

```gitignore
# Commit these (team-shared):
# .vscode/extensions.json
# .vscode/launch.json
# .vscode/tasks.json
# .vscode/settings.json (if no local paths)

# Ignore these (local-only):
.vscode/*.code-snippets  # unless team snippets
.vscode/.ropeproject
.vscode/ltex.*
```

---

## File Nesting Patterns

With file nesting enabled, generated files (`.g.dart`, `.freezed.dart`) collapse under their source file in the Explorer panel. This keeps the file tree clean while keeping generated files accessible.

The patterns in settings.json above handle:
- `pubspec.yaml` nests lock files, plugin configs, and analysis options
- `*.dart` nests codegen output (`.g.dart`, `.freezed.dart`, `.gr.dart`)
