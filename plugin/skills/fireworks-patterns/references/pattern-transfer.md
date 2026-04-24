# Pattern Transfer Protocol — Complete Reference

## The Core Insight

Every problem you face as an engineer has been solved before — usually many
times, in many languages, across many domains. The difference between a junior
and a senior engineer is not knowledge of more syntax but the ability to
**recognize problem classes** and **transfer proven solutions** across contexts.

Pattern transfer is a 5-step protocol: Identify, Recall, Extract, Map, Reify.

---

## Step 1: IDENTIFY the Problem Class

Surface symptoms are misleading. Two problems that look completely different
on the surface often belong to the same problem class underneath. Train
yourself to see past the surface.

### Problem Class Taxonomy

#### 1. State Observation
**Surface symptoms:**
- "The UI doesn't update when the data changes"
- "Two components show different values for the same data"
- "State gets out of sync"

**Core problem:** Multiple consumers need to react to state changes from a
single source of truth, with automatic propagation.

**Canonical patterns:** Observer, Pub/Sub, Reactive Streams, Data Binding

#### 2. Async Orchestration
**Surface symptoms:**
- "Race condition between these two API calls"
- "Operations run in the wrong order"
- "Concurrent modifications corrupt data"

**Core problem:** Multiple asynchronous operations must be coordinated with
defined ordering, error handling, and cancellation semantics.

**Canonical patterns:** Promise.all/race, Saga, Workflow, Actor Model, CSP

#### 3. Caching / Memoization
**Surface symptoms:**
- "This calculation is too slow"
- "We're making the same API call repeatedly"
- "The same query runs hundreds of times"

**Core problem:** Expensive computations or data fetches are repeated with
identical inputs, wasting resources.

**Canonical patterns:** Memoization, LRU Cache, Write-Through, Write-Behind,
Cache-Aside, HTTP Caching (ETag, Cache-Control)

#### 4. Rate Limiting / Throttling
**Surface symptoms:**
- "Too many API calls, we're getting rate limited"
- "The search fires on every keystroke"
- "Users can click the button multiple times"

**Core problem:** Actions occur more frequently than the system can handle or
than is useful, requiring flow control.

**Canonical patterns:** Debounce, Throttle, Token Bucket, Leaky Bucket,
Sliding Window, Circuit Breaker

#### 5. Retry / Resilience
**Surface symptoms:**
- "Intermittent network failures crash the app"
- "The API sometimes returns 503"
- "The database connection drops occasionally"

**Core problem:** External systems are unreliable and operations must succeed
eventually despite transient failures.

**Canonical patterns:** Exponential Backoff, Circuit Breaker, Bulkhead,
Timeout, Fallback, Hedged Requests

#### 6. Data Transformation
**Surface symptoms:**
- "The API returns data in a different format than we need"
- "Need to convert between two data models"
- "ETL pipeline for data migration"

**Core problem:** Data exists in one shape/format and must be converted to
another shape/format without loss.

**Canonical patterns:** Adapter, Mapper, Transformer, Pipe/Filter, ETL,
Serialization/Deserialization

#### 7. Access Control
**Surface symptoms:**
- "Only admins should see this page"
- "Users can only edit their own data"
- "API endpoints need authentication"

**Core problem:** Operations and data must be restricted based on identity,
role, or context.

**Canonical patterns:** RBAC, ABAC, ACL, Policy, Guard, Middleware

#### 8. Extensibility
**Surface symptoms:**
- "Need to add new payment methods without changing existing code"
- "Plugin system for third-party extensions"
- "Different behavior for different customer tiers"

**Core problem:** System must accommodate new variations without modifying
existing, tested code.

**Canonical patterns:** Strategy, Plugin, Hook, Decorator, Visitor,
Open/Closed Principle

#### 9. Coordination
**Surface symptoms:**
- "Multiple services need to agree on this operation"
- "Distributed transaction across databases"
- "Leader election for the cluster"

**Core problem:** Multiple independent processes must reach consensus or
coordinate actions without a single point of control.

**Canonical patterns:** Saga, Two-Phase Commit, Event Sourcing, CQRS,
Consensus (Raft/Paxos), Eventual Consistency

#### 10. Lifecycle Management
**Surface symptoms:**
- "Resources leak when the component unmounts"
- "Database connections aren't being closed"
- "Need to initialize before use, cleanup after"

