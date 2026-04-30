# Code Smells — Full Catalog

> Every smell listed here includes: what it is, how to detect it, a bad example, a good example, and the named refactoring technique to fix it. All examples use TypeScript.

---

## 1. Long Method

**What**: A function that does too many things. Hard to read, hard to test, hard to reuse.

**Detection Pattern**: Function body exceeds 20 lines. Multiple levels of indentation. Comments explaining "sections" of the function.

**Bad Example**:
```typescript
function processOrder(order: Order): Invoice {
  // validate
  if (!order.items.length) throw new Error('Empty order');
  if (!order.customer) throw new Error('No customer');
  for (const item of order.items) {
    if (item.quantity <= 0) throw new Error('Invalid quantity');
    if (item.price < 0) throw new Error('Invalid price');
  }
  // calculate totals
  let subtotal = 0;
  for (const item of order.items) {
    subtotal += item.price * item.quantity;
  }
  const tax = subtotal * 0.07;
  const total = subtotal + tax;
  // create invoice
  const invoice: Invoice = {
    id: generateId(),
    customer: order.customer,
    items: order.items,
    subtotal, tax, total,
    date: new Date(),
  };
  // save and notify
  db.save(invoice);
  emailService.send(order.customer.email, invoice);
  return invoice;
}
```

**Good Example**:
```typescript
function processOrder(order: Order): Invoice {
  validateOrder(order);
  const totals = calculateTotals(order.items);
  const invoice = createInvoice(order, totals);
  saveAndNotify(invoice, order.customer);
  return invoice;
}
```

**Technique**: **Extract Method** — pull each logical section into its own named function.

---

## 2. Feature Envy

**What**: A method that uses data from another object more than its own. It "envies" another class.

**Detection Pattern**: A method references `other.x`, `other.y`, `other.z` repeatedly but rarely touches `this`.

**Bad Example**:
```typescript
class InvoicePrinter {
  printTotal(invoice: Invoice): string {
    const subtotal = invoice.items.reduce((sum, i) => sum + i.price * i.qty, 0);
    const tax = subtotal * invoice.taxRate;
    const discount = subtotal * invoice.discountPercent;
    return `Total: ${subtotal - discount + tax}`;
  }
}
```

**Good Example**:
```typescript
class Invoice {
  getTotal(): number {
    const subtotal = this.items.reduce((sum, i) => sum + i.price * i.qty, 0);
    const tax = subtotal * this.taxRate;
    const discount = subtotal * this.discountPercent;
    return subtotal - discount + tax;
  }
}

class InvoicePrinter {
  printTotal(invoice: Invoice): string {
    return `Total: ${invoice.getTotal()}`;
  }
}
```

**Technique**: **Move Method** — relocate the method to the class whose data it primarily uses.

---

## 3. Data Clump

**What**: The same group of variables appears together in multiple places (function params, class fields, etc.).

**Detection Pattern**: 3+ parameters that always travel together across multiple function signatures.

**Bad Example**:
```typescript
function createCustomer(name: string, street: string, city: string, state: string, zip: string) { ... }
function updateAddress(customerId: string, street: string, city: string, state: string, zip: string) { ... }
function validateAddress(street: string, city: string, state: string, zip: string): boolean { ... }
```

**Good Example**:
```typescript
interface Address {
  street: string;
  city: string;
  state: string;
  zip: string;
}

function createCustomer(name: string, address: Address) { ... }
function updateAddress(customerId: string, address: Address) { ... }
function validateAddress(address: Address): boolean { ... }
```

**Technique**: **Extract Class / Extract Interface** — group the clump into a cohesive type.

---

## 4. Primitive Obsession

**What**: Using primitive types (string, number) where a domain-specific type would add safety and clarity.

**Detection Pattern**: String/number parameters with names like `email`, `price`, `currency`, `phone`, `percentage`.

