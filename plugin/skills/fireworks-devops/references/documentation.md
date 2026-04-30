# Documentation — Deep Reference

> Documentation sync checklist, changelog format, migration guides, JSDoc/TSDoc guidelines, and automation.

---

## Documentation Sync Checklist

Run this checklist before every release or significant PR.

### Per-Release Checklist

```
README.md:
  [ ] Features list reflects current state (no removed features listed)
  [ ] Installation instructions still work
  [ ] Screenshots are current (if UI changed)
  [ ] Version badge updated (if applicable)
  [ ] Prerequisites list is accurate (Node version, OS requirements)
  [ ] Quick start guide works on a fresh clone

CHANGELOG.md:
  [ ] New version entry added at the top
  [ ] All changes categorized (Added/Changed/Fixed/Removed)
  [ ] Each entry is user-facing (not internal refactoring details)
  [ ] Links to PRs or issues where applicable
  [ ] Date is correct

API Documentation (if public API):
  [ ] All public functions documented
  [ ] Parameter types and descriptions match code
  [ ] Return types documented
  [ ] Examples compile and run
  [ ] Deprecated APIs marked with alternatives

Migration Guide (if breaking changes):
  [ ] Written with before/after code examples
  [ ] All breaking changes listed
  [ ] Automated migration script (if possible)
  [ ] Testing steps for verifying migration
```

### Per-PR Checklist

```
  [ ] README updated if feature changes user-facing behavior
  [ ] JSDoc/TSDoc added for new public functions
  [ ] Inline comments for non-obvious code (but not obvious code)
  [ ] Type exports added for any new public types
```

---

## Changelog Format (Keep a Changelog)

### Standard Structure

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- New feature that hasn't been released yet

## [1.3.0] - 2026-03-25

### Added
- Barcode scanner support with UPC-A, EAN-13, Code-128 detection (#234)
- PDF export for invoices with customizable templates (#256)
- Keyboard shortcuts for common inventory actions (#261)

### Changed
- Improved inventory search performance by 3x (#270)
- Updated Electron from v39 to v40

### Fixed
- Profit margin calculation now accounts for discounts correctly (#245)
- Dark mode text contrast in the reports section (#252)
- Crash when exporting large inventory lists to Excel (#258)

### Deprecated
- Legacy CSV import (use XLSX import instead, removal in v2.0.0)

### Removed
- Support for Node.js 16 (minimum is now Node.js 18)

### Security
- Updated dependency X to fix CVE-2026-XXXXX

## [1.2.1] - 2026-03-10

### Fixed
- Hot fix for database migration failure on fresh installs (#240)

[Unreleased]: https://github.com/user/repo/compare/v1.3.0...HEAD
[1.3.0]: https://github.com/user/repo/compare/v1.2.1...v1.3.0
[1.2.1]: https://github.com/user/repo/compare/v1.2.0...v1.2.1
```

### Changelog Rules

```
- Most recent version at the top
- Use ISO date format: YYYY-MM-DD
- Group by: Added, Changed, Fixed, Deprecated, Removed, Security
- Write from the USER's perspective (not developer's)
  GOOD: "Added barcode scanner support"
  BAD:  "Refactored scanner module to use QuaggaJS"
- Include issue/PR numbers for traceability
- Keep entries concise — one line per change
- Link version numbers to diff URLs at the bottom
```

---

## Migration Guides

### When to Write a Migration Guide

```
ALWAYS write one when:
  - Database schema changes (even if automated)
  - API contract changes (function signatures, IPC channels)
  - Configuration format changes
  - Removing a feature that users relied on
  - Changing default behavior

SKIP when:
  - Internal refactoring with no external impact
  - Adding new features (backward compatible)
  - Bug fixes (unless the bug was being relied upon)
```

### Migration Guide Format

```markdown
# Migration Guide: v1.x to v2.0

## Breaking Changes

### 1. Database Schema Update

The `products` table now uses `decimal` instead of `float` for prices.

**Automatic migration**: The app will migrate automatically on first launch.
**Manual migration**: If you manage the database externally:

```sql
-- Before (v1.x)
CREATE TABLE products (price REAL);

-- After (v2.0)
ALTER TABLE products ADD COLUMN price_new DECIMAL(10,2);
UPDATE products SET price_new = ROUND(price, 2);
-- Then drop old column and rename
```

### 2. IPC Channel Rename

**Before (v1.x):**
```typescript
window.api.getProducts()
```

**After (v2.0):**
```typescript
window.api.inventory.getProducts()
```

**Find and replace**: Search for `window.api.get` and replace with
the appropriate namespaced version. See the mapping table below.

| Old Channel | New Channel |
|------------|-------------|
| `getProducts` | `inventory.getProducts` |
| `saveProduct` | `inventory.saveProduct` |
| `getInvoices` | `billing.getInvoices` |

## Testing Your Migration

1. Back up your database before upgrading
2. Install v2.0
3. Verify the app starts without errors
4. Check that existing data displays correctly
5. Test creating new records
6. Verify all reports generate correctly
```

---

## JSDoc / TSDoc Guidelines

### When to Add Documentation

```
ALWAYS document:
  - Public API functions (exported from a module)
  - Complex algorithms (anything > 20 lines with non-obvious logic)
  - Workarounds (explain WHY the workaround is needed)
  - Configuration options (what each option does)
  - Type definitions that will be used by consumers

NEVER document:
  - Obvious code (getName() returns a name — no kidding)
  - Implementation details that change frequently
  - Every single line (over-documentation is noise)
  - Private functions with self-explanatory names
```

### TSDoc Format

```typescript
/**
 * Calculates the profit margin for a product including applicable discounts.
 *
 * @param unitCost - The wholesale cost per unit
 * @param unitPrice - The retail selling price per unit
 * @param discountPercent - Optional discount percentage (0-100), defaults to 0
 * @returns The profit margin as a decimal (e.g., 0.25 for 25%)
 * @throws {RangeError} If discount percentage is negative or greater than 100
 *
 * @example
 * ```ts
 * const margin = calculateProfitMargin(10, 15);      // 0.333...
 * const margin = calculateProfitMargin(10, 15, 10);   // 0.233...
 * ```
 */
export function calculateProfitMargin(
  unitCost: number,
  unitPrice: number,
  discountPercent: number = 0
): number {
  // implementation
}
```

### Common TSDoc Tags

```
@param name - Description of the parameter
@returns Description of what is returned
@throws {ErrorType} When this error occurs
@example Code example showing usage
@deprecated Use newFunction() instead
@see relatedFunction or URL
@internal Not part of the public API
@alpha / @beta Stability level
```

### Documentation Anti-Patterns

```
BAD: Restating the function name
  /** Gets the user name */
  function getUserName() {}

BAD: Outdated documentation (worse than no documentation)
  /** Returns the price in dollars */
  function getPrice(): Euro {}  // <-- returns Euro, not dollars!

BAD: Documenting obvious parameters
  /** @param id - The id */
  function getById(id: string) {}

GOOD: Explaining non-obvious behavior
  /**
   * Returns products matching the query. Results are cached for 5 minutes.
   * Pass `{ fresh: true }` to bypass the cache.
   */
  function searchProducts(query: string, options?: SearchOptions) {}
```
