# VS Code Snippets Library — Complete Reference

> Custom snippet definitions for Flutter and React/Electron development.
> Place Flutter snippets in `.vscode/dart.code-snippets`.
> Place React/TS snippets in `.vscode/typescriptreact.code-snippets`.

---

## Flutter Snippets

### Riverpod Provider (codegen)

**Prefix:** `rprov`

```json
{
  "Riverpod Provider (codegen)": {
    "prefix": "rprov",
    "body": [
      "@riverpod",
      "Future<${1:ReturnType}> ${2:providerName}(Ref ref) async {",
      "  $0",
      "}"
    ],
    "description": "Riverpod @riverpod async provider with codegen"
  }
}
```

### Riverpod Notifier (codegen)

**Prefix:** `rnotif`

```json
{
  "Riverpod Notifier (codegen)": {
    "prefix": "rnotif",
    "body": [
      "@riverpod",
      "class ${1:Name} extends _$${1:Name} {",
      "  @override",
      "  ${2:FutureOr<${3:State}>} build() {",
      "    $0",
      "  }",
      "}"
    ],
    "description": "Riverpod @riverpod class notifier with codegen"
  }
}
```

### Riverpod Family Provider

**Prefix:** `rprovfam`

```json
{
  "Riverpod Family Provider": {
    "prefix": "rprovfam",
    "body": [
      "@riverpod",
      "Future<${1:ReturnType}> ${2:providerName}(Ref ref, ${3:String} ${4:id}) async {",
      "  $0",
      "}"
    ],
    "description": "Riverpod family provider with parameter"
  }
}
```

### Riverpod Consumer Widget

**Prefix:** `rconsumer`

```json
{
  "Riverpod Consumer Widget": {
    "prefix": "rconsumer",
    "body": [
      "class ${1:Name} extends ConsumerWidget {",
      "  const ${1:Name}({super.key});",
      "",
      "  @override",
      "  Widget build(BuildContext context, WidgetRef ref) {",
      "    $0",
      "    return const Placeholder();",
      "  }",
      "}"
    ],
    "description": "Riverpod ConsumerWidget"
  }
}
```

### BLoC Event (Freezed)

**Prefix:** `blocevent`

```json
{
  "BLoC Event (Freezed)": {
    "prefix": "blocevent",
    "body": [
      "@freezed",
      "sealed class ${1:Feature}Event with _$${1:Feature}Event {",
      "  const factory ${1:Feature}Event.${2:started}() = _${2:Started};",
      "  const factory ${1:Feature}Event.${3:loaded}(${4:List<$5>} ${6:data}) = _${3:Loaded};",
      "  $0",
      "}"
    ],
    "description": "BLoC event sealed class with Freezed"
  }
}
```

### BLoC State (Freezed)

**Prefix:** `blocstate`

```json
{
  "BLoC State (Freezed)": {
    "prefix": "blocstate",
    "body": [
      "@freezed",
      "sealed class ${1:Feature}State with _$${1:Feature}State {",
      "  const factory ${1:Feature}State.initial() = _Initial;",
      "  const factory ${1:Feature}State.loading() = _Loading;",
      "  const factory ${1:Feature}State.loaded(${2:List<$3>} ${4:data}) = _Loaded;",
      "  const factory ${1:Feature}State.error(String message) = _Error;",
      "}"
    ],
    "description": "BLoC state sealed class with Freezed"
  }
}
```

### GoRouter Route

**Prefix:** `goroute`

```json
{
  "GoRouter Route": {
    "prefix": "goroute",
    "body": [
      "GoRoute(",
      "  path: '/${1:path}',",
      "  name: '${2:routeName}',",
      "  builder: (context, state) => const ${3:Screen}(),",
      "  routes: [",
      "    $0",
      "  ],",
      "),"
    ],
    "description": "GoRouter route definition"
  }
}
```

### GoRouter Shell Route (Bottom Nav)

**Prefix:** `goshell`

