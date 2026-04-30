# Accessibility — Deep Reference Guide

## Overview

Accessibility is not optional. Every component must meet WCAG 2.1 AA standards. This means
proper contrast ratios, keyboard navigation, screen reader support, and inclusive design
patterns. Premium quality includes premium accessibility.

---

## WCAG AA Contrast Requirements

| Element Type | Minimum Ratio | How to Verify |
|-------------|---------------|---------------|
| Body text (< 18px) | 4.5:1 | Use theme tokens, test both themes |
| Large text (>= 18px or >= 14px bold) | 3:1 | Headers and display text |
| UI components (borders, icons, controls) | 3:1 | Buttons, inputs, checkboxes |
| Non-text graphics (charts, data viz) | 3:1 | Chart lines, graph areas |
| Decorative elements | No requirement | Purely visual, no information conveyed |

### Glass Surface Contrast
On glassmorphic surfaces, text contrast can be unpredictable because the background shows through.
Mitigation strategies:
- Use `text-foreground` (not muted) for primary text on glass
- Add a subtle solid background fallback: `bg-white/10 dark:bg-black/20` provides baseline contrast
- Test with various content behind the glass surface
- Consider adding a text shadow for critical text: `[text-shadow:_0_1px_2px_rgb(0_0_0_/_20%)]`

---

## Keyboard Navigation

### All Interactive Elements Must Be Focusable
```tsx
// Buttons — naturally focusable
<button>Click me</button>

// Links — naturally focusable
<a href="/page">Navigate</a>

// Custom interactive elements — add tabIndex
<div role="button" tabIndex={0} onKeyDown={handleKeyDown} onClick={handleClick}>
  Custom action
</div>
```

### Tab Order
- Use natural DOM order — never set `tabIndex` greater than 0
- `tabIndex={0}` — element is focusable in natural order
- `tabIndex={-1}` — element is focusable programmatically but not via Tab
- Never use `tabIndex={1}`, `tabIndex={2}`, etc. — this breaks natural flow

### Key Handlers for Custom Widgets
```tsx
function handleKeyDown(e: React.KeyboardEvent) {
  switch (e.key) {
    case 'Enter':
    case ' ':
      e.preventDefault();
      handleActivation();
      break;
    case 'Escape':
      handleDismiss();
      break;
    case 'ArrowDown':
      e.preventDefault();
      focusNextItem();
      break;
    case 'ArrowUp':
      e.preventDefault();
      focusPreviousItem();
      break;
  }
}
```

### Keyboard Patterns by Widget

| Widget | Enter/Space | Escape | Arrow Keys | Tab |
|--------|------------|--------|------------|-----|
| Button | Activate | - | - | Move to next |
| Link | Navigate | - | - | Move to next |
| Menu | Open/select item | Close | Navigate items | Move out |
| Modal | - | Close | - | Cycle within |
| Tabs | Select tab | - | Switch tabs | Move out |
| Dropdown | Open/select | Close | Navigate options | Move out |
| Accordion | Toggle section | - | - | Move to next |
| Slider | - | - | Adjust value | Move to next |

---

## ARIA Patterns

### Icon-Only Buttons
```tsx
// Every icon-only button MUST have aria-label
<button aria-label="Close dialog" className="p-2 rounded-md">
  <X className="w-4 h-4" />
</button>

<button aria-label="Delete item" className="p-2 rounded-md">
  <Trash className="w-4 h-4" />
</button>
```

### Loading States
```tsx
// Container-level loading
<div aria-busy={isLoading} aria-live="polite">
  {isLoading ? <Skeleton /> : <Content />}
</div>

// Button loading
<button disabled={isLoading} aria-busy={isLoading}>
  {isLoading ? 'Saving...' : 'Save'}
</button>
```

### Dynamic Content Updates
```tsx
// Non-urgent updates (toast, status change)
<div aria-live="polite" aria-atomic="true">
  {statusMessage}
</div>

// Urgent updates (error, alert)
<div role="alert" aria-live="assertive">
  {errorMessage}
</div>
```

### Custom Select/Listbox
```tsx
<div role="listbox" aria-label="Select option" aria-activedescendant={activeId}>
  {options.map(option => (
    <div
      key={option.id}
      id={option.id}
      role="option"
      aria-selected={selectedId === option.id}
      tabIndex={-1}
      onClick={() => handleSelect(option)}
    >
      {option.label}
    </div>
  ))}
</div>
```

### Modal Dialog
```tsx
<div
  role="dialog"
  aria-modal="true"
  aria-labelledby="modal-title"
  aria-describedby="modal-description"
>
  <h2 id="modal-title">Confirm Action</h2>
  <p id="modal-description">Are you sure you want to proceed?</p>
  {/* Modal content */}
</div>
```

