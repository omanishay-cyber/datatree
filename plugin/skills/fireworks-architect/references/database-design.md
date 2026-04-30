# Database Design — sql.js / SQLite Patterns

## Overview

sql.js is SQLite compiled to WebAssembly, running entirely in-process within Electron's main process. It provides a full SQL database without external dependencies, making it ideal for desktop applications that need local data persistence.

---

## Schema Design

### Normalization

Normalize to Third Normal Form (3NF) by default:

- **1NF**: Every column contains atomic values, no repeating groups
- **2NF**: Every non-key column depends on the entire primary key
- **3NF**: Every non-key column depends only on the primary key, not on other non-key columns

### Primary Keys

Use `INTEGER PRIMARY KEY` for auto-increment IDs in SQLite:

```sql
CREATE TABLE products (
  id INTEGER PRIMARY KEY,  -- Auto-increments in SQLite
  name TEXT NOT NULL,
  sku TEXT UNIQUE NOT NULL,
  price REAL NOT NULL DEFAULT 0,
  quantity INTEGER NOT NULL DEFAULT 0,
  category_id INTEGER REFERENCES categories(id),
  active INTEGER NOT NULL DEFAULT 1,  -- SQLite has no BOOLEAN, use 0/1
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### Common Column Patterns

```sql
-- Timestamps (ISO 8601 strings)
created_at TEXT NOT NULL DEFAULT (datetime('now')),
updated_at TEXT NOT NULL DEFAULT (datetime('now')),

-- Soft deletes
deleted_at TEXT DEFAULT NULL,

-- Boolean (SQLite uses INTEGER 0/1)
active INTEGER NOT NULL DEFAULT 1,

-- JSON data (SQLite supports JSON functions)
metadata TEXT DEFAULT '{}',

-- Money (store as INTEGER cents to avoid floating point)
price_cents INTEGER NOT NULL DEFAULT 0,
```

---

## Migrations

### Version Table

Track schema version in the database itself:

```sql
CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  description TEXT NOT NULL,
  applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### Sequential Migration Files

```
src/main/db/migrations/
  |-- 001_initial_schema.sql
  |-- 002_add_categories.sql
  |-- 003_add_sync_metadata.sql
  |-- 004_add_invoice_tables.sql
```

### Migration Runner Pattern

```typescript
const MIGRATIONS = [
  { version: 1, description: 'Initial schema', up: migration001Up },
  { version: 2, description: 'Add categories', up: migration002Up },
  { version: 3, description: 'Add sync metadata', up: migration003Up },
];

function runMigrations(db: Database): void {
  // Create migrations table if not exists
  db.run(`
    CREATE TABLE IF NOT EXISTS schema_migrations (
      version INTEGER PRIMARY KEY,
      description TEXT NOT NULL,
      applied_at TEXT NOT NULL DEFAULT (datetime('now'))
    )
  `);

  // Get current version
  const result = db.exec('SELECT MAX(version) as v FROM schema_migrations');
  const currentVersion = result[0]?.values[0]?.[0] as number ?? 0;

  // Run pending migrations
  for (const migration of MIGRATIONS) {
    if (migration.version > currentVersion) {
      db.run('BEGIN TRANSACTION');
      try {
        migration.up(db);
        db.run(
          'INSERT INTO schema_migrations (version, description) VALUES (?, ?)',
          [migration.version, migration.description]
        );
        db.run('COMMIT');
      } catch (error) {
        db.run('ROLLBACK');
        throw error;
      }
    }
  }
}
```

---

## Query Optimization

### Indexes

Create indexes on columns frequently used in WHERE, JOIN, and ORDER BY:

```sql
-- Single column index
CREATE INDEX idx_products_sku ON products(sku);
CREATE INDEX idx_products_category ON products(category_id);

-- Composite index (for queries that filter by both columns)
CREATE INDEX idx_invoices_date_status ON invoices(date, status);

-- Partial index (only index active products)
CREATE INDEX idx_products_active ON products(name) WHERE active = 1;
```

### EXPLAIN QUERY PLAN

Always check query plans for slow queries:

```sql
EXPLAIN QUERY PLAN
SELECT p.*, c.name as category_name
FROM products p
JOIN categories c ON p.category_id = c.id
WHERE p.active = 1
ORDER BY p.name;
```

Look for `SCAN TABLE` (full table scan — add an index) vs `SEARCH TABLE USING INDEX` (indexed lookup — good).

---

## Transactions

### Wrap Multi-Statement Operations