**Bad Example**:
```typescript
function applyDiscount(price: number, discount: number, currency: string): number {
  // Is discount a percentage (0.1) or absolute (10)? Is currency "USD" or "usd"?
  return price - discount;
}
```

**Good Example**:
```typescript
interface Money {
  amount: number;
  currency: Currency;
}

interface Percentage {
  value: number; // 0-100
}

function applyDiscount(price: Money, discount: Percentage): Money {
  return {
    amount: price.amount * (1 - discount.value / 100),
    currency: price.currency,
  };
}
```

**Technique**: **Replace Primitive with Value Object** — create a domain type that encapsulates validation and behavior.

---

## 5. Switch Statement Smell

**What**: The same switch/if-else chain on a type discriminator appears in multiple places. Adding a new type requires editing every switch.

**Detection Pattern**: Multiple functions with `switch (type)` or `if (type === 'X')` on the same discriminator.

**Bad Example**:
```typescript
function calculatePay(employee: Employee): number {
  switch (employee.type) {
    case 'hourly': return employee.hours * employee.rate;
    case 'salaried': return employee.salary / 26;
    case 'commission': return employee.sales * employee.commissionRate;
  }
}

function getTitle(employee: Employee): string {
  switch (employee.type) {
    case 'hourly': return 'Hourly Worker';
    case 'salaried': return 'Salaried Employee';
    case 'commission': return 'Sales Associate';
  }
}
```

**Good Example**:
```typescript
interface PayStrategy {
  calculatePay(employee: Employee): number;
  getTitle(): string;
}

const payStrategies: Record<EmployeeType, PayStrategy> = {
  hourly: {
    calculatePay: (e) => e.hours * e.rate,
    getTitle: () => 'Hourly Worker',
  },
  salaried: {
    calculatePay: (e) => e.salary / 26,
    getTitle: () => 'Salaried Employee',
  },
  commission: {
    calculatePay: (e) => e.sales * e.commissionRate,
    getTitle: () => 'Sales Associate',
  },
};
```

**Technique**: **Replace Conditional with Polymorphism** or **Strategy Pattern** — use a lookup map or polymorphic dispatch.

---

## 6. Parallel Inheritance Hierarchies

**What**: Every time you add a subclass in one hierarchy, you must add a corresponding subclass in another.

**Detection Pattern**: Two class hierarchies with matching names (e.g., `SqlOrder` / `SqlOrderProcessor`, `MongoOrder` / `MongoOrderProcessor`).

**Bad Example**:
```typescript
class SqlOrder extends Order { ... }
class MongoOrder extends Order { ... }
class SqlOrderRepository extends OrderRepository { ... }
class MongoOrderRepository extends OrderRepository { ... }
// Adding a new DB type requires TWO new classes
```

**Good Example**:
```typescript
interface DatabaseAdapter {
  save(entity: Record<string, unknown>): Promise<void>;
  find(id: string): Promise<Record<string, unknown> | null>;
}

class OrderRepository {
  constructor(private db: DatabaseAdapter) {}
  async save(order: Order): Promise<void> { await this.db.save(order); }
}
// Adding a new DB type requires ONE new adapter
```

**Technique**: **Merge Hierarchies / Composition over Inheritance** — use a single hierarchy with injected behavior.

---

## 7. Lazy Class

**What**: A class that does almost nothing. It exists as a thin wrapper or pass-through with no real logic.

**Detection Pattern**: Class with 1-2 methods that simply delegate to another object. Total code under 10 lines.

**Bad Example**:
```typescript
class TaxCalculator {
  calculate(amount: number): number {
    return amount * 0.07;
  }
}
// Used in exactly one place, could be a simple function or inline expression
```

**Good Example**:
```typescript
// Inline the logic where it is used
const tax = amount * 0.07;

// Or if reuse is needed, a plain function
function calculateTax(amount: number, rate: number = 0.07): number {
  return amount * rate;
}
```

**Technique**: **Inline Class** — collapse the class into its consumer. Use a plain function if logic is reusable.

---

## 8. Speculative Generality

