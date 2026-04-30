# Design Pattern Catalog — Complete Reference

## How to Use This Catalog

Each pattern entry includes:
- **Problem**: What situation calls for this pattern
- **When to Use**: Specific triggers that indicate this pattern
- **When NOT to Use**: Common misapplication scenarios
- **React/TypeScript Implementation**: Idiomatic example
- **Flutter/Dart Implementation**: Idiomatic example
- **Common Mistakes**: Pitfalls to avoid

---

## Creational Patterns

### Factory

**Problem:** Creating objects whose exact type depends on runtime conditions.

**When to Use:**
- Object creation logic is complex or involves configuration
- You need to create different types based on input
- Construction details should be hidden from the caller

**When NOT to Use:**
- Only one type is ever created (just use `new` or a constructor)
- The creation logic is trivial (2-3 lines)

**React/TypeScript:**
```typescript
type ChartType = 'bar' | 'line' | 'pie';

function createChart(type: ChartType, data: ChartData): React.ReactNode {
  switch (type) {
    case 'bar':  return <BarChart data={data} />;
    case 'line': return <LineChart data={data} />;
    case 'pie':  return <PieChart data={data} />;
    default:     throw new Error(`Unknown chart type: ${type}`);
  }
}
```

**Flutter/Dart:**
```dart
Widget createChart(ChartType type, ChartData data) {
  switch (type) {
    case ChartType.bar:  return BarChart(data: data);
    case ChartType.line: return LineChart(data: data);
    case ChartType.pie:  return PieChart(data: data);
  }
}
```

**Common Mistakes:**
- Over-engineering: creating a factory for a single type
- Forgetting exhaustiveness checks in switch statements
- Not using TypeScript discriminated unions for type safety

---

### Builder

**Problem:** Constructing complex objects step by step, where the construction
process must allow different representations.

**When to Use:**
- Object has many optional parameters (>4)
- Construction involves multiple steps that can vary
- Same construction process should create different representations

**When NOT to Use:**
- Object is simple with few required fields
- All parameters are required (just use a constructor)

**React/TypeScript:**
```typescript
class QueryBuilder {
  private query: QueryConfig = { table: '', conditions: [], limit: 100 };

  from(table: string): this { this.query.table = table; return this; }
  where(condition: Condition): this { this.query.conditions.push(condition); return this; }
  limit(n: number): this { this.query.limit = n; return this; }
  orderBy(field: string, dir: 'asc' | 'desc' = 'asc'): this {
    this.query.orderBy = { field, dir }; return this;
  }
  build(): QueryConfig { return { ...this.query }; }
}

// Usage
const query = new QueryBuilder()
  .from('products')
  .where({ field: 'price', op: '>', value: 10 })
  .orderBy('name')
  .limit(50)
  .build();
```

**Flutter/Dart:**
```dart
class NotificationBuilder {
  String _title = '';
  String _body = '';
  NotificationPriority _priority = NotificationPriority.normal;

  NotificationBuilder title(String t) { _title = t; return this; }
  NotificationBuilder body(String b) { _body = b; return this; }
  NotificationBuilder priority(NotificationPriority p) { _priority = p; return this; }

  AppNotification build() => AppNotification(
    title: _title, body: _body, priority: _priority,
  );
}
```

**Common Mistakes:**
- Not returning `this` for chaining
- Mutable builder that can be reused accidentally (add a `build` that clones)
- Missing required field validation in `build()`

---

### Singleton

**Problem:** Ensuring a class has only one instance and providing global access.

**When to Use:**
- Exactly one instance must exist (database connection pool, app config)
- Global access point is genuinely needed

**When NOT to Use:**
- You just want shared state (use dependency injection or module scope)
- Testing requires different instances (singletons make testing harder)
- Multiple instances might be needed in the future

**React/TypeScript:**
```typescript
// Module-scoped singleton (preferred in TypeScript)
class DatabaseService {
  private static instance: DatabaseService | null = null;
  private constructor(private db: Database) {}

  static getInstance(): DatabaseService {
    if (!DatabaseService.instance) {
      DatabaseService.instance = new DatabaseService(new Database());
    }
    return DatabaseService.instance;
  }
}

// Even simpler: module scope
let dbInstance: Database | null = null;
export function getDatabase(): Database {
  if (!dbInstance) dbInstance = new Database();
  return dbInstance;
}
```

**Flutter/Dart:**
```dart
class AppConfig {
  static final AppConfig _instance = AppConfig._internal();
  factory AppConfig() => _instance;
  AppConfig._internal();

  late final String apiUrl;
  late final String appVersion;
}
```

