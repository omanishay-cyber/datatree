# Architecture Review Reference — Fireworks Review

Detailed checklist for the **Architecture** lens. Use this reference when reviewing code for design quality, maintainability, and structural integrity.

---

## SOLID Principles

### Single Responsibility Principle (SRP)
- A module/class/function should have **one reason to change**
- **Signs of violation**:
  - A component that fetches data AND renders UI AND handles form validation
  - A utility file with 20+ unrelated functions
  - A store that manages auth state AND UI preferences AND product data
  - An IPC handler that validates input AND queries DB AND formats response AND sends notifications
- **Fix**: Extract each responsibility into its own module
- **In React**: Separate data-fetching hooks from presentation components
- **In Electron**: Separate IPC handlers (transport) from business logic (service layer)

### Open/Closed Principle (OCP)
- Open for extension, closed for modification
- **Signs of violation**:
  - Adding a new report type requires modifying the report generator function
  - Adding a new product category requires changing validation logic
  - Switch statements that grow with every new feature
- **Fix**: Use strategy pattern, plugin architecture, or configuration objects
  ```typescript
  // BAD: Must modify function for each new type
  function generateReport(type: string) {
    if (type === 'sales') { ... }
    else if (type === 'inventory') { ... }
    // Must add new else-if for every report
  }

  // GOOD: Register new types without modifying core
  const reportGenerators: Record<string, ReportGenerator> = {};
  function registerReport(type: string, generator: ReportGenerator) {
    reportGenerators[type] = generator;
  }
  function generateReport(type: string) {
    return reportGenerators[type]?.generate();
  }
  ```

### Liskov Substitution Principle (LSP)
- Subtypes must be substitutable for their base types without breaking behavior
- **Signs of violation**:
  - A subclass that throws "not implemented" for a parent method
  - A function that checks `instanceof` to decide behavior
  - A derived type that ignores or overrides parent constraints
- **In TypeScript**: Interfaces and type unions are the primary LSP mechanism
- **Practical check**: Can you replace the type with any of its subtypes and have the code still work?

### Interface Segregation Principle (ISP)
- No code should be forced to depend on methods it does not use
- **Signs of violation**:
  - A component receives 15 props but only uses 3
  - An interface with 20 methods where most implementors only need 5
  - A hook that returns 10 values but callers only destructure 1-2
- **Fix**: Split large interfaces into focused ones
  ```typescript
  // BAD: One giant interface
  interface Product {
    id: string; name: string; price: number;
    supplierName: string; supplierPhone: string; supplierEmail: string;
    warehouseLocation: string; shelfNumber: string;
    taxRate: number; taxCategory: string;
  }

  // GOOD: Focused interfaces composed together
  interface ProductCore { id: string; name: string; price: number; }
  interface SupplierInfo { supplierName: string; supplierPhone: string; supplierEmail: string; }
  interface StorageInfo { warehouseLocation: string; shelfNumber: string; }
  interface TaxInfo { taxRate: number; taxCategory: string; }
  type Product = ProductCore & SupplierInfo & StorageInfo & TaxInfo;
  ```

### Dependency Inversion Principle (DIP)
- High-level modules should not depend on low-level modules. Both should depend on abstractions.
- **Signs of violation**:
  - Business logic directly imports database driver (`import Database from 'better-sqlite3'`)
  - React component directly calls `fetch()` instead of using a service/hook
  - Main process handler directly accesses file system without a service layer
- **Fix**: Depend on interfaces/types, inject implementations
  ```typescript
  // BAD: Direct dependency on implementation
  import Database from 'better-sqlite3';
  function getProducts() {
    const db = new Database('store.db');
    return db.prepare('SELECT * FROM products').all();
  }

  // GOOD: Depend on abstraction
  interface ProductRepository {
    getAll(): Product[];
    getById(id: string): Product | null;
  }
  // Implementation injected at app startup
  ```

---

## Coupling

### Tight Coupling Signs
- **Direct imports across boundaries**: Feature A imports internal modules of Feature B
- **Shared mutable state**: Two modules modifying the same global/shared object
- **Circular dependencies**: A imports B, B imports A (even transitively: A->B->C->A)
- **Temporal coupling**: Function A must be called before Function B, but nothing enforces this
- **Content coupling**: Module A depends on the internal implementation details of Module B
- **Stamp coupling**: Passing entire objects when only one field is needed

### Loose Coupling Patterns
- **Dependency injection**: Pass dependencies as parameters, not hardcoded imports
- **Event-driven communication**: Emit events instead of calling directly across boundaries
  ```typescript
  // TIGHT: Direct call across feature boundaries
  import { refreshInventory } from '../inventory/store';
  function afterSale() { refreshInventory(); }

  // LOOSE: Event-based
  eventBus.emit('sale:completed', { saleId });
  // Inventory module listens for this event independently
  ```
- **Interfaces / Type contracts**: Depend on the shape, not the implementation
- **Mediator pattern**: Central coordinator that features communicate through
- **Message passing**: IPC channels in Electron are a natural decoupling mechanism

### Circular Dependency Detection
- Circular imports cause subtle bugs: modules may be partially initialized when accessed
- **Symptoms**: `undefined` when accessing an import, intermittent errors on startup
- **Detection**: Build warnings, `madge --circular` tool, ESLint `import/no-cycle` rule
- **Fix**: Extract shared code into a third module that both depend on, or use lazy imports