**Core problem:** Resources have create/use/destroy lifecycles that must be
managed correctly to prevent leaks and corruption.

**Canonical patterns:** RAII, Dispose, useEffect cleanup, Context Manager,
Pool, Factory + Registry

---

## Step 2: RECALL Prior Implementations

Once you identify the problem class, recall how it has been solved across
different domains. This builds your "solution vocabulary."

### Cross-Framework Pattern Map

#### State Observation

| Domain | Implementation |
|---|---|
| React | `useEffect` + dependency array, `useSyncExternalStore` |
| Svelte | `$:` reactive declarations, stores with `subscribe` |
| Zustand | `useStore(selector)`, `subscribe()` |
| Electron | `ipcMain.on` / `ipcRenderer.on` / `webContents.send` |
| Flutter | `StreamBuilder`, `ValueNotifier`, `ChangeNotifier`, BLoC |
| Node.js | `EventEmitter`, `stream.Readable` |
| Python | `property` decorator, signals (Django/Qt), `asyncio` events |
| Backend | WebSocket, Server-Sent Events, polling, webhooks |
| Database | Triggers, change streams (MongoDB), CDC (Debezium) |

#### Caching

| Domain | Implementation |
|---|---|
| React | `useMemo`, `React.memo`, `useCallback` |
| Zustand | Selector memoization |
| Electron | In-memory `Map`, `electron-store` |
| Flutter | `compute` cache, `CacheManager` |
| Node.js | `node-cache`, `lru-cache`, Redis client |
| Python | `@functools.lru_cache`, `@cached_property` |
| Backend | Redis, Memcached, CDN, HTTP cache headers |
| Database | Query cache, materialized views |

#### Rate Limiting

| Domain | Implementation |
|---|---|
| React | `useDebouncedCallback`, `lodash.debounce` |
| Electron | IPC throttle middleware, `debounce` on handlers |
| Flutter | `Timer`, `Debouncer` class, `StreamTransformer` |
| Node.js | `express-rate-limit`, token bucket implementation |
| Python | `ratelimit` decorator, `asyncio.Semaphore` |
| Backend | API Gateway rate limiting, Redis-based token bucket |
| Database | Connection pool limits, query timeout |

#### Retry / Resilience

| Domain | Implementation |
|---|---|
| React | `react-query` retry, custom `useRetry` hook |
| Electron | IPC retry wrapper, process restart |
| Flutter | `retry` package, `http_retry`, `Dio` interceptors |
| Node.js | `p-retry`, `cockatiel`, custom retry with backoff |
| Python | `tenacity`, `backoff`, `retry` decorator |
| Backend | Circuit breaker (Hystrix, Polly), service mesh retry |
| Database | Connection retry, transaction retry on deadlock |

---

## Step 3: EXTRACT Canonical Solution (the ESSENCE)

The essence is the core algorithm stripped of all language and framework
specifics. It should be expressible in numbered steps that a developer in
any language could implement.

### Essence Extraction Template

```
PROBLEM CLASS: [name]
ESSENCE:
1. [First step — what happens first]
2. [Second step — what happens next]
3. [Conditional/branching step — when X, do Y; when Z, do W]
4. [Iteration/loop step — repeat until condition]
5. [Termination step — how/when does it end]
6. [Cleanup step — what resources need release]

INVARIANTS:
- [What must always be true]
- [What must never happen]

EDGE CASES:
- [Empty input]
- [Single element]
- [Maximum size]
- [Concurrent access]
- [Failure mid-operation]
```

### Worked Example: Retry with Exponential Backoff

```
PROBLEM CLASS: Retry / Resilience
ESSENCE:
1. Attempt the operation
2. If it succeeds, return the result
3. If it fails with a RETRYABLE error:
   a. Increment attempt counter
   b. If attempts >= max_retries, throw the error
   c. Wait for (base_delay * 2^attempt) milliseconds, with jitter
   d. Go to step 1
4. If it fails with a NON-RETRYABLE error, throw immediately
5. Return the result on success

INVARIANTS:
- Total attempts never exceed max_retries + 1
- Delay increases exponentially but is capped at max_delay
- Jitter prevents thundering herd (all retries at the same time)

EDGE CASES:
- Operation succeeds on first try (no retry needed)
- All retries exhausted (throw the last error)
- Operation times out (treat as retryable)
- Operation partially succeeds (idempotency required)
```

