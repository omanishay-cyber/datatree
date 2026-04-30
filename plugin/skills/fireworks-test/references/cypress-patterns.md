# Cypress Patterns — Deep Reference

> Part of the `fireworks-test` skill. See `../SKILL.md` for the master guide.

---

## When to Use Cypress vs Playwright

| Factor | Cypress | Playwright |
|--------|---------|------------|
| **Electron app testing** | Limited — web content only | Full support — ElectronApplication API |
| **Web-only projects** | Excellent | Excellent |
| **Component testing** | Built-in (`cypress/component`) | Via `@playwright/experimental-ct-react` |
| **Real browser testing** | Chrome, Firefox, Edge, WebKit | Chrome, Firefox, WebKit |
| **Multi-tab/multi-window** | Not supported | Full support |
| **File dialogs** | Not supported | Supported via Electron evaluate |
| **Network interception** | `cy.intercept()` — very ergonomic | `page.route()` — powerful but more verbose |
| **Time travel debugging** | Built-in DOM snapshot on each step | Trace viewer (similar but post-hoc) |
| **Speed** | Fast for component tests | Fast for E2E |
| **Learning curve** | Lower — chainable API is intuitive | Moderate — async/await, more configuration |

### Decision

- **Electron app** -> Playwright (Cypress cannot launch Electron or test IPC)
- **Web-only project** -> Either works. Cypress for simpler setup, Playwright for power.
- **Component testing only** -> Cypress component testing is excellent for React

---

## Component Testing with Cypress

### Setup

```ts
// cypress.config.ts
import { defineConfig } from 'cypress';

export default defineConfig({
  component: {
    devServer: {
      framework: 'react',
      bundler: 'vite',
    },
    specPattern: 'src/**/*.cy.{ts,tsx}',
    supportFile: 'cypress/support/component.ts',
  },
});
```

### Basic Component Test

```tsx
// src/components/ProductCard/ProductCard.cy.tsx
import { ProductCard } from './ProductCard';

const mockProduct = {
  id: '1',
  name: 'Grey Goose Vodka',
  price: 29.99,
  quantity: 15,
  category: 'Vodka',
};

describe('ProductCard', () => {
  it('should display product details', () => {
    cy.mount(<ProductCard product={mockProduct} />);

    cy.contains('Grey Goose Vodka').should('be.visible');
    cy.contains('$29.99').should('be.visible');
    cy.contains('15').should('be.visible');
  });

  it('should call onEdit when clicking edit button', () => {
    const onEdit = cy.stub().as('onEdit');
    cy.mount(<ProductCard product={mockProduct} onEdit={onEdit} />);

    cy.get('[data-testid="edit-btn"]').click();
    cy.get('@onEdit').should('have.been.calledOnceWith', '1');
  });

  it('should show low stock badge when quantity is below threshold', () => {
    const lowStock = { ...mockProduct, quantity: 2 };
    cy.mount(<ProductCard product={lowStock} lowStockThreshold={5} />);

    cy.get('[data-testid="low-stock-badge"]').should('be.visible');
  });
});
```

---

## Custom Commands for Common Actions

```ts
// cypress/support/commands.ts

// Login command
Cypress.Commands.add('login', (username: string, password: string) => {
  cy.get('[data-testid="username"]').type(username);
  cy.get('[data-testid="password"]').type(password);
  cy.get('[data-testid="login-btn"]').click();
  cy.get('[data-testid="dashboard"]').should('be.visible');
});

// Fill product form
Cypress.Commands.add('fillProductForm', (product: {
  name: string;
  price: string;
  quantity: string;
  category: string;
}) => {
  cy.get('[data-testid="product-name"]').clear().type(product.name);
  cy.get('[data-testid="product-price"]').clear().type(product.price);
  cy.get('[data-testid="product-quantity"]').clear().type(product.quantity);
  cy.get('[data-testid="product-category"]').select(product.category);
});

// Wait for loading to complete
Cypress.Commands.add('waitForLoading', () => {
  cy.get('[data-testid="loading-spinner"]').should('not.exist');
});

// Type declarations for custom commands
declare global {
  namespace Cypress {
    interface Chainable {
      login(username: string, password: string): Chainable<void>;
      fillProductForm(product: { name: string; price: string; quantity: string; category: string }): Chainable<void>;
      waitForLoading(): Chainable<void>;
    }
  }
}
```

