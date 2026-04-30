# Electron + React + TypeScript VS Code Configuration — Complete Reference

> Full `.vscode/` setup for Electron projects using React, TypeScript, Vite, and Tailwind.
> Tailored for your Electron project architecture (main + renderer + preload + shared).

---

## extensions.json

```json
{
  "recommendations": [
    "dbaeumer.vscode-eslint",
    "esbenp.prettier-vscode",
    "bradlc.vscode-tailwindcss",
    "usernamehw.errorlens",
    "eamodio.gitlens",
    "rangav.vscode-thunder-client",
    "formulahendry.auto-rename-tag",
    "dsznajder.es7-react-js-snippets",
    "github.copilot",
    "pkief.material-icon-theme",
    "gruntfuggly.todo-tree",
    "aaron-bond.better-comments",
    "christian-kohler.path-intellisense",
    "editorconfig.editorconfig",
    "yoavbls.pretty-ts-errors",
    "meganrogge.template-string-converter"
  ],
  "unwantedRecommendations": []
}
```

---

## settings.json

```json
{
  // TypeScript
  "typescript.preferences.importModuleSpecifier": "non-relative",
  "typescript.suggest.autoImports": true,
  "typescript.updateImportsOnFileMove.enabled": "always",
  "typescript.tsdk": "node_modules/typescript/lib",
  "typescript.enablePromptUseWorkspaceTsdk": true,

  // Editor (TypeScript/React)
  "[typescript]": {
    "editor.defaultFormatter": "esbenp.prettier-vscode",
    "editor.formatOnSave": true,
    "editor.codeActionsOnSave": {
      "source.fixAll.eslint": "explicit",
      "source.organizeImports": "explicit"
    }
  },
  "[typescriptreact]": {
    "editor.defaultFormatter": "esbenp.prettier-vscode",
    "editor.formatOnSave": true,
    "editor.codeActionsOnSave": {
      "source.fixAll.eslint": "explicit",
      "source.organizeImports": "explicit"
    }
  },
  "[json]": {
    "editor.defaultFormatter": "esbenp.prettier-vscode"
  },
  "[jsonc]": {
    "editor.defaultFormatter": "esbenp.prettier-vscode"
  },

  // Tailwind CSS
  "tailwindCSS.experimental.classRegex": [
    ["cn\\(([^)]*)\\)", "'([^']*)'"],
    ["cva\\(([^)]*)\\)", "[\"'`]([^\"'`]*).*?[\"'`]"]
  ],
  "tailwindCSS.includeLanguages": {
    "typescriptreact": "html"
  },
  "editor.quickSuggestions": {
    "strings": "on"
  },

  // ESLint
  "eslint.validate": [
    "typescript",
    "typescriptreact",
    "javascript",
    "javascriptreact"
  ],
  "eslint.workingDirectories": [{ "mode": "auto" }],

  // General
  "editor.bracketPairColorization.enabled": true,
  "editor.guides.bracketPairs": "active",
  "editor.minimap.enabled": false,
  "files.autoSave": "onFocusChange",
  "debug.javascript.autoAttachFilter": "smart",

  // Search exclusions
  "search.exclude": {
    "**/node_modules": true,
    "**/dist": true,
    "**/out": true,
    "**/release": true,
    "**/.vite": true,
    "**/coverage": true
  },
  "files.watcherExclude": {
    "**/node_modules/**": true,
    "**/dist/**": true,
    "**/out/**": true
  },

  // File nesting
  "explorer.fileNesting.enabled": true,
  "explorer.fileNesting.patterns": {
    "package.json": "package-lock.json, yarn.lock, pnpm-lock.yaml, .npmrc, .nvmrc",
    "tsconfig.json": "tsconfig.*.json",
    "vite.config.*": "vitest.config.*, playwright.config.*",
    ".eslintrc*": ".eslintignore, .prettierrc*, .prettierignore, .editorconfig",
    "*.ts": "${capture}.test.ts, ${capture}.spec.ts",
    "*.tsx": "${capture}.test.tsx, ${capture}.spec.tsx, ${capture}.module.css"
  },

  // Todo Tree
  "todo-tree.general.tags": ["TODO", "FIXME", "HACK", "BUG", "PERF", "SECURITY"],
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
      "name": "Electron: Main Process",
      "type": "node",
      "request": "launch",
      "cwd": "${workspaceFolder}",
      "runtimeExecutable": "${workspaceFolder}/node_modules/.bin/electron",
      "runtimeArgs": [
        "--inspect=5858",
        "."
      ],
      "env": {
        "NODE_ENV": "development"
      },
      "sourceMaps": true,
      "outFiles": [
        "${workspaceFolder}/dist/**/*.js",
        "${workspaceFolder}/out/**/*.js"
      ],
      "resolveSourceMapLocations": [
        "${workspaceFolder}/**",
        "!**/node_modules/**"
      ],
      "preLaunchTask": "Vite Build (Main)"
    },
    {
      "name": "Electron: Renderer (Chrome)",
      "type": "chrome",
      "request": "attach",
      "port": 9222,
      "webRoot": "${workspaceFolder}/src/renderer",
      "sourceMaps": true,
      "sourceMapPathOverrides": {
        "webpack:///./src/*": "${workspaceFolder}/src/*",
        "webpack:///src/*": "${workspaceFolder}/src/*"
      },
      "timeout": 30000
    },
    {
      "name": "Electron: Full Debug",
      "type": "node",
      "request": "launch",
      "cwd": "${workspaceFolder}",
      "runtimeExecutable": "${workspaceFolder}/node_modules/.bin/electron",
      "runtimeArgs": [
        "--inspect=5858",
        "--remote-debugging-port=9222",
        "."
      ],
      "env": {
        "NODE_ENV": "development"
      },
      "sourceMaps": true,
      "outFiles": [
        "${workspaceFolder}/dist/**/*.js"
      ]
    },
    {
      "name": "Vite: React (Chrome)",
      "type": "chrome",
      "request": "launch",
      "url": "http://localhost:5173",
      "webRoot": "${workspaceFolder}/src",
      "sourceMaps": true,
      "sourceMapPathOverrides": {
        "webpack:///./src/*": "${workspaceFolder}/src/*"
      },
      "preLaunchTask": "Vite Dev Server"
    },
    {
      "name": "Vite: React (Edge)",
      "type": "msedge",
      "request": "launch",
      "url": "http://localhost:5173",
      "webRoot": "${workspaceFolder}/src",
      "sourceMaps": true
    },
    {
      "name": "Vitest: Current File",
      "type": "node",
      "request": "launch",
      "program": "${workspaceFolder}/node_modules/vitest/vitest.mjs",
      "args": ["run", "${relativeFile}"],
      "console": "integratedTerminal",
      "cwd": "${workspaceFolder}"
    },
    {
      "name": "Vitest: Debug Current Test",
      "type": "node",
      "request": "launch",
      "program": "${workspaceFolder}/node_modules/vitest/vitest.mjs",
      "args": ["run", "${relativeFile}", "--reporter=verbose"],
      "console": "integratedTerminal",
      "cwd": "${workspaceFolder}",
      "autoAttachChildProcesses": true
    }
  ],
  "compounds": [
    {
      "name": "Electron: Main + Renderer",
      "configurations": [
        "Electron: Main Process",
        "Electron: Renderer (Chrome)"
      ],
      "stopAll": true,
      "preLaunchTask": "Vite Build (Main)"
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
      "label": "TypeScript Check",
      "type": "shell",
      "command": "npx tsc --noEmit",
      "group": {
        "kind": "build",
        "isDefault": true
      },
      "problemMatcher": "$tsc",
      "presentation": {
        "reveal": "always",
        "panel": "shared"
      }
    },
    {
      "label": "Vite Dev Server",
      "type": "shell",
      "command": "npx vite",
      "isBackground": true,
      "problemMatcher": {
        "pattern": {
          "regexp": "^$"
        },
        "background": {
          "activeOnStart": true,
          "beginsPattern": "VITE",
          "endsPattern": "ready in"
        }
      },
      "presentation": {
        "reveal": "always",
        "panel": "dedicated"
      }
    },
    {
      "label": "Vite Build",
      "type": "shell",
      "command": "npx vite build",
      "group": "build",
      "problemMatcher": []
    },
    {
      "label": "Vite Build (Main)",
      "type": "shell",
      "command": "npx vite build --config vite.main.config.ts",
      "group": "build",
      "problemMatcher": []
    },
    {
      "label": "ESLint",
      "type": "shell",
      "command": "npx eslint src/ --ext .ts,.tsx --max-warnings=0",
      "group": "test",
      "problemMatcher": "$eslint-stylish"
    },
    {
      "label": "Vitest Run All",
      "type": "shell",
      "command": "npx vitest run",
      "group": "test",
      "problemMatcher": [],
      "presentation": {
        "reveal": "always",
        "panel": "dedicated"
      }
    },
    {
      "label": "Vitest Watch",
      "type": "shell",
      "command": "npx vitest",
      "isBackground": true,
      "group": "test",
      "problemMatcher": [],
      "presentation": {
        "reveal": "always",
        "panel": "dedicated"
      }
    },
    {
      "label": "Electron Builder",
      "type": "shell",
      "command": "npx electron-builder --config",
      "group": "build",
      "problemMatcher": []
    },
    {
      "label": "Clean Build",
      "type": "shell",
      "command": "rm -rf dist out node_modules/.vite && npm run build",
      "problemMatcher": []
    },
    {
      "label": "Full Check (Lint + Types + Test)",
      "type": "shell",
      "command": "npx eslint src/ --ext .ts,.tsx --max-warnings=0 && npx tsc --noEmit && npx vitest run",
      "group": "test",
      "problemMatcher": [],
      "presentation": {
        "reveal": "always",
        "panel": "dedicated"
      }
    }
  ]
}
```

---

## Multi-Root Workspace for Electron Projects

For large Electron apps with separate main/renderer packages:

### your project.code-workspace

```json
{
  "folders": [
    { "name": "your project Root", "path": "." },
    { "name": "Main Process", "path": "./src/main" },
    { "name": "Renderer", "path": "./src/renderer" },
    { "name": "Preload", "path": "./src/preload" },
    { "name": "Shared Types", "path": "./src/shared" }
  ],
  "settings": {
    "editor.formatOnSave": true,
    "typescript.tsdk": "node_modules/typescript/lib"
  },
  "extensions": {
    "recommendations": [
      "dbaeumer.vscode-eslint",
      "esbenp.prettier-vscode",
      "bradlc.vscode-tailwindcss"
    ]
  }
}
```

---

## Environment-Specific Settings

For different build environments, use `--dart-define` (Flutter) or `env` in launch configs (Electron):

```json
{
  "name": "Electron: Dev",
  "type": "node",
  "request": "launch",
  "env": {
    "NODE_ENV": "development",
    "DB_PATH": "${workspaceFolder}/data/dev.db",
    "LOG_LEVEL": "debug"
  }
}
```

Never hardcode secrets in launch.json. Use `.env` files (gitignored) with `envFile` property:

```json
{
  "name": "Electron: Dev with .env",
  "type": "node",
  "request": "launch",
  "envFile": "${workspaceFolder}/.env.development"
}
```