**Common Mistakes:**
- Using singleton when dependency injection would be better
- Making testing difficult (no way to inject mock)
- Thread safety issues in multi-threaded environments

---

## Structural Patterns

### Adapter

**Problem:** Making incompatible interfaces work together.

**When to Use:**
- Integrating a third-party library with a different interface
- Migrating from one API to another
- Wrapping legacy code to match new interfaces

**When NOT to Use:**
- Interfaces are already compatible
- You control both sides and can change the interface

**React/TypeScript:**
```typescript
// Adapting a third-party chart library to your data format
interface InternalData { label: string; value: number; color: string; }
interface ChartJsData { labels: string[]; datasets: { data: number[]; backgroundColor: string[] }[] }

function adaptToChartJs(data: InternalData[]): ChartJsData {
  return {
    labels: data.map(d => d.label),
    datasets: [{
      data: data.map(d => d.value),
      backgroundColor: data.map(d => d.color),
    }],
  };
}
```

**Flutter/Dart:**
```dart
// Adapting a REST API response to your domain model
class UserAdapter {
  static User fromApiResponse(Map<String, dynamic> json) {
    return User(
      id: json['user_id'] as String,
      name: '${json["first_name"]} ${json["last_name"]}',
      email: json['email_address'] as String,
    );
  }
}
```

**Common Mistakes:**
- Adapting too many things (indicates a design problem upstream)
- Losing type safety in the adaptation layer
- Not handling edge cases from the source format

---

### Decorator

**Problem:** Adding responsibilities to objects dynamically without modifying
their class.

**When to Use:**
- Need to add behavior to individual objects, not the whole class
- Adding behavior through subclassing would cause an explosion of subclasses
- Responsibilities can be combined independently

**When NOT to Use:**
- The combination of decorators is always the same (just build it in)
- Performance-critical paths where the indirection matters

**React/TypeScript (HOC Pattern):**
```typescript
// Decorator as a Higher-Order Component
function withLoading<P extends object>(
  Component: React.ComponentType<P>
): React.FC<P & { isLoading: boolean }> {
  return function WithLoading({ isLoading, ...props }) {
    if (isLoading) return <LoadingSpinner />;
    return <Component {...(props as P)} />;
  };
}

// Decorator as a custom hook
function useWithRetry<T>(
  fn: () => Promise<T>,
  maxRetries = 3
): { execute: () => Promise<T>; attempts: number } {
  const [attempts, setAttempts] = useState(0);
  const execute = async () => {
    for (let i = 0; i <= maxRetries; i++) {
      try {
        setAttempts(i + 1);
        return await fn();
      } catch (e) {
        if (i === maxRetries) throw e;
        await new Promise(r => setTimeout(r, 1000 * Math.pow(2, i)));
      }
    }
    throw new Error('Unreachable');
  };
  return { execute, attempts };
}
```

**Flutter/Dart:**
```dart
// Widget decoration (composition)
class BorderedCard extends StatelessWidget {
  final Widget child;
  final Color borderColor;
  const BorderedCard({required this.child, this.borderColor = Colors.grey});

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        border: Border.all(color: borderColor, width: 2),
        borderRadius: BorderRadius.circular(12),
      ),
      child: child,
    );
  }
}
```

**Common Mistakes:**
- Too many layers of decoration (debugging becomes painful)
- Decorator changes the interface (it should be transparent)
- Not considering the order of decoration (it matters)

---

## Behavioral Patterns

### Observer

**Problem:** When one object changes state, all its dependents are notified and
updated automatically.

**When to Use:**
- Multiple components need to react to state changes
- The set of observers is dynamic (can change at runtime)
- Loose coupling between the state owner and its consumers

