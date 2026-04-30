# Simplicity Review Reference — Fireworks Review

Detailed checklist for the **Simplicity** lens. Use this reference when reviewing code for unnecessary complexity, dead code, and over-engineering.

---

## YAGNI Checklist

"You Aren't Gonna Need It" — build what is needed NOW, not what MIGHT be needed later.

### Feature Flags for One Condition
```typescript
// YAGNI: Config system for a single boolean
const config = loadConfig();
if (config.features.enableNewCheckout) { ... }

// SIMPLER: Just use a constant until you actually need config
const ENABLE_NEW_CHECKOUT = true;
if (ENABLE_NEW_CHECKOUT) { ... }
```
- One feature flag is a constant, not a configuration system
- Build the config system when you have 3+ flags, not before
- Dead feature flags that are always `true` or always `false` should be removed

### Generic Factories for One Type
```typescript
// YAGNI: Factory pattern for one product type
class WidgetFactory {
  create(type: string): Widget {
    switch (type) {
      case 'standard': return new StandardWidget();
      default: throw new Error(`Unknown type: ${type}`);
    }
  }
}

// SIMPLER: Direct instantiation
const widget = new StandardWidget();
```
- Factories make sense when you have 3+ types or need runtime type selection
- A factory with one case in the switch is an unnecessary indirection
- Extract a factory when the second type is actually needed, not before

### Abstract Classes with One Implementation
```typescript
// YAGNI: Abstract class with single concrete class
abstract class BaseRepository<T> {
  abstract findAll(): T[];
  abstract findById(id: string): T | null;
  abstract save(item: T): void;
}
class ProductRepository extends BaseRepository<Product> { ... }

// SIMPLER: Just the concrete class (until you need a second repo)
class ProductRepository {
  findAll(): Product[] { ... }
  findById(id: string): Product | null { ... }
  save(item: Product): void { ... }
}
```
- Abstractions should be extracted from concrete implementations, not designed upfront
- Wait for the pattern to emerge from 2-3 concrete cases before abstracting
- TypeScript interfaces are lightweight enough to use freely, but abstract classes add overhead

### Config Systems for One Value
- A JSON/YAML config file for a single setting is overhead
- Environment variable or a constant is sufficient until complexity grows
- Config parsers, validators, and loaders for one value = over-engineering
- Build config infrastructure when you have 5+ values that change between environments

### Plugin Architectures for One Plugin
- A plugin system with registration, lifecycle hooks, and dependency resolution for one plugin
- Just import the module directly until you have 2+ plugins
- Plugin systems have significant maintenance cost — earn that cost first
- Extension points should emerge from real needs, not theoretical flexibility

---

## Over-Engineering Signs

### 5+ Layers for Simple CRUD
```
// OVER-ENGINEERED: 6 layers for "get products"
Controller -> Service -> UseCase -> Repository -> DataMapper -> Database

// APPROPRIATE for a simple app:
IPC Handler -> Repository -> Database
// The handler validates input, the repo queries, the DB stores
```
- Each layer must earn its existence by providing a distinct responsibility
- If two layers always just pass through to the next, collapse them
- Electron apps with sql.js rarely need more than 3 layers (handler -> service -> database)

### Design Patterns for Pattern's Sake
- Using Observer pattern when a simple callback would work
- Using Strategy pattern when an if/else with 2 branches is clearer
- Using Builder pattern for an object with 3 fields
- Using Singleton pattern when a module-level variable is sufficient (ESM modules are already singletons)
- **Rule**: Name the problem first, then decide if a pattern solves it

### Premature Optimization
- Caching data that is only fetched once per session
- Memoizing a function that takes microseconds to execute
- Using Web Workers for a calculation that takes 5ms
- Building a custom virtual scroll for a list with 50 items
- Implementing pagination for a dataset with 100 records
- **Rule**: Measure first, optimize second. Premature optimization = complexity without evidence.

### Excessive Type Gymnastics
```typescript
// OVER-ENGINEERED: Complex generic type for simple use case
type DeepPartial<T> = {
  [P in keyof T]?: T[P] extends object ? DeepPartial<T[P]> : T[P];
};
type StrictOmit<T, K extends keyof T> = Pick<T, Exclude<keyof T, K>>;
type RequiredKeys<T> = { [K in keyof T]-?: {} extends Pick<T, K> ? never : K }[keyof T];

// SIMPLER: Just define the type you need
interface ProductUpdate {
  name?: string;
  price?: number;
}
```
- Complex mapped types and conditional types make code hard to read and debug
- Use them when they genuinely reduce duplication across many types
- If the type utility is used once, inline the result type instead
- TypeScript's built-in utility types (`Partial`, `Pick`, `Omit`, `Record`) cover most needs

---

## Dead Code Detection

### Unused Exports
- Search for import references of each export — if nothing imports it, it is dead code
- Barrel files (`index.ts`) that re-export unused modules
- Exported helper functions that were only needed during development
- **Tools**: `ts-prune`, `knip`, ESLint `no-unused-vars` with `"vars": "all"`
- Unused exports in shared libraries are harder to detect — check all consumers

### Unreachable Branches
```typescript
// Dead code: condition is always false
if (typeof window === 'undefined') {
  // This never runs in Electron renderer
  setupServerRendering();
}

// Dead code: early return makes else unreachable
function validate(input: string): boolean {
  if (!input) return false;
  if (input.length > 100) return false;
  return true;
  // Everything below is unreachable
  console.log('validated');
}
```
- Code after unconditional `return`, `throw`, `break`, or `continue`
- Conditions that are always true or always false based on types
- Switch cases that cannot be reached due to type narrowing
- Feature flags that have been permanently on/off for months