```json
{
  "GoRouter Shell Route": {
    "prefix": "goshell",
    "body": [
      "StatefulShellRoute.indexedStack(",
      "  builder: (context, state, navigationShell) {",
      "    return ScaffoldWithNavBar(navigationShell: navigationShell);",
      "  },",
      "  branches: [",
      "    StatefulShellBranch(",
      "      routes: [",
      "        GoRoute(",
      "          path: '/${1:tab1}',",
      "          builder: (context, state) => const ${2:Tab1Screen}(),",
      "        ),",
      "      ],",
      "    ),",
      "    $0",
      "  ],",
      "),"
    ],
    "description": "GoRouter StatefulShellRoute with indexed stack"
  }
}
```

### Freezed Data Model

**Prefix:** `frzmodel`

```json
{
  "Freezed Data Model": {
    "prefix": "frzmodel",
    "body": [
      "@freezed",
      "abstract class ${1:ModelName} with _$${1:ModelName} {",
      "  const factory ${1:ModelName}({",
      "    required ${2:String} ${3:id},",
      "    required ${4:String} ${5:name},",
      "    $0",
      "  }) = _${1:ModelName};",
      "",
      "  factory ${1:ModelName}.fromJson(Map<String, dynamic> json) =>",
      "      _$${1:ModelName}FromJson(json);",
      "}"
    ],
    "description": "Freezed data model with JSON serialization"
  }
}
```

### Flutter StatelessWidget

**Prefix:** `stless`

```json
{
  "Flutter StatelessWidget": {
    "prefix": "stless",
    "body": [
      "class ${1:Name} extends StatelessWidget {",
      "  const ${1:Name}({super.key});",
      "",
      "  @override",
      "  Widget build(BuildContext context) {",
      "    return $0;",
      "  }",
      "}"
    ],
    "description": "Stateless widget with const constructor"
  }
}
```

### Flutter Widget Test

**Prefix:** `wtest`

```json
{
  "Flutter Widget Test": {
    "prefix": "wtest",
    "body": [
      "testWidgets('${1:description}', (tester) async {",
      "  await tester.pumpWidget(",
      "    const MaterialApp(",
      "      home: ${2:Widget}(),",
      "    ),",
      "  );",
      "",
      "  $0",
      "",
      "  expect(find.byType(${2:Widget}), findsOneWidget);",
      "});"
    ],
    "description": "Flutter widget test with MaterialApp wrapper"
  }
}
```

---

## React / TypeScript Snippets

### React Functional Component

**Prefix:** `rfc`

```json
{
  "React Functional Component": {
    "prefix": "rfc",
    "body": [
      "interface ${1:Component}Props {",
      "  $2",
      "}",
      "",
      "export const ${1:Component} = ({ $3 }: ${1:Component}Props) => {",
      "  return (",
      "    <div className=\"$4\">",
      "      $0",
      "    </div>",
      "  );",
      "};"
    ],
    "description": "React functional component with TypeScript props"
  }
}
```

### React Custom Hook

**Prefix:** `rhook`

```json
{
  "React Custom Hook": {
    "prefix": "rhook",
    "body": [
      "import { useState, useEffect } from 'react';",
      "",
      "export const use${1:HookName} = (${2:params}: ${3:ParamType}) => {",
      "  const [${4:state}, set${4/(.*)/${4:/capitalize}/}] = useState<${5:StateType}>(${6:initialValue});",
      "",
      "  useEffect(() => {",
      "    $0",
      "  }, []);",
      "",
      "  return { ${4:state} };",
      "};"
    ],
    "description": "Custom React hook with TypeScript"
  }
}
```

### Zustand Store

**Prefix:** `zustand`

```json
{
  "Zustand Store": {
    "prefix": "zustand",
    "body": [
      "import { create } from 'zustand';",
      "",
      "interface ${1:Store}State {",
      "  ${2:items}: ${3:Item}[];",
      "  ${4:isLoading}: boolean;",
      "  ${5:fetch${2/(.*)/${2:/capitalize}/}}: () => Promise<void>;",
      "}",
      "",
      "export const use${1:Store}Store = create<${1:Store}State>((set, get) => ({",
      "  ${2:items}: [],",
      "  ${4:isLoading}: false,",
      "  ${5:fetch${2/(.*)/${2:/capitalize}/}}: async () => {",
      "    set({ ${4:isLoading}: true });",
      "    try {",
      "      $0",
      "      set({ ${2:items}: result, ${4:isLoading}: false });",
      "    } catch (error) {",
      "      set({ ${4:isLoading}: false });",
      "      console.error(error);",
      "    }",
      "  },",
      "}));"
    ],
    "description": "Zustand store with TypeScript"
  }
}
```

