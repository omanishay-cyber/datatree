# Dead Code Detection & Removal Reference

> Dead code is code that exists but is never executed. It adds cognitive load, increases bundle size, and creates false confidence in test coverage. Remove it.

---

## 1. Detection Methods

### Unused Exports

Every exported symbol should be imported somewhere. If it is not, it is dead.

```bash
# For each exported function/class/const, search for its import
# Example: check if `calculateTax` is imported anywhere
grep -rn "import.*calculateTax" src/

# If the only match is the file that defines it, it is dead code

# Automated approach: list all exports, then check each
grep -rn "export " src/ --include="*.ts" --include="*.tsx" | \
  grep -v "node_modules" | \
  grep -v ".d.ts"
```

### Unreferenced Functions

Functions that are defined but never called.

```bash
# Search for the function name across the entire codebase
# If only the definition appears, the function is dead
grep -rn "functionName" src/ --include="*.ts" --include="*.tsx"

# For class methods, search for .methodName
grep -rn "\.methodName" src/ --include="*.ts" --include="*.tsx"
```

### Unreachable Code Branches

Code after `return`, `throw`, or `break` statements. Conditions that are always true or always false.

```bash
# TypeScript compiler catches some of these:
tsc --noEmit --allowUnreachableCode false
```

Common patterns:
```typescript
// Dead: code after return
function example() {
  return 42;
  console.log('never runs'); // DEAD
}

// Dead: always-true condition
if (true) {
  doSomething();
} else {
  doOtherThing(); // DEAD
}

// Dead: impossible type narrowing
function process(val: string) {
  if (typeof val === 'number') {
    handleNumber(val); // DEAD — val is always string
  }
}
```

### Commented-Out Code

Code that has been commented out "just in case." Version control makes this unnecessary.

```bash
# Find large blocks of commented-out code
grep -rn "^[[:space:]]*//" src/ --include="*.ts" --include="*.tsx" | \
  grep -E "function |const |let |var |return |import |export |class "
```

**Rule**: If code is commented out, DELETE IT. Git history preserves everything. Commented-out code is noise.

### Unused Imports

Imports that are never used in the file.

```bash
# TypeScript compiler flags these:
tsc --noEmit --noUnusedLocals

# ESLint rule:
# "no-unused-vars": "error"
# "@typescript-eslint/no-unused-vars": "error"
```

---

## 2. Tool-Assisted Detection

### TypeScript Compiler Flags

```jsonc
// tsconfig.json — enable these for detection
{
  "compilerOptions": {
    "noUnusedLocals": true,         // flag unused local variables
    "noUnusedParameters": true,      // flag unused function parameters
    "allowUnreachableCode": false,   // flag unreachable code
    "allowUnusedLabels": false       // flag unused labels
  }
}
```

### ESLint Rules

```jsonc
// .eslintrc — add these rules
{
  "rules": {
    "no-unused-vars": "off",
    "@typescript-eslint/no-unused-vars": ["error", {
      "argsIgnorePattern": "^_",
      "varsIgnorePattern": "^_"
    }],
    "no-unreachable": "error",
    "no-unused-expressions": "error"
  }
}
```

### Dependency Analysis

```bash
# List all installed dependencies
npm ls --depth=0

# For each dependency, check if it is actually imported
# Check both import and require patterns
grep -rn "from ['\"]<package-name>" src/
grep -rn "require(['\"]<package-name>" src/

# If no results, the dependency is unused — remove it
npm uninstall <package-name>
```

---

## 3. Safe Removal Protocol

Before removing any code, verify ALL of these:

### Checklist

```
[ ] No dynamic imports reference this code
    - Search for: import() with variable paths, require() with variables
    - Dynamic imports can reference code that grep does not find statically

[ ] No reflection or string-based lookups
    - Search for: Object.keys, eval, property access via bracket notation
    - Some frameworks resolve handlers by string name at runtime

[ ] Not test-only code
    - Check __tests__/, *.test.*, *.spec.*, test-utils/
    - Test helpers may look unused in src/ but are used in tests

[ ] Not used by build scripts or config files
    - Check: webpack.config, vite.config, jest.config, scripts/
    - Build plugins may reference source files

[ ] Not referenced in HTML templates or CSS
    - Check: *.html, *.css, *.scss for class names, IDs, data attributes
    - Template engines may use code that TypeScript cannot analyze

[ ] Not a public API consumed by external packages
    - If this is a library, check that removed exports are not documented
    - Consumers outside this repo will not appear in grep results

[ ] Not referenced via environment variables or config files
    - Check: .env, config.json, settings files
    - Feature flags may gate code that looks unused
```

### Removal Process

```
1. Identify the dead code (use detection methods above)
2. Run through the safety checklist (all boxes must be checked)
3. Delete the code
4. Run `tsc --noEmit` — must pass
5. Run the full test suite — must pass
6. Verify the app starts and runs correctly
7. Commit: `refactor: remove dead code — <description of what was removed>`
```

---

## 4. Import Cleanup

### Remove Unused Imports

```typescript
// BEFORE: three unused imports
import { useState, useEffect, useCallback, useMemo } from 'react';
// Only useState and useEffect are used in the file

// AFTER: only what is needed
import { useState, useEffect } from 'react';
```

### Consolidate Barrel Exports

```typescript
// BEFORE: many separate imports from the same directory
import { Button } from './components/Button';
import { Input } from './components/Input';
import { Modal } from './components/Modal';

// Create a barrel file: components/index.ts
export { Button } from './Button';
export { Input } from './Input';
export { Modal } from './Modal';

// AFTER: single clean import
import { Button, Input, Modal } from './components';
```

### Remove Re-Exports of Deleted Code

When you delete a module, also check all barrel files (index.ts) that re-export from it. Remove those re-export lines.

---

## 5. Dependency Cleanup

### Find Unused npm Packages

```bash
# Step 1: List all dependencies
cat package.json | grep -E "\"[^\"]+\":" | head -50

# Step 2: For each dependency, search for usage
# Check both import and require patterns
grep -rn "from ['\"]<package-name>" src/
grep -rn "require(['\"]<package-name>" src/

# Step 3: Also check config files and scripts
grep -rn "<package-name>" *.config.* scripts/ .github/
```

### Safe Dependency Removal

```bash
# 1. Remove the package
npm uninstall <package-name>

# 2. Run TypeScript compilation
tsc --noEmit

# 3. Run tests
npm test

# 4. Start the app and verify
npm run dev

# 5. If everything works, commit
git commit -m "chore: remove unused dependency <package-name>"
```

### DevDependency vs Dependency Audit

```bash
# Check if a devDependency is actually used at runtime (should be in dependencies)
# Check if a dependency is only used in tests/build (should be in devDependencies)

# List runtime dependencies used only in test files
# These should be moved to devDependencies
```

---

## 6. Prevention

### Prevent Dead Code from Accumulating

1. **Enable TypeScript unused checks** in tsconfig.json (noUnusedLocals, noUnusedParameters)
2. **Enable ESLint no-unused-vars** rule as error, not warning
3. **Review PRs for dead code** -- reject additions of unused exports
4. **Never comment out code** -- delete it, git has history
5. **Run periodic audits** -- schedule dead code sweeps quarterly
6. **Delete feature-flagged code** once the flag is permanently on or off