---

## Step 4: MAP to Local Idioms

Translate each step of the essence into the target domain's conventions.
This is where you apply framework-specific knowledge.

### Idiom Mapping Examples

#### Retry → React Hook

```
Essence Step 1 (attempt) → async function call
Essence Step 2 (success) → setState with result, return
Essence Step 3 (fail + retry) → catch block, setTimeout, recursive call
Essence Step 4 (non-retryable) → throw, caught by error boundary
Essence Step 5 (result) → setState triggers re-render

Additional React idioms:
- Use useRef for attempt counter (persists across renders)
- Use useCallback to stabilize the retry function
- Return loading/error/data states for UI consumption
- Cleanup pending timeouts on unmount (useEffect return)
```

#### Retry → Electron IPC

```
Essence Step 1 (attempt) → ipcRenderer.invoke(channel, args)
Essence Step 2 (success) → resolve the promise
Essence Step 3 (fail + retry) → catch, setTimeout, re-invoke
Essence Step 4 (non-retryable) → reject with error, main process logs
Essence Step 5 (result) → send result back to renderer

Additional Electron idioms:
- Main process wraps handler with retry logic
- Timeout includes IPC round-trip overhead
- Failed retries logged with structured logging
- Circuit breaker pattern if external service is down
```

#### Retry → Flutter

```
Essence Step 1 (attempt) → http.get() or dio.get()
Essence Step 2 (success) → return parsed response
Essence Step 3 (fail + retry) → catch DioError, Future.delayed, recursive
Essence Step 4 (non-retryable) → throw, caught by FutureBuilder
Essence Step 5 (result) → update state, rebuild widget

Additional Flutter idioms:
- Use Dio interceptors for automatic retry
- Respect connectivity status (connectivity_plus package)
- Show retry UI with ElevatedButton in error state
- Cancel pending retries on widget dispose
```

---

## Step 5: REIFY (Implement)

Write the actual code. At this point, you have:
1. A clear problem class
2. Known solutions from other domains
3. A language-independent essence
4. A mapping to local idioms

The implementation should feel almost mechanical — the hard thinking is done.

### Reification Example: Retry Hook in React

```typescript
import { useRef, useCallback, useState } from 'react';

interface RetryOptions {
  maxRetries?: number;
  baseDelay?: number;
  maxDelay?: number;
  retryableErrors?: (error: unknown) => boolean;
}

interface RetryState<T> {
  data: T | null;
  error: Error | null;
  isLoading: boolean;
  attempts: number;
  execute: () => Promise<void>;
  reset: () => void;
}

function useRetry<T>(
  operation: () => Promise<T>,
  options: RetryOptions = {}
): RetryState<T> {
  const {
    maxRetries = 3,
    baseDelay = 1000,
    maxDelay = 30000,
    retryableErrors = () => true,
  } = options;

  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [attempts, setAttempts] = useState(0);
  const timeoutRef = useRef<NodeJS.Timeout>();

  const execute = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    for (let attempt = 0; attempt <= maxRetries; attempt++) {
      try {
        setAttempts(attempt + 1);
        const result = await operation();
        setData(result);
        setIsLoading(false);
        return;
      } catch (err) {
        const isRetryable = retryableErrors(err);
        if (!isRetryable || attempt === maxRetries) {
          setError(err instanceof Error ? err : new Error(String(err)));
          setIsLoading(false);
          return;
        }
        const delay = Math.min(baseDelay * Math.pow(2, attempt), maxDelay);
        const jitter = delay * (0.5 + Math.random() * 0.5);
        await new Promise<void>((resolve) => {
          timeoutRef.current = setTimeout(resolve, jitter);
        });
      }
    }
  }, [operation, maxRetries, baseDelay, maxDelay, retryableErrors]);

  const reset = useCallback(() => {
    clearTimeout(timeoutRef.current);
    setData(null);
    setError(null);
    setIsLoading(false);
    setAttempts(0);
  }, []);

  return { data, error, isLoading, attempts, execute, reset };
}
```

---

## Anti-Patterns in Pattern Transfer

