# Error Recovery Reference

> Patterns for handling errors gracefully: error boundaries, IPC error propagation,
> retry strategies, graceful degradation, recovery actions, user-facing errors, and logging.

---

## 1. Error Boundary Pattern

React Error Boundaries catch JavaScript errors in the component tree and display a fallback UI instead of crashing the entire application.

### Implementation
```typescript
import { Component, ErrorInfo, ReactNode } from 'react';

interface ErrorBoundaryProps {
  children: ReactNode;
  fallback?: ReactNode;
  onError?: (error: Error, errorInfo: ErrorInfo) => void;
  onRetry?: () => void;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo): void {
    console.error('[ErrorBoundary] Caught error:', error, errorInfo);
    this.props.onError?.(error, errorInfo);
  }

  handleRetry = (): void => {
    this.setState({ hasError: false, error: null });
    this.props.onRetry?.();
  };

  render(): ReactNode {
    if (this.state.hasError) {
      if (this.props.fallback) return this.props.fallback;
      return (
        <div className="p-6 text-center">
          <h2 className="text-lg font-semibold text-red-500">Something went wrong</h2>
          <p className="text-sm text-gray-500 mt-2">{this.state.error?.message}</p>
          <button onClick={this.handleRetry} className="mt-4 px-4 py-2 bg-blue-500 text-white rounded">
            Try Again
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
```

### Usage
```typescript
// Wrap every route-level component:
<ErrorBoundary onError={(error) => logToService(error)}>
  <ProductsPage />
</ErrorBoundary>

// With custom fallback:
<ErrorBoundary fallback={<EmptyState message="Failed to load products" />}>
  <ProductList />
</ErrorBoundary>
```

### Key Rules
- Place Error Boundaries at route level — every page should have one.
- Error Boundaries do NOT catch: event handlers, async code, server-side rendering, or errors in the boundary itself.
- For event handler errors, use try/catch directly.
- For async errors, use `.catch()` or try/catch in async functions.

---

## 2. IPC Error Propagation

Errors in the main process IPC handler must be explicitly propagated to the renderer. If the handler throws and the error is not caught, the renderer's `invoke()` call will reject with a generic error that loses the original message.

### Structured Error Response Pattern
```typescript
// Types shared between main and renderer:
interface IpcResponse<T> {
  success: true;
  data: T;
}

interface IpcError {
  success: false;
  error: string;
  code?: string;
}

type IpcResult<T> = IpcResponse<T> | IpcError;

// Main process handler:
ipcMain.handle('get-products', async (event, category: string): Promise<IpcResult<Product[]>> => {
  try {
    const products = await db.getProductsByCategory(category);
    return { success: true, data: products };
  } catch (error) {
    console.error('[IPC] get-products error:', error);
    return {
      success: false,
      error: error instanceof Error ? error.message : String(error),
      code: 'DB_QUERY_FAILED',
    };
  }
});

// Renderer-side handling:
async function fetchProducts(category: string): Promise<Product[]> {
  const result = await window.api.getProducts(category);
  if (!result.success) {
    throw new Error(`Failed to fetch products: ${result.error}`);
  }
  return result.data;
}
```

### Key Rules
- ALWAYS wrap handler logic in try/catch.
- ALWAYS return structured responses, never raw throws.
- Include an error code for programmatic handling on the renderer side.
- Log the full error on the main side (where you have context) but send only the message to the renderer.

---

## 3. Retry Strategies

### Exponential Backoff
```typescript
async function withRetry<T>(
  operation: () => Promise<T>,
  options: {
    maxRetries?: number;
    baseDelay?: number;
    maxDelay?: number;
    jitter?: boolean;
    onRetry?: (attempt: number, error: Error) => void;
  } = {}
): Promise<T> {
  const { maxRetries = 3, baseDelay = 1000, maxDelay = 10000, jitter = true, onRetry } = options;

  let lastError: Error;

  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      return await operation();
    } catch (error) {
      lastError = error instanceof Error ? error : new Error(String(error));

      if (attempt === maxRetries) break;

      let delay = Math.min(baseDelay * Math.pow(2, attempt), maxDelay);
      if (jitter) {
        delay = delay * (0.5 + Math.random() * 0.5); // Add 0-50% jitter
      }

      onRetry?.(attempt + 1, lastError);
      await new Promise((resolve) => setTimeout(resolve, delay));
    }
  }

  throw lastError!;
}
```

### Usage
```typescript
const data = await withRetry(() => fetchFromServer('/api/data'), {
  maxRetries: 3,
  baseDelay: 1000,  // 1s, 2s, 4s
  onRetry: (attempt, error) => {
    console.warn(`Retry ${attempt}: ${error.message}`);
  },
});
```

### Circuit Breaker Pattern
```typescript
class CircuitBreaker {
  private failures = 0;
  private lastFailureTime = 0;
  private state: 'closed' | 'open' | 'half-open' = 'closed';

  constructor(
    private threshold: number = 5,
    private resetTimeout: number = 30000
  ) {}

  async execute<T>(operation: () => Promise<T>): Promise<T> {
    if (this.state === 'open') {
      if (Date.now() - this.lastFailureTime > this.resetTimeout) {
        this.state = 'half-open';
      } else {
        throw new Error('Circuit breaker is open — service unavailable');
      }
    }

    try {
      const result = await operation();
      this.onSuccess();
      return result;
    } catch (error) {
      this.onFailure();
      throw error;
    }
  }

  private onSuccess(): void {
    this.failures = 0;
    this.state = 'closed';
  }

  private onFailure(): void {
    this.failures++;
    this.lastFailureTime = Date.now();
    if (this.failures >= this.threshold) {
      this.state = 'open';
    }
  }
}
```