### Commented-Out Code Blocks
- Commented-out code is not documentation — it is noise
- If the code might be needed later, git history preserves it
- Comments explaining WHY code was removed are useful; the code itself is not
- **Rule**: Delete commented-out code. If someone needs it, they can check git blame.

### TODO / FIXME Never Addressed
- TODOs older than 3 months are likely abandoned
- FIXMEs that describe known bugs but were never fixed
- HACK comments marking temporary solutions that became permanent
- **Action**: Either address them now or create a tracking issue and remove the comment
- Search pattern: `// TODO`, `// FIXME`, `// HACK`, `// XXX`, `// TEMP`

### Feature Flags Never Removed
- Feature flag added for a gradual rollout, rollout completed months ago
- The flag is always `true` in all environments — the conditional is dead code
- Both branches of the flag still exist even though one will never execute
- **Rule**: When a feature is fully rolled out, remove the flag and the old code path

---

## Simplification Opportunities

### Extract Magic Numbers to Constants
```typescript
// BAD: What do these numbers mean?
if (quantity > 144) {
  discount = price * 0.15;
} else if (quantity > 48) {
  discount = price * 0.10;
}
setTimeout(retry, 30000);

// GOOD: Self-documenting constants
const BULK_QUANTITY_THRESHOLD = 144;
const CASE_QUANTITY_THRESHOLD = 48;
const BULK_DISCOUNT_RATE = 0.15;
const CASE_DISCOUNT_RATE = 0.10;
const RETRY_DELAY_MS = 30_000;

if (quantity > BULK_QUANTITY_THRESHOLD) {
  discount = price * BULK_DISCOUNT_RATE;
} else if (quantity > CASE_QUANTITY_THRESHOLD) {
  discount = price * CASE_DISCOUNT_RATE;
}
setTimeout(retry, RETRY_DELAY_MS);
```
- Any number whose meaning is not immediately obvious from context
- Exception: `0`, `1`, `-1`, `100` (percentage) are usually self-evident
- Numeric constants should include units in the name: `_MS`, `_SECONDS`, `_PIXELS`, `_PERCENT`
- String constants too: status codes, error messages, channel names

### Replace Complex Boolean Expressions with Named Functions
```typescript
// BAD: What does this condition mean?
if (product.quantity > 0 && product.status === 'active' && !product.discontinued && product.price > 0) {
  showProduct(product);
}

// GOOD: Named function explains intent
function isAvailableForSale(product: Product): boolean {
  return product.quantity > 0
    && product.status === 'active'
    && !product.discontinued
    && product.price > 0;
}

if (isAvailableForSale(product)) {
  showProduct(product);
}
```
- Any boolean expression with 3+ conditions should be extracted
- The function name should describe the BUSINESS meaning, not the code logic
- Predicate functions are reusable and testable in isolation

### Collapse Nested Ternaries
```typescript
// BAD: Nested ternaries are unreadable
const label = status === 'active' ? 'Active' : status === 'pending' ? 'Pending' : status === 'expired' ? 'Expired' : 'Unknown';

// GOOD: Object lookup
const STATUS_LABELS: Record<string, string> = {
  active: 'Active',
  pending: 'Pending',
  expired: 'Expired',
};
const label = STATUS_LABELS[status] ?? 'Unknown';

// ALSO GOOD: Simple if/else for 2-3 cases
// Single ternary is fine: const label = isActive ? 'Active' : 'Inactive';
```
- One ternary is fine. Two nested ternaries are borderline. Three or more are unreadable.
- Replace with object lookup, switch, or if/else chain
- Ternaries in JSX are acceptable for simple conditional rendering (one level only)

### Simplify Guard Clauses
```typescript
// BAD: Deep nesting
function processOrder(order: Order) {
  if (order) {
    if (order.items.length > 0) {
      if (order.status === 'confirmed') {
        // Actual logic buried 3 levels deep
        calculateTotal(order);
      } else {
        throw new Error('Order not confirmed');
      }
    } else {
      throw new Error('No items');
    }
  } else {
    throw new Error('No order');
  }
}

// GOOD: Guard clauses flatten the logic
function processOrder(order: Order) {
  if (!order) throw new Error('No order');
  if (order.items.length === 0) throw new Error('No items');
  if (order.status !== 'confirmed') throw new Error('Order not confirmed');

  // Happy path at the top level
  calculateTotal(order);
}
```
- Guard clauses eliminate nesting by handling error cases first
- The "happy path" should be the least indented code
- Each guard clause handles one concern and exits early
- Makes the function's requirements immediately visible at the top

### Reduce Function Length
- Functions longer than 30-40 lines often do too much
- If a function needs a comment to explain a section, that section could be a separate function
- Look for natural boundaries: data validation, transformation, side effects
- Extract loops with complex bodies into named functions
- Exception: Long switch/match statements with simple cases are acceptable

### Eliminate Redundant State
```typescript
// BAD: Derived state stored separately
const [items, setItems] = useState<Item[]>([]);
const [itemCount, setItemCount] = useState(0);
const [totalPrice, setTotalPrice] = useState(0);
// Must keep itemCount and totalPrice in sync with items manually

// GOOD: Derive from source of truth
const [items, setItems] = useState<Item[]>([]);
const itemCount = items.length;
const totalPrice = items.reduce((sum, item) => sum + item.price, 0);
// Or useMemo if the derivation is expensive
const totalPrice = useMemo(() => items.reduce((sum, item) => sum + item.price, 0), [items]);
```
- State that can be computed from other state should not be stored separately
- Storing derived state creates synchronization bugs
- Use `useMemo` for expensive derivations, plain computation for cheap ones
- In Zustand: use selectors for derived values, not additional state fields