### 1. Pattern Obsession
**Symptom:** Applying patterns everywhere, even where a simple function would
suffice. "Everything is a Strategy!" "Everything needs a Factory!"

**Fix:** Patterns exist to solve specific problems. If the problem is simple,
the solution should be simple. A 5-line function does not need the Strategy
pattern — it needs to be a 5-line function.

**Rule of thumb:** If the pattern adds more code than it saves, don't use it.

### 2. Inappropriate Transfer
**Symptom:** Transferring a pattern from one domain where it works perfectly to
another domain where it's a poor fit. "Java uses Abstract Factory everywhere,
so let's use it in our React app!"

**Fix:** Always complete Step 4 (MAP to Local Idioms) before implementing. Each
framework has its own conventions. React uses hooks, not Abstract Factories.
Flutter uses widget composition, not class hierarchies.

**Rule of thumb:** If the pattern fights the framework, the pattern is wrong.

### 3. Cargo Cult Implementation
**Symptom:** Copying pattern structure without understanding the essence. The
code looks like the pattern but doesn't solve the problem.

**Fix:** Always complete Step 3 (EXTRACT Canonical Solution). If you can't
explain the essence in 5 numbered steps without mentioning any language or
framework, you don't understand it well enough to implement it.

**Rule of thumb:** If you can't explain WHY each step exists, you're cargo culting.

### 4. Premature Abstraction
**Symptom:** Building pattern infrastructure for future use cases that may never
arrive. "We might need 10 different sorting strategies, so let's build the
Strategy pattern now" (when you only have 1 sort).

**Fix:** Follow the Rule of Three — don't abstract until you see the same
pattern three times. Until then, inline is fine.

**Rule of thumb:** The best abstraction is no abstraction until you need one.

### 5. Golden Hammer
**Symptom:** Using the same pattern for every problem because you're comfortable
with it. "I know the Observer pattern really well, so I'll use events for
everything."

**Fix:** Maintain a broad solution vocabulary. For every problem, consider at
least 2 alternative patterns before choosing. Use the decision trees from the
main SKILL.md to evaluate options.

**Rule of thumb:** If your only tool is a hammer, everything looks like a nail.

---

## Performance Targets

| Activity | Target Time | Notes |
|---|---|---|
| Problem class identification | < 5 minutes | Recognize the core class, not just symptoms |
| Prior implementation recall | < 5 minutes | List 3+ implementations from different domains |
| Essence extraction | < 10 minutes | 5-8 numbered steps, language-independent |
| Idiom mapping | < 10 minutes | Map each step to framework conventions |
| Implementation (reification) | < 30 minutes | Write working code with tests |
| **Total pattern transfer** | **< 60 minutes** | From problem identification to working code |

If you're spending more than 60 minutes on a pattern transfer, one of these
is happening:
1. You misidentified the problem class (go back to Step 1)
2. The problem is genuinely novel (rare — most problems are not new)
3. The framework has no good idiom for this pattern (consider a different approach)
4. The problem is actually multiple problems (decompose first)

---

## Quick Reference: Problem to Pattern

| I see this problem... | Try this pattern first | Then consider... |
|---|---|---|
| State not updating | Observer / Reactive binding | Event bus, Polling |
| Race condition | Mutex / Semaphore / Queue | Actor model, CSP |
| Slow repeated computation | Memoization / Cache | Lazy evaluation, Pre-computation |
| Too many requests | Debounce / Throttle | Token bucket, Circuit breaker |
| Intermittent failures | Retry with backoff | Circuit breaker, Fallback |
| Format mismatch | Adapter / Mapper | Transformer pipeline |
| Need undo/redo | Command pattern | Event sourcing, Memento |
| Complex conditionals | Strategy / State machine | Table-driven logic |
| Need plugin system | Strategy + Registry | Hook system, Middleware |
| Many-to-many communication | Mediator / Event bus | Pub/Sub, Message queue |
| Complex object construction | Builder | Factory, Fluent API |
| Need single instance | Module scope / DI singleton | Service locator |
| Cross-cutting concerns | Middleware / Decorator | AOP, Interceptors |
| Need audit trail | Event sourcing / Command log | CQRS, Append-only log |
| Feature flags | Strategy + Config | Proxy, Decorator |
