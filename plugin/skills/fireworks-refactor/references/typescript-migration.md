# TypeScript Strict Mode Migration Reference

> Incrementally migrate a codebase from loose TypeScript to strict TypeScript. One flag at a time, one commit at a time.

---

## 1. The `any` Audit

### Finding All `any` Usage

```bash
# Direct type annotations with any
grep -rn ": any" src/

# Type assertions using any
grep -rn "as any" src/

# Generic parameters with any
grep -rn "<any>" src/

# Record/Map with any values
grep -rn "Record<string, any>" src/
grep -rn "Record<number, any>" src/

# Function signatures with any
grep -rn "(.*: any" src/

# Count total any usage (baseline metric)
grep -rn "any" src/ --include="*.ts" --include="*.tsx" | grep -v node_modules | wc -l
```

### Categorize by Difficulty

```
EASY (fix immediately):
  - `any` where the actual type is obvious from context
  - `any` on a variable that is immediately assigned a typed value
  - `any` in a catch block (replace with `unknown`)
  - `as any` used to silence a fixable type error

MEDIUM (requires interface design):
  - `any` in function parameters — need to define the expected shape
  - `any` in function return types — need to trace what is actually returned
  - `Record<string, any>` — need to define the value type
  - `any[]` — need to define the element type

HARD (requires architectural thought):
  - `any` in complex generic utility types
  - `any` in third-party library integration boundaries
  - `any` used to work around TypeScript limitations
  - `any` in deeply nested callback chains
```

---

## 2. Migration Path: `any` -> `unknown` -> Proper Type

### Step 1: Replace `any` with `unknown`

```typescript
// BEFORE: unsafe — allows any operation without checking
function processData(data: any) {
  return data.name.toUpperCase(); // no error, but crashes if data has no name
}

// AFTER: type-safe — forces you to check before using
function processData(data: unknown) {
  // TypeScript error: Object is of type 'unknown'
  // return data.name.toUpperCase();

  // Must narrow the type first
  if (typeof data === 'object' && data !== null && 'name' in data) {
    const record = data as { name: string };
    return record.name.toUpperCase();
  }
  throw new Error('Invalid data shape');
}
```

### Step 2: Replace `unknown` with a Proper Type

```typescript
// BEST: define the exact shape
interface UserData {
  name: string;
  email: string;
  age: number;
}

function processData(data: UserData) {
  return data.name.toUpperCase(); // fully type-safe, no narrowing needed
}
```

---

## 3. Type Guard Patterns

### Basic Type Guards

```typescript
// Primitive type guard
function isString(val: unknown): val is string {
  return typeof val === 'string';
}

// Object type guard with shape checking
function isProduct(val: unknown): val is Product {
  return (
    typeof val === 'object' &&
    val !== null &&
    'sku' in val &&
    'price' in val &&
    typeof (val as Product).sku === 'string' &&
    typeof (val as Product).price === 'number'
  );
}

// Array type guard
function isStringArray(val: unknown): val is string[] {
  return Array.isArray(val) && val.every((item) => typeof item === 'string');
}

// Nullable type guard
function isDefined<T>(val: T | null | undefined): val is T {
  return val !== null && val !== undefined;
}
```

### Discriminated Union Guards

```typescript
type Result<T> =
  | { status: 'success'; data: T }
  | { status: 'error'; error: string };

function isSuccess<T>(result: Result<T>): result is { status: 'success'; data: T } {
  return result.status === 'success';
}

function isError<T>(result: Result<T>): result is { status: 'error'; error: string } {
  return result.status === 'error';
}

// Usage
function handleResult(result: Result<User>) {
  if (isSuccess(result)) {
    console.log(result.data.name); // TypeScript knows data exists
  } else {
    console.error(result.error); // TypeScript knows error exists
  }
}
```

### Assertion Functions (TypeScript 3.7+)

```typescript
function assertIsUser(val: unknown): asserts val is User {
  if (typeof val !== 'object' || val === null) {
    throw new Error('Expected an object');
  }
  if (!('name' in val) || !('email' in val)) {
    throw new Error('Expected User shape');
  }
}

// Usage — after the assertion, TypeScript narrows the type
function processUser(data: unknown) {
  assertIsUser(data);
  // TypeScript knows data is User from here onward
  console.log(data.name, data.email);
}
```

---

## 4. Strict Mode Flags — Enable One at a Time

### Flag 1: `strictNullChecks`

**What it catches**: Variables that could be `null` or `undefined` used without checking.

```jsonc
// tsconfig.json
{ "compilerOptions": { "strictNullChecks": true } }
```