---

## 4. Graceful Degradation

When a feature fails, the app should continue working with reduced functionality rather than crashing entirely.

### Patterns

**Offline Mode:**
```typescript
// Check connectivity before network operations:
const isOnline = navigator.onLine;

window.addEventListener('online', () => {
  syncPendingChanges(); // Sync queued changes when back online
});

window.addEventListener('offline', () => {
  showNotification('Working offline — changes will sync when connected');
});
```

**Cached Data Fallback:**
```typescript
async function getProducts(): Promise<Product[]> {
  try {
    const fresh = await api.fetchProducts();
    cache.set('products', fresh);
    return fresh;
  } catch {
    const cached = cache.get('products');
    if (cached) {
      showNotification('Showing cached data — could not reach server');
      return cached;
    }
    throw new Error('No data available — check your connection');
  }
}
```

**Feature Flags for Broken Features:**
```typescript
const FEATURES = {
  syncEnabled: true,
  reportsEnabled: true,
  exportEnabled: true,
};

// Disable a feature if it's causing issues:
if (FEATURES.syncEnabled) {
  startSync();
} else {
  showBanner('Sync is temporarily disabled for maintenance');
}
```

---

## 5. Recovery Actions

### Auto-Save on Crash
```typescript
// Save state periodically and before known crash points:
const autoSaveInterval = setInterval(() => {
  const state = store.getState();
  localStorage.setItem('autosave', JSON.stringify(state));
  localStorage.setItem('autosave-timestamp', Date.now().toString());
}, 30000); // Every 30 seconds

// On app startup, offer to restore:
const autosave = localStorage.getItem('autosave');
const timestamp = localStorage.getItem('autosave-timestamp');
if (autosave && timestamp) {
  const age = Date.now() - parseInt(timestamp);
  if (age < 3600000) { // Less than 1 hour old
    const restore = confirm('Unsaved changes detected. Restore?');
    if (restore) {
      store.setState(JSON.parse(autosave));
    }
  }
}
```

### Transaction Rollback
```typescript
// For database operations, always use transactions:
function safeUpdate(queries: string[]): void {
  db.run('BEGIN TRANSACTION');
  try {
    for (const query of queries) {
      db.run(query);
    }
    db.run('COMMIT');
  } catch (error) {
    db.run('ROLLBACK');
    console.error('Transaction rolled back:', error);
    throw error;
  }
}
```

---

## 6. User-Facing Errors

### Rules
- NEVER show stack traces to users.
- NEVER show raw error messages from libraries.
- ALWAYS provide a human-readable explanation.
- ALWAYS suggest an action the user can take.
- Use toast notifications for transient errors.
- Use inline messages for form validation errors.
- Use full-page error states for critical failures.

### Error Message Guidelines
```typescript
// BAD:
showError("TypeError: Cannot read properties of undefined (reading 'name')");

// GOOD:
showError("Could not load product details. Please try refreshing the page.");

// BAD:
showError("SQLITE_CONSTRAINT: UNIQUE constraint failed: products.sku");

// GOOD:
showError("A product with this SKU already exists. Please use a different SKU.");
```

### Toast Notification Pattern
```typescript
function showToast(message: string, type: 'success' | 'error' | 'warning' | 'info'): void {
  // Add to a toast queue managed by Zustand or context:
  addToast({
    id: crypto.randomUUID(),
    message,
    type,
    duration: type === 'error' ? 8000 : 4000, // Errors stay longer
    dismissible: true,
  });
}
```

---

## 7. Logging

### Structured Logging
```typescript
interface LogEntry {
  timestamp: string;
  level: 'error' | 'warn' | 'info' | 'debug';
  action: string;
  context?: Record<string, unknown>;
  error?: {
    message: string;
    stack?: string;
    code?: string;
  };
}

function log(entry: Omit<LogEntry, 'timestamp'>): void {
  const fullEntry: LogEntry = {
    ...entry,
    timestamp: new Date().toISOString(),
  };

  // Console output:
  const method = entry.level === 'error' ? 'error' : entry.level === 'warn' ? 'warn' : 'log';
  console[method](`[${entry.level.toUpperCase()}] ${entry.action}`, entry.context || '', entry.error || '');

  // File output (main process only):
  if (process.type === 'browser') {
    appendToLogFile(fullEntry);
  }
}

// Usage:
log({
  level: 'error',
  action: 'product.save',
  context: { productId: 123, sku: 'ABC-001' },
  error: { message: 'UNIQUE constraint failed', code: 'SQLITE_CONSTRAINT' },
});
```

### Log Levels
- **error**: Something failed that should not have. Requires attention.
- **warn**: Something unexpected happened but was handled. Monitor for patterns.
- **info**: Significant actions completed successfully. Useful for audit trails.
- **debug**: Detailed information for troubleshooting. Disabled in production.

### Log Rotation
```typescript
// Implement basic log rotation to prevent disk fill:
function appendToLogFile(entry: LogEntry): void {
  const logDir = path.join(app.getPath('userData'), 'logs');
  const logFile = path.join(logDir, `app-${new Date().toISOString().split('T')[0]}.log`);

  fs.mkdirSync(logDir, { recursive: true });
  fs.appendFileSync(logFile, JSON.stringify(entry) + '\n');

  // Clean up logs older than 7 days:
  const files = fs.readdirSync(logDir);
  const cutoff = Date.now() - 7 * 24 * 60 * 60 * 1000;
  for (const file of files) {
    const filePath = path.join(logDir, file);
    const stat = fs.statSync(filePath);
    if (stat.mtimeMs < cutoff) {
      fs.unlinkSync(filePath);
    }
  }
}
```