```typescript
function transferStock(
  db: Database,
  fromId: string,
  toId: string,
  quantity: number
): void {
  db.run('BEGIN TRANSACTION');
  try {
    db.run(
      'UPDATE products SET quantity = quantity - ? WHERE id = ?',
      [quantity, fromId]
    );
    db.run(
      'UPDATE products SET quantity = quantity + ? WHERE id = ?',
      [quantity, toId]
    );
    db.run('COMMIT');
  } catch (error) {
    db.run('ROLLBACK');
    throw error;
  }
}
```

### Batch Inserts

For inserting many rows, use a single transaction:

```typescript
function bulkInsertProducts(db: Database, products: Product[]): void {
  const stmt = db.prepare(
    'INSERT INTO products (name, sku, price, quantity) VALUES (?, ?, ?, ?)'
  );

  db.run('BEGIN TRANSACTION');
  try {
    for (const p of products) {
      stmt.run([p.name, p.sku, p.price, p.quantity]);
    }
    db.run('COMMIT');
  } catch (error) {
    db.run('ROLLBACK');
    throw error;
  } finally {
    stmt.free();
  }
}
```

---

## Parameterized Queries

**ALWAYS use `?` placeholders. NEVER use string concatenation.**

```typescript
// GOOD: parameterized
db.run('SELECT * FROM products WHERE sku = ?', [sku]);
db.run('INSERT INTO products (name, price) VALUES (?, ?)', [name, price]);
db.run('UPDATE products SET price = ? WHERE id = ?', [newPrice, id]);
db.run('DELETE FROM products WHERE id = ?', [id]);

// BAD: string concatenation — SQL injection vulnerability
db.run(`SELECT * FROM products WHERE sku = '${sku}'`);  // NEVER DO THIS
db.run(`DELETE FROM products WHERE id = ${id}`);          // NEVER DO THIS
```

---

## Pagination

### LIMIT/OFFSET (Simple)

```typescript
function getProductsPage(db: Database, page: number, pageSize: number) {
  const offset = (page - 1) * pageSize;
  return db.exec(
    'SELECT * FROM products ORDER BY name LIMIT ? OFFSET ?',
    [pageSize, offset]
  );
}
```

### Cursor-Based (Large Datasets)

For large datasets, cursor-based pagination is more efficient than OFFSET:

```typescript
function getProductsAfter(db: Database, lastId: number, pageSize: number) {
  return db.exec(
    'SELECT * FROM products WHERE id > ? ORDER BY id LIMIT ?',
    [lastId, pageSize]
  );
}
```

---

## Backup and Recovery

### Export Database to File

```typescript
function backupDatabase(db: Database, backupPath: string): void {
  const data = db.export(); // Returns Uint8Array
  const buffer = Buffer.from(data);
  fs.writeFileSync(backupPath, buffer);
}
```

### Import Database from File

```typescript
function restoreDatabase(backupPath: string): Database {
  const buffer = fs.readFileSync(backupPath);
  return new SQL.Database(new Uint8Array(buffer));
}
```

### Auto-Backup Strategy

```typescript
// Backup before risky operations
function autoBackup(db: Database): string {
  const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
  const backupPath = path.join(
    app.getPath('userData'),
    'backups',
    `db-${timestamp}.sqlite`
  );
  backupDatabase(db, backupPath);
  return backupPath;
}
```

---

## Concurrent Access

sql.js runs in-memory and is single-threaded. Key rules:

1. **All database access goes through the main process** — never expose the database object to the renderer
2. **Serialize concurrent IPC requests** — use a queue or mutex if multiple renderer windows can write simultaneously
3. **Save to disk periodically** — sql.js operates in-memory; call `db.export()` and write to disk on a timer and before quit
4. **No WAL mode** — sql.js does not support WAL (Write-Ahead Logging) since it runs in-memory

### Serialization Pattern

```typescript
class DatabaseQueue {
  private queue: Promise<unknown> = Promise.resolve();

  async execute<T>(operation: () => Promise<T>): Promise<T> {
    const result = new Promise<T>((resolve, reject) => {
      this.queue = this.queue
        .then(() => operation())
        .then(resolve)
        .catch(reject);
    });
    return result;
  }
}

const dbQueue = new DatabaseQueue();

// Usage in IPC handlers:
ipcMain.handle('db:insert', async (_event, params) => {
  return dbQueue.execute(async () => {
    // Only one operation runs at a time
    return db.run('INSERT INTO products (name) VALUES (?)', [params.name]);
  });
});
```