**What**: Abstractions created "just in case" that have only one implementation. Adds complexity without value.

**Detection Pattern**: Abstract class with exactly one subclass. Interface with exactly one implementation. Generic type parameter used with only one concrete type.

**Bad Example**:
```typescript
interface IUserRepository {
  findById(id: string): Promise<User | null>;
  save(user: User): Promise<void>;
}

class UserRepository implements IUserRepository {
  // The only implementation — the interface adds nothing
  async findById(id: string): Promise<User | null> { ... }
  async save(user: User): Promise<void> { ... }
}
```

**Good Example**:
```typescript
class UserRepository {
  async findById(id: string): Promise<User | null> { ... }
  async save(user: User): Promise<void> { ... }
}
// Extract an interface WHEN you actually need a second implementation
```

**Technique**: **Collapse Hierarchy / Remove Interface** — delete the abstraction until a second implementation justifies it.

---

## 9. Temporary Field

**What**: An object field that is only populated or meaningful under certain conditions. Most of the time it is null/undefined.

**Detection Pattern**: Fields checked with `if (this.x)` before every use. Fields set only in specific methods, not in the constructor.

**Bad Example**:
```typescript
class ReportGenerator {
  private tempData: ReportData | null = null;
  private tempFormat: string | null = null;

  prepare(data: ReportData, format: string) {
    this.tempData = data;
    this.tempFormat = format;
  }

  generate(): string {
    if (!this.tempData || !this.tempFormat) throw new Error('Call prepare() first');
    return this.tempFormat === 'html' ? toHtml(this.tempData) : toCsv(this.tempData);
  }
}
```

**Good Example**:
```typescript
class ReportGenerator {
  generate(data: ReportData, format: 'html' | 'csv'): string {
    return format === 'html' ? toHtml(data) : toCsv(data);
  }
}
// Or if state is truly needed, extract a ReportJob class
```

**Technique**: **Extract Class** for the conditional behavior, or pass data as parameters instead of storing as fields.

---

## 10. Message Chain

**What**: A long chain of method calls: `a.getB().getC().getD().doThing()`. Couples the caller to the entire chain structure.

**Detection Pattern**: 3+ chained dot accesses. Caller must know the internal structure of multiple objects.

**Bad Example**:
```typescript
const cityName = order.getCustomer().getAddress().getCity().getName();
```

**Good Example**:
```typescript
// Add a convenience method on Order
class Order {
  getCustomerCity(): string {
    return this.customer.address.city.name;
  }
}

const cityName = order.getCustomerCity();
```

**Technique**: **Hide Delegate** — introduce a method on the nearest object that encapsulates the chain.

---

## 11. Middle Man

**What**: A class where most methods simply delegate to another object. The class adds no value.

**Detection Pattern**: More than half the methods are one-line delegations: `return this.delegate.method()`.

**Bad Example**:
```typescript
class CustomerService {
  constructor(private repo: CustomerRepository) {}
  findById(id: string) { return this.repo.findById(id); }
  save(customer: Customer) { return this.repo.save(customer); }
  delete(id: string) { return this.repo.delete(id); }
  findAll() { return this.repo.findAll(); }
  // Every method just calls repo — this class is pointless
}
```

**Good Example**:
```typescript
// Let callers use the repository directly
class OrderProcessor {
  constructor(private customerRepo: CustomerRepository) {}

  async processOrder(orderId: string) {
    const customer = await this.customerRepo.findById(orderId);
    // ... actual business logic here
  }
}
```

**Technique**: **Remove Middle Man** — let callers interact with the delegate directly.

---

## 12. Shotgun Surgery

**What**: A single logical change requires modifying many different files/classes. Changes are scattered.

**Detection Pattern**: Adding a new field requires editing 5+ files. A "simple" change touches model, service, controller, view, test, and config.