**Common fixes**:
```typescript
// BEFORE: crashes at runtime if user is null
const name = user.name;

// AFTER: handle the null case
const name = user?.name ?? 'Unknown';

// Or with explicit check
if (user) {
  const name = user.name;
}
```

### Flag 2: `noImplicitAny`

**What it catches**: Parameters and variables where TypeScript infers `any` because no type was provided.

```jsonc
{ "compilerOptions": { "noImplicitAny": true } }
```

**Common fixes**:
```typescript
// BEFORE: 'e' implicitly has 'any' type
function handler(e) { console.log(e.target.value); }

// AFTER: explicit type
function handler(e: React.ChangeEvent<HTMLInputElement>) {
  console.log(e.target.value);
}
```

### Flag 3: `strictFunctionTypes`

**What it catches**: Function type parameter contravariance violations. Functions assigned to types with incompatible parameter types.

```jsonc
{ "compilerOptions": { "strictFunctionTypes": true } }
```

**Common fixes**: Ensure callback parameter types match exactly. Use generics when handler types need to be flexible.

### Flag 4: `strictPropertyInitialization`

**What it catches**: Class properties declared but not initialized in the constructor.

```jsonc
{ "compilerOptions": { "strictPropertyInitialization": true } }
```

**Common fixes**:
```typescript
// BEFORE: Property 'name' has no initializer
class User {
  name: string;
}

// AFTER: option A — initialize in constructor
class User {
  name: string;
  constructor(name: string) { this.name = name; }
}

// AFTER: option B — definite assignment assertion (if set elsewhere, e.g., by a framework)
class User {
  name!: string; // the ! tells TS "I know this will be set"
}

// AFTER: option C — default value
class User {
  name: string = '';
}
```

### Flag 5: `strict` (Master Flag)

Enables ALL strict flags at once. Only enable this after you have enabled and fixed each individual flag.

```jsonc
{ "compilerOptions": { "strict": true } }
```

---

## 5. Incremental Strategy

### The Commit-Per-Flag Workflow

```
1. Enable ONE strict flag in tsconfig.json
2. Run `tsc --noEmit` — see all new errors
3. Fix errors file by file (easiest files first)
4. Run `tsc --noEmit` after every few files to track progress
5. When zero errors remain, run the full test suite
6. Commit: `refactor: enable strictNullChecks — fix all N errors`
7. Move to the next flag, repeat from step 1
```

### Per-File Override (For Large Codebases)

If enabling a flag globally produces too many errors, use `// @ts-expect-error` temporarily:

```typescript
// @ts-expect-error — TODO: fix strict null check (tracked in issue #123)
const name = user.name;
```

Then systematically remove these comments, one file at a time.

---

## 6. Generic Pattern Replacements

### Replace `any[]` with `T[]`

```typescript
// BEFORE
function first(arr: any[]): any {
  return arr[0];
}

// AFTER
function first<T>(arr: T[]): T | undefined {
  return arr[0];
}
```

### Replace `Record<string, any>` with a Proper Interface

```typescript
// BEFORE
function updateSettings(settings: Record<string, any>) { ... }

// AFTER
interface AppSettings {
  theme: 'light' | 'dark';
  fontSize: number;
  language: string;
  notifications: boolean;
}
function updateSettings(settings: Partial<AppSettings>) { ... }
```

### Replace `Function` Type with Specific Signature

```typescript
// BEFORE
function debounce(fn: Function, ms: number): Function { ... }

// AFTER
function debounce<T extends (...args: unknown[]) => unknown>(
  fn: T,
  ms: number
): (...args: Parameters<T>) => void { ... }
```

### Replace `object` with Specific Shape

```typescript
// BEFORE
function merge(a: object, b: object): object { ... }

// AFTER
function merge<A extends Record<string, unknown>, B extends Record<string, unknown>>(
  a: A, b: B
): A & B { ... }
```

---

## 7. Measuring Progress

Track these metrics across the migration:

```bash
# Count of explicit `any` annotations
grep -rn ": any\|as any\|<any>" src/ --include="*.ts" --include="*.tsx" | wc -l

# Count of @ts-expect-error / @ts-ignore
grep -rn "@ts-expect-error\|@ts-ignore" src/ --include="*.ts" --include="*.tsx" | wc -l

# TypeScript error count
tsc --noEmit 2>&1 | grep "error TS" | wc -l

# Strict flags enabled (check tsconfig.json)
grep -c "strict\|strictNullChecks\|noImplicitAny\|strictFunctionTypes\|strictPropertyInitialization" tsconfig.json
```

Log these numbers in each migration commit message so progress is visible in the git history.
