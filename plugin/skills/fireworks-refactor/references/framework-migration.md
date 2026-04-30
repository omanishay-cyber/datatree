# Framework Migration Reference

> The number one rule of framework migration: NEVER big-bang. Run old and new in parallel, migrate one piece at a time, verify after every step.

---

## 1. The Golden Rule

```
WRONG (big-bang):
  Rewrite the entire application from Framework A to Framework B
  in a single branch over 3 weeks. Merge it all at once. Pray.

RIGHT (incremental):
  Keep old and new running side by side. Migrate one component
  per commit. Verify after each migration. Roll back instantly
  if anything breaks. Remove old code only after new is verified.
```

Big-bang rewrites fail because:
- You cannot test the new system until it is 100% complete
- Bugs compound -- by the time you find them, you cannot isolate them
- The old system keeps changing while you rewrite -- you are chasing a moving target
- Morale collapses because there is no visible progress for weeks

---

## 2. The Adapter Pattern

Create a thin adapter layer between your code and the framework. This decouples migration from feature work.

### Before Migration

```typescript
// Your code calls Framework A directly
import { FrameworkARouter } from 'framework-a';

const router = new FrameworkARouter();
router.get('/users', getUsers);
router.post('/users', createUser);
```

### Step 1: Introduce the Adapter

```typescript
// adapter.ts — abstracts the framework
interface RouterAdapter {
  get(path: string, handler: RequestHandler): void;
  post(path: string, handler: RequestHandler): void;
}

// Framework A implementation
class FrameworkAAdapter implements RouterAdapter {
  private router = new FrameworkARouter();
  get(path: string, handler: RequestHandler) { this.router.get(path, handler); }
  post(path: string, handler: RequestHandler) { this.router.post(path, handler); }
}

// Your code uses the adapter, not the framework directly
const router: RouterAdapter = new FrameworkAAdapter();
router.get('/users', getUsers);
router.post('/users', createUser);
```

### Step 2: Add Framework B Implementation

```typescript
class FrameworkBAdapter implements RouterAdapter {
  private app = new FrameworkBApp();
  get(path: string, handler: RequestHandler) { this.app.route('GET', path, handler); }
  post(path: string, handler: RequestHandler) { this.app.route('POST', path, handler); }
}
```

### Step 3: Switch Implementations

```typescript
// Toggle between old and new with a single line change
const router: RouterAdapter = useNewFramework
  ? new FrameworkBAdapter()
  : new FrameworkAAdapter();
```

---

## 3. Feature Flags

Use feature flags to control which implementation runs. This enables gradual rollout and instant rollback.

### Simple Feature Flag

```typescript
// feature-flags.ts
const flags = {
  useNewRouter: process.env.USE_NEW_ROUTER === 'true',
  useNewAuth: process.env.USE_NEW_AUTH === 'true',
  useNewDatabase: process.env.USE_NEW_DATABASE === 'true',
};

export function isEnabled(flag: keyof typeof flags): boolean {
  return flags[flag] ?? false;
}
```

### Usage in Code

```typescript
import { isEnabled } from './feature-flags';

function getRouter(): RouterAdapter {
  if (isEnabled('useNewRouter')) {
    return new FrameworkBAdapter();
  }
  return new FrameworkAAdapter();
}
```

### Rollback Plan

```
If the new implementation has a bug:
  1. Set the feature flag to false
  2. Restart the application
  3. Old implementation is back instantly — no code changes needed
  4. Fix the bug in the new implementation
  5. Re-enable the flag when ready
```

---

## 4. Migration Checklist

### Phase 1: Inventory

```
[ ] List EVERY usage of the old framework/pattern in the codebase
    Command: grep -rn "import.*from.*old-framework" src/
[ ] Categorize by type: components, utilities, hooks, services
[ ] Estimate complexity: simple (1 hour), medium (half day), hard (full day)
[ ] Identify dependencies between usages (migration order matters)
[ ] Total count: N files to migrate
```

### Phase 2: Adapter Layer

```
[ ] Design the adapter interface (what operations does your code need?)
[ ] Implement the adapter for the OLD framework (verify existing behavior)
[ ] Implement the adapter for the NEW framework
[ ] Add a feature flag to toggle between old and new
[ ] Write tests for both adapter implementations
[ ] Commit: "refactor: introduce adapter layer for <framework>"
```

### Phase 3: Migrate One File