**Bad Example**:
```
Adding a "phone" field to Customer requires editing:
  - customer.model.ts (add field)
  - customer.service.ts (add to CRUD)
  - customer.controller.ts (add to API)
  - customer.validator.ts (add validation)
  - customer.mapper.ts (add to mapping)
  - customer.form.tsx (add input)
  - customer.table.tsx (add column)
```

**Good Example**:
```
Consolidate related logic so a new field only requires:
  - customer.schema.ts (single source of truth — generates types, validation, form fields)
  - customer.table.tsx (add column config)
Derive everything else from the schema.
```

**Technique**: **Move Method / Move Field** — consolidate scattered logic. Use schema-driven code generation where possible.

---

## 13. Divergent Change

**What**: One class is modified for many unrelated reasons. It has too many responsibilities.

**Detection Pattern**: Git log shows the same file modified in commits for different features. The class has methods that serve different concerns.

**Bad Example**:
```typescript
class UserManager {
  authenticate(email: string, password: string) { ... }  // auth concern
  updateProfile(userId: string, data: ProfileData) { ... }  // profile concern
  sendWelcomeEmail(userId: string) { ... }  // notification concern
  calculateLoyaltyPoints(userId: string) { ... }  // business logic concern
  exportToCSV(users: User[]) { ... }  // reporting concern
}
```

**Good Example**:
```typescript
class AuthService { authenticate(email: string, password: string) { ... } }
class ProfileService { updateProfile(userId: string, data: ProfileData) { ... } }
class NotificationService { sendWelcomeEmail(userId: string) { ... } }
class LoyaltyService { calculatePoints(userId: string) { ... } }
class UserExporter { exportToCSV(users: User[]) { ... } }
```

**Technique**: **Extract Class** — one class per responsibility (Single Responsibility Principle).

---

## 14. God Class

**What**: A massive class that knows everything, does everything, and everything depends on it. The single point of failure.

**Detection Pattern**: Class with 500+ lines. 10+ methods. Imported by most other files. Has fields for many unrelated concerns.

**Bad Example**:
```typescript
class AppManager {
  private db: Database;
  private auth: AuthState;
  private cart: CartState;
  private ui: UIState;
  private notifications: Notification[];
  private analytics: AnalyticsTracker;
  // ... 50 more fields, 80 methods covering every feature
}
```

**Good Example**:
```typescript
// Decompose into focused modules
class AuthStore { ... }       // authentication state
class CartStore { ... }       // shopping cart state
class UIStore { ... }         // UI preferences
class NotificationStore { ... } // notifications
class AnalyticsService { ... }  // analytics

// Compose at the top level if needed
interface AppServices {
  auth: AuthStore;
  cart: CartStore;
  ui: UIStore;
  notifications: NotificationStore;
  analytics: AnalyticsService;
}
```

**Technique**: **Extract Class** repeatedly. Identify clusters of related fields and methods. Each cluster becomes its own class. Repeat until no class exceeds ~200 lines.

---

## Quick Decision Matrix

| Smell | Key Question | Action |
|---|---|---|
| Long Method | Can I name what each section does? | Extract Method for each section |
| Feature Envy | Whose data does this method use most? | Move to that class |
| Data Clump | Do these params always travel together? | Extract into an interface |
| Primitive Obsession | Would a domain type prevent bugs? | Create a value object |
| Switch Smell | Will I need to add cases often? | Use polymorphism or strategy map |
| Parallel Inheritance | Must I add classes in pairs? | Merge with composition |
| Lazy Class | Does this class earn its existence? | Inline it |
| Speculative Generality | Is there more than one implementation? | Remove until needed |
| Temporary Field | Is this field always valid? | Pass as parameter or extract class |
| Message Chain | Does the caller need to know the chain? | Hide delegate |
| Middle Man | Does this class add any logic? | Remove it, use delegate directly |
| Shotgun Surgery | Does one change touch many files? | Consolidate related logic |
| Divergent Change | Does one class change for many reasons? | Split by responsibility |
| God Class | Does this class do everything? | Extract classes until each is focused |