---

## Interceptors for Network Mocking

```ts
describe('Product List', () => {
  it('should display products from API', () => {
    cy.intercept('GET', '/api/products', {
      statusCode: 200,
      body: [
        { id: '1', name: 'Hennessy VS', price: 39.99 },
        { id: '2', name: 'Jack Daniels', price: 27.99 },
      ],
    }).as('getProducts');

    cy.visit('/products');
    cy.wait('@getProducts');

    cy.contains('Hennessy VS').should('be.visible');
    cy.contains('Jack Daniels').should('be.visible');
  });

  it('should show error message when API fails', () => {
    cy.intercept('GET', '/api/products', {
      statusCode: 500,
      body: { error: 'Internal server error' },
    }).as('getProductsFailed');

    cy.visit('/products');
    cy.wait('@getProductsFailed');

    cy.get('[data-testid="error-message"]').should('contain', 'Failed to load products');
  });

  it('should show loading state while fetching', () => {
    cy.intercept('GET', '/api/products', (req) => {
      // Delay the response to observe loading state
      req.reply({
        delay: 1000,
        statusCode: 200,
        body: [],
      });
    }).as('getProductsSlow');

    cy.visit('/products');
    cy.get('[data-testid="loading-spinner"]').should('be.visible');
    cy.wait('@getProductsSlow');
    cy.get('[data-testid="loading-spinner"]').should('not.exist');
  });
});
```

---

## cy.clock for Timer-Dependent Tests

```ts
describe('Auto-save', () => {
  it('should auto-save after 5 seconds of inactivity', () => {
    cy.clock();

    cy.mount(<EditProductForm product={mockProduct} onSave={cy.stub().as('onSave')} />);

    // Type something
    cy.get('[data-testid="product-name"]').type(' Updated');

    // Nothing saved yet
    cy.get('@onSave').should('not.have.been.called');

    // Advance time by 5 seconds
    cy.tick(5000);

    // Auto-save should have triggered
    cy.get('@onSave').should('have.been.calledOnce');
  });
});

describe('Session timeout', () => {
  it('should show warning after 25 minutes of inactivity', () => {
    cy.clock();
    cy.visit('/dashboard');

    cy.tick(25 * 60 * 1000); // 25 minutes

    cy.get('[data-testid="session-warning"]').should('be.visible');
    cy.contains('Your session will expire soon').should('be.visible');
  });
});
```

---

## Retry-ability and Assertions

Cypress automatically retries assertions until they pass or time out. This eliminates most timing issues.

```ts
// GOOD: Cypress retries until the element appears (default 4s timeout)
cy.get('[data-testid="product-list"]').should('have.length.at.least', 1);

// GOOD: Cypress retries until text matches
cy.contains('Total: $142.50').should('be.visible');

// BAD: Manual wait — fragile and slow
cy.wait(2000); // Don't do this
cy.get('[data-testid="product-list"]').should('exist');

// Custom timeout for slow operations
cy.get('[data-testid="export-complete"]', { timeout: 10000 }).should('be.visible');

// Chained assertions — all retry together
cy.get('[data-testid="product-card"]')
  .should('have.length', 3)
  .first()
  .should('contain', 'Hennessy')
  .and('contain', '$39.99');
```

### Assertion Anti-Patterns

```ts
// BAD: Asserting on a variable — no retry
const text = cy.get('.title').invoke('text');
expect(text).to.equal('Products'); // Does not retry!

// GOOD: Chain assertion on the element
cy.get('.title').should('have.text', 'Products'); // Retries until true

// BAD: Using .then() breaks retry chain
cy.get('.count').then(($el) => {
  expect($el.text()).to.equal('5'); // Single check, no retry
});

// GOOD: Use should() for retry
cy.get('.count').should('have.text', '5');
```
