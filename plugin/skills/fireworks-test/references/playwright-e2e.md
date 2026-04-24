# Playwright E2E Patterns — Deep Reference

> Part of the `fireworks-test` skill. See `../SKILL.md` for the master guide.

---

## Playwright Configuration for Electron Apps

### playwright.config.ts

```ts
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests/e2e',
  timeout: 30000,
  retries: process.env.CI ? 2 : 0,
  workers: 1,                         // Electron tests must run serially
  reporter: [
    ['html', { open: 'never' }],
    ['list'],
  ],
  use: {
    trace: 'on-first-retry',          // Capture traces for debugging failures
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  projects: [
    {
      name: 'electron',
      testMatch: '**/*.spec.ts',
    },
  ],
});
```

---

## Launching an Electron Application

### Basic Launcher

```ts
import { test as base, ElectronApplication, Page } from '@playwright/test';
import { _electron as electron } from 'playwright';
import path from 'path';

// Custom test fixture that launches the Electron app
const test = base.extend<{
  electronApp: ElectronApplication;
  mainWindow: Page;
}>({
  electronApp: async ({}, use) => {
    const app = await electron.launch({
      args: [path.join(__dirname, '../../dist/main/index.js')],
      env: {
        ...process.env,
        NODE_ENV: 'test',
        E2E_TEST: 'true',
      },
    });

    await use(app);
    await app.close();
  },

  mainWindow: async ({ electronApp }, use) => {
    // Wait for the first BrowserWindow to open
    const window = await electronApp.firstWindow();

    // Wait for the app to be fully loaded
    await window.waitForLoadState('domcontentloaded');

    // Optionally wait for a specific element that indicates the app is ready
    await window.waitForSelector('[data-testid="app-ready"]', { timeout: 10000 });

    await use(window);
  },
});

export { test };
export { expect } from '@playwright/test';
```

### Using the Custom Fixture

```ts
import { test, expect } from './fixtures';

test('should display the dashboard on startup', async ({ mainWindow }) => {
  await expect(mainWindow.locator('h1')).toHaveText('Dashboard');
  await expect(mainWindow.locator('[data-testid="total-revenue"]')).toBeVisible();
});
```

---

## Page Object Pattern for Electron Windows

### Defining a Page Object

```ts
// tests/e2e/pages/ProductsPage.ts
import { Page, Locator, expect } from '@playwright/test';

export class ProductsPage {
  readonly page: Page;
  readonly searchInput: Locator;
  readonly addButton: Locator;
  readonly productTable: Locator;
  readonly categoryFilter: Locator;
  readonly exportButton: Locator;

  constructor(page: Page) {
    this.page = page;
    this.searchInput = page.locator('[data-testid="product-search"]');
    this.addButton = page.locator('[data-testid="add-product-btn"]');
    this.productTable = page.locator('[data-testid="product-table"]');
    this.categoryFilter = page.locator('[data-testid="category-filter"]');
    this.exportButton = page.locator('[data-testid="export-btn"]');
  }

  async navigate() {
    await this.page.click('[data-testid="nav-products"]');
    await this.page.waitForSelector('[data-testid="product-table"]');
  }

  async search(query: string) {
    await this.searchInput.fill(query);
    // Wait for debounced search to take effect
    await this.page.waitForTimeout(400);
  }

  async addProduct(product: { name: string; price: string; quantity: string; category: string }) {
    await this.addButton.click();
    await this.page.fill('[data-testid="product-name"]', product.name);
    await this.page.fill('[data-testid="product-price"]', product.price);
    await this.page.fill('[data-testid="product-quantity"]', product.quantity);
    await this.page.selectOption('[data-testid="product-category"]', product.category);
    await this.page.click('[data-testid="save-product-btn"]');
    await this.page.waitForSelector('[data-testid="save-success"]');
  }

  async getProductCount(): Promise<number> {
    const rows = this.productTable.locator('tbody tr');
    return rows.count();
  }

  async getProductByName(name: string): Promise<Locator> {
    return this.productTable.locator(`tr:has-text("${name}")`);
  }

  async filterByCategory(category: string) {
    await this.categoryFilter.selectOption(category);
    await this.page.waitForLoadState('networkidle');
  }

  async exportToExcel() {
    await this.exportButton.click();
    // Wait for download to complete
    const download = await this.page.waitForEvent('download');
    return download;
  }
}
```

### Using Page Objects in Tests

```ts
import { test, expect } from './fixtures';
import { ProductsPage } from './pages/ProductsPage';

test.describe('Products Management', () => {
  let productsPage: ProductsPage;

  test.beforeEach(async ({ mainWindow }) => {
    productsPage = new ProductsPage(mainWindow);
    await productsPage.navigate();
  });

  test('should add a new product', async () => {
    const initialCount = await productsPage.getProductCount();

    await productsPage.addProduct({
      name: 'Hennessy VSOP',
      price: '54.99',
      quantity: '12',
      category: 'Cognac',
    });

    const newCount = await productsPage.getProductCount();
    expect(newCount).toBe(initialCount + 1);

    const productRow = await productsPage.getProductByName('Hennessy VSOP');
    await expect(productRow).toBeVisible();
    await expect(productRow).toContainText('$54.99');
  });

  test('should search and filter products', async () => {
    await productsPage.search('Hennessy');

    const count = await productsPage.getProductCount();
    expect(count).toBeGreaterThan(0);

    // Every visible row should contain the search term
    const rows = productsPage.productTable.locator('tbody tr');
    const rowCount = await rows.count();
    for (let i = 0; i < rowCount; i++) {
      await expect(rows.nth(i)).toContainText('Hennessy');
    }
  });
});
```