### Expandable Sections
```tsx
<button
  aria-expanded={isOpen}
  aria-controls="section-content"
  onClick={() => setIsOpen(!isOpen)}
>
  Section Title
</button>
<div id="section-content" role="region" hidden={!isOpen}>
  {content}
</div>
```

---

## Focus Management

### Modal Focus Trap
```tsx
import { useEffect, useRef } from 'react';

function useFocusTrap(isActive: boolean) {
  const containerRef = useRef<HTMLDivElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!isActive || !containerRef.current) return;

    // Save current focus
    previousFocusRef.current = document.activeElement as HTMLElement;

    // Focus first focusable element
    const focusable = containerRef.current.querySelectorAll<HTMLElement>(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
    );
    focusable[0]?.focus();

    // Trap focus
    function handleTab(e: KeyboardEvent) {
      if (e.key !== 'Tab' || !containerRef.current) return;
      const focusableElements = containerRef.current.querySelectorAll<HTMLElement>(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
      );
      const first = focusableElements[0];
      const last = focusableElements[focusableElements.length - 1];

      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }

    document.addEventListener('keydown', handleTab);
    return () => {
      document.removeEventListener('keydown', handleTab);
      // Return focus
      previousFocusRef.current?.focus();
    };
  }, [isActive]);

  return containerRef;
}
```

### Skip-to-Content Link
```tsx
// First focusable element on every page
<a
  href="#main-content"
  className="sr-only focus:not-sr-only focus:fixed focus:top-4 focus:left-4 focus:z-[100] focus:px-4 focus:py-2 focus:bg-primary focus:text-primary-foreground focus:rounded-md"
>
  Skip to main content
</a>

// Target
<main id="main-content" tabIndex={-1}>
  {/* Page content */}
</main>
```

### Focus Visible (Keyboard Only)
```tsx
// Focus ring only shows for keyboard navigation, not mouse clicks
<button className="focus-visible:ring-2 focus-visible:ring-primary/50 focus-visible:ring-offset-2 focus-visible:ring-offset-background outline-none">
  Action
</button>
```

---

## Color Independence

Never use color alone to convey meaning. Always pair with icons, text, or patterns:

```tsx
// BAD: only color indicates status
<span className="text-green-500">Active</span>
<span className="text-red-500">Inactive</span>

// GOOD: icon + color + text
<span className="text-green-500 flex items-center gap-1.5">
  <CheckCircle className="w-4 h-4" /> Active
</span>
<span className="text-red-500 flex items-center gap-1.5">
  <XCircle className="w-4 h-4" /> Inactive
</span>
```

---

## Touch and Click Targets

| Platform | Minimum Size | Recommended Size |
|----------|-------------|-----------------|
| Desktop | 32x32px (`w-8 h-8`) | 36x36px (`w-9 h-9`) |
| Mobile | 44x44px (`w-11 h-11`) | 48x48px (`w-12 h-12`) |

```tsx
// Small icon button with adequate hit area
<button className="relative p-2 min-w-[32px] min-h-[32px] flex items-center justify-center">
  <X className="w-4 h-4" />
</button>
```

---

## Disabled States

```tsx
// Use aria-disabled instead of disabled when you need to keep element focusable
<button
  aria-disabled={isDisabled}
  onClick={isDisabled ? undefined : handleClick}
  className={cn(
    "px-4 py-2 rounded-md",
    isDisabled && "opacity-50 cursor-not-allowed pointer-events-none"
  )}
>
  Submit
</button>
```

---

## Error Messages and Form Validation

```tsx
<div>
  <label htmlFor="email" className="text-sm font-medium">
    Email
  </label>
  <input
    id="email"
    type="email"
    aria-invalid={!!error}
    aria-errormessage={error ? 'email-error' : undefined}
    aria-describedby="email-help"
    className={cn(
      "w-full px-3 py-2 rounded-md border",
      error ? "border-destructive" : "border-border"
    )}
  />
  <p id="email-help" className="text-xs text-muted-foreground mt-1">
    We will never share your email.
  </p>
  {error && (
    <p id="email-error" role="alert" className="text-xs text-destructive mt-1 flex items-center gap-1">
      <AlertCircle className="w-3 h-3" /> {error}
    </p>
  )}
</div>
```

On form submission with errors, focus the first field with an error:
```tsx
function focusFirstError() {
  const firstError = document.querySelector('[aria-invalid="true"]') as HTMLElement;
  firstError?.focus();
}
```