### IPC Main Handler

**Prefix:** `ipcmain`

```json
{
  "IPC Main Handler": {
    "prefix": "ipcmain",
    "body": [
      "ipcMain.handle('${1:channel}', async (_event, ${2:args}: ${3:ArgType}): Promise<${4:ReturnType}> => {",
      "  try {",
      "    $0",
      "  } catch (error) {",
      "    console.error('[IPC] ${1:channel} error:', error);",
      "    throw error;",
      "  }",
      "});"
    ],
    "description": "Electron IPC main handler with typed channel"
  }
}
```

### IPC Renderer Invoke

**Prefix:** `ipcrender`

```json
{
  "IPC Renderer Invoke": {
    "prefix": "ipcrender",
    "body": [
      "const ${1:result} = await window.electron.invoke<${2:ReturnType}>('${3:channel}', ${4:args});"
    ],
    "description": "Electron IPC renderer invoke with typed channel"
  }
}
```

### IPC Preload Bridge

**Prefix:** `ipcbridge`

```json
{
  "IPC Preload Bridge": {
    "prefix": "ipcbridge",
    "body": [
      "contextBridge.exposeInMainWorld('${1:api}', {",
      "  ${2:methodName}: (${3:args}: ${4:ArgType}): Promise<${5:ReturnType}> =>",
      "    ipcRenderer.invoke('${6:channel}', ${3:args}),",
      "  $0",
      "});"
    ],
    "description": "Electron preload context bridge"
  }
}
```

### Vitest Test

**Prefix:** `vtest`

```json
{
  "Vitest Test": {
    "prefix": "vtest",
    "body": [
      "import { describe, it, expect } from 'vitest';",
      "",
      "describe('${1:module}', () => {",
      "  it('${2:should do something}', () => {",
      "    $0",
      "    expect(true).toBe(true);",
      "  });",
      "});"
    ],
    "description": "Vitest test with describe/it/expect"
  }
}
```

### Error Boundary

**Prefix:** `errbound`

```json
{
  "Error Boundary Component": {
    "prefix": "errbound",
    "body": [
      "import { Component, ErrorInfo, ReactNode } from 'react';",
      "",
      "interface Props {",
      "  children: ReactNode;",
      "  fallback?: ReactNode;",
      "}",
      "",
      "interface State {",
      "  hasError: boolean;",
      "  error: Error | null;",
      "}",
      "",
      "export class ${1:Name}ErrorBoundary extends Component<Props, State> {",
      "  state: State = { hasError: false, error: null };",
      "",
      "  static getDerivedStateFromError(error: Error): State {",
      "    return { hasError: true, error };",
      "  }",
      "",
      "  componentDidCatch(error: Error, info: ErrorInfo) {",
      "    console.error('[${1:Name}ErrorBoundary]', error, info);",
      "  }",
      "",
      "  render() {",
      "    if (this.state.hasError) {",
      "      return this.props.fallback ?? <div>Something went wrong.</div>;",
      "    }",
      "    return this.props.children;",
      "  }",
      "}"
    ],
    "description": "React Error Boundary with TypeScript"
  }
}
```

---

## How to Install Snippets

### Project-level (recommended for teams)

Place snippet files in `.vscode/` directory:
- `.vscode/dart.code-snippets` — Flutter/Dart snippets
- `.vscode/typescriptreact.code-snippets` — React/TSX snippets
- `.vscode/typescript.code-snippets` — TypeScript-only snippets

These are committed to git and shared with the team.

### User-level (personal)

1. Open Command Palette (`Ctrl+Shift+P`)
2. Type "Preferences: Configure User Snippets"
3. Select language (dart, typescriptreact, typescript)
4. Add snippet definitions

User snippets are stored in `%APPDATA%/Code/User/snippets/`.