**When NOT to Use:**
- Only one consumer exists (direct callback is simpler)
- Updates must be ordered (observer doesn't guarantee order)

**React/TypeScript:**
```typescript
// Zustand store (built-in observer pattern)
const useStore = create<AppState>((set) => ({
  count: 0,
  increment: () => set((s) => ({ count: s.count + 1 })),
}));

// Each component is an observer with selective subscription
function Counter() {
  const count = useStore((s) => s.count);
  return <span>{count}</span>;
}

// Manual observer with EventEmitter
class AppEventBus {
  private listeners = new Map<string, Set<Function>>();
  on(event: string, fn: Function) {
    if (!this.listeners.has(event)) this.listeners.set(event, new Set());
    this.listeners.get(event)!.add(fn);
    return () => this.listeners.get(event)?.delete(fn);
  }
  emit(event: string, data?: unknown) {
    this.listeners.get(event)?.forEach(fn => fn(data));
  }
}
```

**Flutter/Dart:**
```dart
// StreamBuilder (reactive observer)
StreamBuilder<int>(
  stream: counterStream,
  builder: (context, snapshot) {
    if (!snapshot.hasData) return CircularProgressIndicator();
    return Text('${snapshot.data}');
  },
)

// ValueNotifier + ValueListenableBuilder
final counter = ValueNotifier<int>(0);

ValueListenableBuilder<int>(
  valueListenable: counter,
  builder: (context, value, child) => Text('$value'),
)
```

**Common Mistakes:**
- Memory leaks from not unsubscribing (always clean up in useEffect return or dispose)
- Infinite loops from observer triggering the observed state
- Performance issues from notifying on every change (use selectors)

---

### Strategy

**Problem:** Define a family of algorithms, encapsulate each one, and make them
interchangeable at runtime.

**When to Use:**
- Multiple algorithms for the same task
- Algorithm selection depends on runtime conditions
- Need to switch algorithms without changing client code

**When NOT to Use:**
- Only one algorithm will ever be used
- The algorithms are trivially different (use a parameter instead)

**React/TypeScript:**
```typescript
// Sorting strategies
type SortStrategy<T> = (items: T[]) => T[];

const sortByName: SortStrategy<Product> = (items) =>
  [...items].sort((a, b) => a.name.localeCompare(b.name));

const sortByPrice: SortStrategy<Product> = (items) =>
  [...items].sort((a, b) => a.price - b.price);

const sortByDate: SortStrategy<Product> = (items) =>
  [...items].sort((a, b) => b.createdAt.getTime() - a.createdAt.getTime());

// Usage in component
function ProductList({ sortBy }: { sortBy: 'name' | 'price' | 'date' }) {
  const strategies: Record<string, SortStrategy<Product>> = {
    name: sortByName, price: sortByPrice, date: sortByDate,
  };
  const sorted = strategies[sortBy](products);
  return <ul>{sorted.map(p => <li key={p.id}>{p.name}</li>)}</ul>;
}
```

**Flutter/Dart:**
```dart
typedef PricingStrategy = double Function(double basePrice, int quantity);

double standardPricing(double base, int qty) => base * qty;
double bulkPricing(double base, int qty) => qty >= 10 ? base * qty * 0.9 : base * qty;
double premiumPricing(double base, int qty) => base * qty * 1.2;

class OrderCalculator {
  final PricingStrategy strategy;
  OrderCalculator(this.strategy);
  double calculate(double price, int quantity) => strategy(price, quantity);
}
```

**Common Mistakes:**
- Creating a strategy interface for two simple cases (overkill)
- Not making strategies stateless (strategies should be pure functions when possible)
- Hardcoding strategy selection instead of making it configurable

---

### Command

**Problem:** Encapsulate a request as an object, allowing parameterization,
queueing, logging, and undo/redo operations.

**When to Use:**
- Need undo/redo functionality
- Need to queue or schedule operations
- Need to log all operations for audit
- Need to support macro operations (batch commands)

**When NOT to Use:**
- Simple one-off operations with no undo
- Performance-critical paths where command overhead matters

**React/TypeScript:**
```typescript
interface Command {
  execute(): void;
  undo(): void;
  description: string;
}

class AddItemCommand implements Command {
  description: string;
  constructor(private store: InventoryStore, private item: Item) {
    this.description = `Add ${item.name}`;
  }
  execute() { this.store.addItem(this.item); }
  undo() { this.store.removeItem(this.item.id); }
}

class CommandHistory {
  private history: Command[] = [];
  private position = -1;

  execute(cmd: Command) {
    this.history = this.history.slice(0, this.position + 1);
    cmd.execute();
    this.history.push(cmd);
    this.position++;
  }
  undo() {
    if (this.position < 0) return;
    this.history[this.position].undo();
    this.position--;
  }
  redo() {
    if (this.position >= this.history.length - 1) return;
    this.position++;
    this.history[this.position].execute();
  }
}
```

**Flutter/Dart:**
```dart
abstract class Command {
  void execute();
  void undo();
  String get description;
}

class ChangeColorCommand implements Command {
  final CanvasState canvas;
  final Color newColor;
  late final Color oldColor;

  ChangeColorCommand(this.canvas, this.newColor);

  @override String get description => 'Change color to $newColor';
  @override void execute() { oldColor = canvas.color; canvas.color = newColor; }
  @override void undo() { canvas.color = oldColor; }
}
```

---

### Chain of Responsibility

**Problem:** Pass a request along a chain of handlers. Each handler decides
whether to process the request or pass it to the next handler.

**When to Use:**
- Multiple handlers can process a request
- Handler order matters
- Set of handlers is dynamic

**When NOT to Use:**
- Only one handler will ever handle the request (just call it directly)
- All handlers must process the request (that's a pipeline, not a chain)

**React/TypeScript:**
```typescript
type Middleware<T> = (data: T, next: () => void) => void;

function createChain<T>(...middlewares: Middleware<T>[]) {
  return (data: T) => {
    let index = 0;
    const next = () => {
      if (index < middlewares.length) {
        const middleware = middlewares[index++];
        middleware(data, next);
      }
    };
    next();
  };
}

// Usage: validation chain
const validateOrder = createChain<Order>(
  (order, next) => { if (!order.items.length) throw new Error('Empty order'); next(); },
  (order, next) => { if (order.total < 0) throw new Error('Negative total'); next(); },
  (order, next) => { if (!order.customerId) throw new Error('No customer'); next(); },
);
```

---

### State Machine

**Problem:** An object's behavior changes based on its internal state, and
transitions between states follow defined rules.

**When to Use:**
- Object has clear, finite states
- Transitions between states have rules and guards
- Invalid state transitions should be prevented
- State history or logging is needed

**When NOT to Use:**
- Only 2 states (a boolean flag is fine)
- State transitions have no rules (just use a variable)

**React/TypeScript:**
```typescript
type OrderState = 'draft' | 'submitted' | 'processing' | 'shipped' | 'delivered' | 'cancelled';

const transitions: Record<OrderState, OrderState[]> = {
  draft:      ['submitted', 'cancelled'],
  submitted:  ['processing', 'cancelled'],
  processing: ['shipped', 'cancelled'],
  shipped:    ['delivered'],
  delivered:  [],
  cancelled:  [],
};

function transitionOrder(current: OrderState, target: OrderState): OrderState {
  if (!transitions[current].includes(target)) {
    throw new Error(`Invalid transition: ${current} → ${target}`);
  }
  return target;
}
```

---

### Mediator

**Problem:** Reduce chaotic dependencies between objects by introducing a
mediator object that handles communication.

**When to Use:**
- Many components need to communicate with each other
- Direct references between components create tight coupling
- Communication patterns change frequently

**When NOT to Use:**
- Only 2 components communicate (direct reference is fine)
- The mediator becomes a god object (split into multiple mediators)

**React/TypeScript:**
```typescript
// Event bus as mediator
type EventHandler = (data: unknown) => void;

class AppMediator {
  private handlers = new Map<string, Set<EventHandler>>();

  register(event: string, handler: EventHandler): () => void {
    if (!this.handlers.has(event)) this.handlers.set(event, new Set());
    this.handlers.get(event)!.add(handler);
    return () => this.handlers.get(event)?.delete(handler);
  }

  notify(event: string, data?: unknown): void {
    this.handlers.get(event)?.forEach(h => h(data));
  }
}

// Components communicate through mediator, not directly
// ProductList: mediator.notify('product-selected', product)
// ProductDetail: mediator.register('product-selected', (p) => setProduct(p))
```

---

## Architectural Patterns

### Repository

**Problem:** Mediate between the domain and data mapping layers using a
collection-like interface for accessing domain objects.

**When to Use:**
- Abstracting database access from business logic
- Supporting multiple data sources (SQLite for local, API for cloud)
- Testing business logic without a real database

**When NOT to Use:**
- Trivial CRUD with no business logic
- Single data source that will never change

**React/TypeScript:**
```typescript
interface ProductRepository {
  findAll(): Promise<Product[]>;
  findById(id: string): Promise<Product | null>;
  save(product: Product): Promise<void>;
  delete(id: string): Promise<void>;
  findByCategory(category: string): Promise<Product[]>;
}

class SqliteProductRepository implements ProductRepository {
  constructor(private db: Database) {}
  async findAll() { return this.db.all<Product>('SELECT * FROM products'); }
  async findById(id: string) { return this.db.get<Product>('SELECT * FROM products WHERE id = ?', [id]); }
  async save(p: Product) { await this.db.run('INSERT OR REPLACE INTO products VALUES (?, ?, ?)', [p.id, p.name, p.price]); }
  async delete(id: string) { await this.db.run('DELETE FROM products WHERE id = ?', [id]); }
  async findByCategory(cat: string) { return this.db.all<Product>('SELECT * FROM products WHERE category = ?', [cat]); }
}
```

---

### Middleware / Pipeline

**Problem:** Process a request through a series of steps, where each step can
modify the request, short-circuit, or pass to the next step.

**When to Use:**
- Cross-cutting concerns (logging, auth, validation, error handling)
- Request/response processing pipelines
- Plugin systems

**When NOT to Use:**
- Linear processing with no branching (just call functions in sequence)
- Steps don't share a common interface

**React/TypeScript:**
```typescript
type Next = () => Promise<void>;
type Middleware = (ctx: RequestContext, next: Next) => Promise<void>;

async function runMiddleware(ctx: RequestContext, middlewares: Middleware[]) {
  let index = 0;
  const next: Next = async () => {
    if (index < middlewares.length) {
      const mw = middlewares[index++];
      await mw(ctx, next);
    }
  };
  await next();
}

// Example middlewares
const logger: Middleware = async (ctx, next) => {
  console.log(`→ ${ctx.method} ${ctx.path}`);
  const start = Date.now();
  await next();
  console.log(`← ${ctx.status} (${Date.now() - start}ms)`);
};

const auth: Middleware = async (ctx, next) => {
  if (!ctx.headers.authorization) { ctx.status = 401; return; }
  ctx.user = verifyToken(ctx.headers.authorization);
  await next();
};
```

---

### Compound Components

**Problem:** A set of components designed to work together, sharing implicit
state while giving the consumer full control over rendering.

**When to Use:**
- Component group that shares state (Tabs, Accordion, Dropdown)
- Consumer needs control over layout and rendering
- Internal state should be hidden from the consumer

**When NOT to Use:**
- Single-purpose component with no variants
- Components don't share state

**React/TypeScript:**
```typescript
// Compound component pattern
const TabsContext = createContext<{ active: string; setActive: (id: string) => void } | null>(null);

function Tabs({ children, defaultTab }: { children: React.ReactNode; defaultTab: string }) {
  const [active, setActive] = useState(defaultTab);
  return <TabsContext.Provider value={{ active, setActive }}>{children}</TabsContext.Provider>;
}

function TabList({ children }: { children: React.ReactNode }) {
  return <div role="tablist" className="flex gap-2">{children}</div>;
}

function Tab({ id, children }: { id: string; children: React.ReactNode }) {
  const ctx = useContext(TabsContext)!;
  return (
    <button role="tab" aria-selected={ctx.active === id} onClick={() => ctx.setActive(id)}>
      {children}
    </button>
  );
}

function TabPanel({ id, children }: { id: string; children: React.ReactNode }) {
  const ctx = useContext(TabsContext)!;
  if (ctx.active !== id) return null;
  return <div role="tabpanel">{children}</div>;
}

// Usage — consumer controls layout
<Tabs defaultTab="inventory">
  <TabList>
    <Tab id="inventory">Inventory</Tab>
    <Tab id="sales">Sales</Tab>
  </TabList>
  <TabPanel id="inventory"><InventoryView /></TabPanel>
  <TabPanel id="sales"><SalesView /></TabPanel>
</Tabs>
```

---

### Provider

**Problem:** Share state or services across a component tree without prop drilling.

**When to Use:**
- State needed by many components at different tree depths
- Service instances (database, auth, theme) shared across the app
- Avoiding prop drilling beyond 2 levels

**When NOT to Use:**
- State is local to a single component or parent-child pair
- Frequent updates would cause unnecessary re-renders across the tree

**React/TypeScript:**
```typescript
// Theme provider
interface Theme { mode: 'light' | 'dark'; primary: string; }
const ThemeContext = createContext<Theme>({ mode: 'light', primary: '#3b82f6' });

function ThemeProvider({ children }: { children: React.ReactNode }) {
  const [theme, setTheme] = useState<Theme>({ mode: 'light', primary: '#3b82f6' });
  return (
    <ThemeContext.Provider value={theme}>
      {children}
    </ThemeContext.Provider>
  );
}

function useTheme() {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error('useTheme must be used within ThemeProvider');
  return ctx;
}
```

---

## Pattern Selection Cheat Sheet

| I need to... | Consider these patterns |
|---|---|
| Create objects conditionally | Factory |
| Build complex objects step by step | Builder |
| Ensure single instance | Singleton (or module scope) |
| Adapt incompatible interfaces | Adapter |
| Add behavior without subclassing | Decorator / HOC |
| React to state changes | Observer / Pub-Sub |
| Choose algorithm at runtime | Strategy |
| Support undo/redo | Command |
| Process through pipeline | Chain / Middleware |
| Manage finite states | State Machine |
| Decouple many-to-many communication | Mediator |
| Abstract data access | Repository |
| Share state across tree | Provider / Context |
| Compose related components | Compound Components |