---

## IPC Testing Through Electron Handle

```ts
import { test, expect } from './fixtures';

test('should communicate via IPC', async ({ electronApp }) => {
  // Evaluate code in the main process context
  const appVersion = await electronApp.evaluate(async ({ app }) => {
    return app.getVersion();
  });
  expect(appVersion).toMatch(/\d+\.\d+\.\d+/);

  // Call an IPC handler from the test
  const result = await electronApp.evaluate(async ({ ipcMain }) => {
    // Access registered handlers
    return new Promise((resolve) => {
      // Simulate an IPC call
      ipcMain.emit('test-channel', {}, 'test-data');
      resolve('done');
    });
  });

  // Or test through the renderer
  const mainWindow = await electronApp.firstWindow();
  const products = await mainWindow.evaluate(async () => {
    return window.api.invoke('db:get-products');
  });
  expect(Array.isArray(products)).toBe(true);
});
```

---

## File Dialog Testing

Playwright can intercept Electron's file dialogs.

```ts
import { test, expect } from './fixtures';

test('should handle file save dialog', async ({ electronApp, mainWindow }) => {
  // Mock the dialog before the action that triggers it
  await electronApp.evaluate(async ({ dialog }) => {
    // Override showSaveDialog to return a specific path
    dialog.showSaveDialog = async () => ({
      canceled: false,
      filePath: '/tmp/test-export.xlsx',
    });
  });

  // Trigger the export action
  await mainWindow.click('[data-testid="export-btn"]');

  // Verify the export completed
  await expect(mainWindow.locator('[data-testid="export-success"]')).toBeVisible();
});

test('should handle canceled file dialog', async ({ electronApp, mainWindow }) => {
  await electronApp.evaluate(async ({ dialog }) => {
    dialog.showOpenDialog = async () => ({
      canceled: true,
      filePaths: [],
    });
  });

  await mainWindow.click('[data-testid="import-btn"]');

  // Nothing should change when user cancels
  await expect(mainWindow.locator('[data-testid="import-success"]')).not.toBeVisible();
});
```

---

## Multi-Window Testing

```ts
import { test, expect } from './fixtures';

test('should open settings in a new window', async ({ electronApp, mainWindow }) => {
  // Click to open settings window
  await mainWindow.click('[data-testid="open-settings"]');

  // Wait for the second window to appear
  const settingsWindow = await electronApp.waitForEvent('window');
  await settingsWindow.waitForLoadState('domcontentloaded');

  // Interact with the settings window
  await expect(settingsWindow.locator('h1')).toHaveText('Settings');
  await settingsWindow.fill('[data-testid="store-name"]', 'Test business');
  await settingsWindow.click('[data-testid="save-settings"]');

  // Verify the change is reflected in the main window
  await mainWindow.bringToFront();
  await expect(mainWindow.locator('[data-testid="store-header"]')).toHaveText('Test business');

  // Close settings window
  await settingsWindow.close();
});
```

---

## Screenshot Comparison Testing

```ts
import { test, expect } from './fixtures';

test('should match dashboard visual snapshot', async ({ mainWindow }) => {
  // Wait for all data to load and animations to complete
  await mainWindow.waitForSelector('[data-testid="dashboard-loaded"]');
  await mainWindow.waitForTimeout(500); // Wait for animations

  // Full page screenshot comparison
  await expect(mainWindow).toHaveScreenshot('dashboard.png', {
    maxDiffPixels: 100,                  // Allow minor rendering differences
    threshold: 0.2,                       // Pixel comparison threshold
  });
});

test('should match product card visual snapshot', async ({ mainWindow }) => {
  const productCard = mainWindow.locator('[data-testid="product-card"]').first();

  await expect(productCard).toHaveScreenshot('product-card.png', {
    maxDiffPixelRatio: 0.01,             // Max 1% of pixels can differ
  });
});

// Update snapshots: npx playwright test --update-snapshots
```

---

## Test Fixtures for Database State

```ts
import { test as base, ElectronApplication, Page } from '@playwright/test';
import { _electron as electron } from 'playwright';
import fs from 'fs/promises';
import path from 'path';

const test = base.extend<{
  electronApp: ElectronApplication;
  mainWindow: Page;
}>({
  electronApp: async ({}, use) => {
    // Copy a seed database to the test directory
    const seedDbPath = path.join(__dirname, 'fixtures/seed-database.db');
    const testDbPath = path.join(__dirname, 'temp/test-database.db');

    await fs.mkdir(path.dirname(testDbPath), { recursive: true });
    await fs.copyFile(seedDbPath, testDbPath);

    const app = await electron.launch({
      args: [path.join(__dirname, '../../dist/main/index.js')],
      env: {
        ...process.env,
        NODE_ENV: 'test',
        DB_PATH: testDbPath,             // Point app to test database
      },
    });

    await use(app);
    await app.close();

    // Clean up test database
    await fs.rm(testDbPath, { force: true });
  },

  mainWindow: async ({ electronApp }, use) => {
    const window = await electronApp.firstWindow();
    await window.waitForLoadState('domcontentloaded');
    await use(window);
  },
});

export { test };
export { expect } from '@playwright/test';
```

---

## CI Configuration

### GitHub Actions

```yaml
# .github/workflows/e2e.yml
name: E2E Tests
on: [push, pull_request]

jobs:
  e2e:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - run: npm ci
      - run: npm run build
      - run: npx playwright install --with-deps chromium
      - run: npx playwright test
      - uses: actions/upload-artifact@v4
        if: failure()
        with:
          name: playwright-report
          path: playwright-report/
```