```
[ ] Pick the simplest, most isolated file from the inventory
[ ] Migrate it to use the adapter (or new framework directly)
[ ] Run tsc --noEmit
[ ] Run test suite
[ ] Manual smoke test
[ ] Commit: "refactor: migrate <filename> to <new framework>"
```

### Phase 4: Migrate Everything (Repeat Phase 3)

```
[ ] Migrate files one at a time, simplest first
[ ] Each migration is its own commit
[ ] Track progress: X of N files migrated
[ ] If a migration breaks tests, revert and try a different approach
```

### Phase 5: Cleanup

```
[ ] All files now use the new framework
[ ] Remove the old framework adapter implementation
[ ] Remove the feature flag (hardcode to new)
[ ] Remove the old framework from package.json
[ ] Run tsc --noEmit — clean
[ ] Run full test suite — all pass
[ ] Run the app — manual smoke test passes
[ ] Commit: "refactor: complete migration to <new framework>, remove <old>"
```

---

## 5. Common Migration Patterns

### Class Components to Functional Components (React)

```typescript
// BEFORE: class component
class UserList extends React.Component<Props, State> {
  state = { users: [], loading: true };

  componentDidMount() {
    fetchUsers().then(users => this.setState({ users, loading: false }));
  }

  render() {
    if (this.state.loading) return <Spinner />;
    return <ul>{this.state.users.map(u => <li key={u.id}>{u.name}</li>)}</ul>;
  }
}

// AFTER: functional component
function UserList({ initialFilter }: Props) {
  const [users, setUsers] = useState<User[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetchUsers().then(data => {
      setUsers(data);
      setLoading(false);
    });
  }, []);

  if (loading) return <Spinner />;
  return <ul>{users.map(u => <li key={u.id}>{u.name}</li>)}</ul>;
}
```

### Callbacks to Async/Await

```typescript
// BEFORE: callback hell
function loadData(callback: (err: Error | null, data: Data | null) => void) {
  fetchUsers((err, users) => {
    if (err) return callback(err, null);
    fetchOrders(users, (err2, orders) => {
      if (err2) return callback(err2, null);
      callback(null, { users, orders });
    });
  });
}

// AFTER: async/await
async function loadData(): Promise<Data> {
  const users = await fetchUsers();
  const orders = await fetchOrders(users);
  return { users, orders };
}
```

### `any` to Typed

```typescript
// BEFORE: untyped
function process(data: any): any {
  return { result: data.value * 2, label: data.name };
}

// AFTER: typed
interface ProcessInput { value: number; name: string; }
interface ProcessOutput { result: number; label: string; }

function process(data: ProcessInput): ProcessOutput {
  return { result: data.value * 2, label: data.name };
}
```

### Lodash to Native JavaScript

```typescript
// BEFORE: lodash
import _ from 'lodash';
const unique = _.uniq(arr);
const grouped = _.groupBy(items, 'category');
const picked = _.pick(obj, ['name', 'email']);
const flat = _.flatten(nested);

// AFTER: native
const unique = [...new Set(arr)];
const grouped = Object.groupBy(items, item => item.category);
const { name, email } = obj; const picked = { name, email };
const flat = nested.flat();
```

### MobX/Redux to Zustand

```typescript
// BEFORE: Redux
const store = createStore(rootReducer);
// dispatch(action), connect(mapState, mapDispatch)(Component), selectors...

// AFTER: Zustand
interface StoreState {
  users: User[];
  loading: boolean;
  fetchUsers: () => Promise<void>;
}

const useStore = create<StoreState>((set) => ({
  users: [],
  loading: false,
  fetchUsers: async () => {
    set({ loading: true });
    const users = await api.getUsers();
    set({ users, loading: false });
  },
}));

// In component:
function UserList() {
  const { users, loading, fetchUsers } = useStore();
  // ...
}
```

---

## 6. Warning Signs During Migration

```
STOP and reassess if:
  - You are modifying more than 5 files for a single migration step
    -> Break it into smaller steps
  - Tests are failing and you do not understand why
    -> Revert and add characterization tests first
  - The adapter layer is growing more complex than the frameworks themselves
    -> Simplify the adapter or accept a thinner abstraction
  - You discover the old and new frameworks have fundamentally different models
    -> Rewrite may actually be needed, but scope it to one module at a time
  - Migration has been in progress for more than 2 weeks with no end in sight
    -> Reassess scope, consider migrating only the most critical paths
```