---

## Cohesion

### High Cohesion (Good)
- All functions in a module relate to the same domain concept
- A component's state, handlers, and render logic all serve one purpose
- A store manages one domain: products, auth, or UI state — not all three
- File name accurately describes everything inside it

### Low Cohesion (Bad)
- A `utils.ts` file with 30 functions covering 10 different domains
- A component that handles user authentication AND product display
- A store that mixes UI state (sidebar open) with domain state (current user)
- **Fix**: Split into focused modules named after their domain

### Feature-Based Organization
```
// BAD: Type-based organization (low cohesion across features)
src/
  components/
    ProductList.tsx
    SaleForm.tsx
    InventoryTable.tsx
  hooks/
    useProducts.ts
    useSales.ts
    useInventory.ts
  stores/
    productStore.ts
    saleStore.ts
    inventoryStore.ts

// GOOD: Feature-based organization (high cohesion within features)
src/
  features/
    products/
      ProductList.tsx
      useProducts.ts
      productStore.ts
      productTypes.ts
    sales/
      SaleForm.tsx
      useSales.ts
      saleStore.ts
      saleTypes.ts
    inventory/
      InventoryTable.tsx
      useInventory.ts
      inventoryStore.ts
      inventoryTypes.ts
```
- Feature-based organization makes it easy to find all related code
- Cross-feature shared code lives in a `shared/` or `common/` directory
- Each feature folder should be self-contained enough to understand in isolation

---

## Abstraction Levels

### Mixing Levels (Bad)
```typescript
// BAD: Business logic mixed with I/O and formatting
async function processSale(saleData: SaleInput) {
  // Low-level: raw database query
  const product = db.prepare('SELECT * FROM products WHERE id = ?').get(saleData.productId);

  // Business logic: calculate total
  const total = product.price * saleData.quantity * (1 + product.taxRate);

  // Low-level: file system write
  fs.writeFileSync(`receipts/${Date.now()}.txt`, `Total: $${total.toFixed(2)}`);

  // Low-level: IPC response formatting
  return { success: true, data: { total, product: product.name } };
}
```

### Clean Separation (Good)
```typescript
// HIGH-LEVEL: Business logic only
async function processSale(saleData: SaleInput): Promise<SaleResult> {
  const product = await productRepo.getById(saleData.productId);
  const total = calculateTotal(product.price, saleData.quantity, product.taxRate);
  await receiptService.generate(product, total);
  return { total, productName: product.name };
}

// MID-LEVEL: Service layer
function calculateTotal(price: number, quantity: number, taxRate: number): number {
  return price * quantity * (1 + taxRate);
}

// LOW-LEVEL: Data access
class ProductRepository {
  getById(id: string): Product | null {
    return db.prepare('SELECT * FROM products WHERE id = ?').get(id);
  }
}
```

### Leaky Abstractions
- When implementation details leak through the abstraction layer
- Example: A `useProducts` hook that returns raw SQL error messages to the component
- Example: A service that exposes database-specific pagination (offset-based vs cursor-based)
- **Fix**: Map low-level details to domain-level concepts at the boundary

### Missing Abstraction Layers
- When high-level code directly uses low-level APIs
- Symptom: React component contains raw SQL, file system calls, or exec commands
- **Electron rule**: Renderer never touches Node.js APIs directly — always through preload/IPC
- Each layer should only talk to the layer directly below it

---

## Pattern Compliance

### Consistency Check
When reviewing new code, compare it against existing patterns in the codebase:

| Aspect | What to Check |
|--------|---------------|
| **Naming** | Does it follow the project's naming conventions? (camelCase, PascalCase, kebab-case for files?) |
| **File organization** | Is the file in the right directory? Does it follow the feature-based structure? |
| **Error handling** | Does it handle errors the same way as existing code? (try/catch, error boundaries, Result types?) |
| **State management** | Does it use the same store patterns? Selectors? Actions? |
| **Data fetching** | Does it follow the project's data fetching pattern? (hooks, services, IPC?) |
| **Component structure** | Does it follow the project's component conventions? (functional, named exports, prop types?) |
| **Testing** | Does it follow existing test patterns? (file naming, test structure, assertion style?) |
| **IPC channels** | Does it follow the channel naming convention? Is it typed end-to-end? |
| **Type definitions** | Are types co-located with their usage? Are shared types in the right place? |
| **Imports** | Do import paths follow the project's alias/path conventions? |

### When to Deviate from Patterns
- The existing pattern has a known flaw that this change could fix
- A new pattern is clearly better AND the team has agreed to migrate
- The existing pattern does not apply to this domain (e.g., different data access needs)
- **Always document WHY you deviated** — future reviewers need context

### Anti-Patterns to Flag
- **God module**: One file that does everything — split by responsibility
- **Shotgun surgery**: One change requires modifying 10+ files — indicates poor encapsulation
- **Feature envy**: A function that uses more data from another module than its own
- **Primitive obsession**: Using `string` and `number` everywhere instead of domain types
- **Copy-paste inheritance**: Duplicated code blocks with minor variations — extract shared logic
