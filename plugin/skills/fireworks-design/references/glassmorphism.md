# Glassmorphism — Deep Reference Guide

## Overview

Glassmorphism creates a frosted-glass effect using backdrop blur, semi-transparent backgrounds,
and subtle borders. It is the signature visual style for all the maintainer projects. Every
surface that could be glass SHOULD be glass — cards, modals, sidebars, dropdowns, tooltips,
navigation bars, and panels.

---

## 3-Tier System

### Tier 1: Subtle Glass
**Classes:** `backdrop-blur-sm bg-white/5 dark:bg-black/10 border border-white/10`

Use for:
- Table row hover backgrounds
- Nested panels inside other glass containers
- Background sections that need minimal emphasis
- List item hover states
- Tag/badge backgrounds
- Tooltip backgrounds in compact UI

```tsx
<div className="backdrop-blur-sm bg-white/5 dark:bg-black/10 border border-white/10 rounded-lg p-3">
  {/* Subtle glass content */}
</div>
```

### Tier 2: Standard Glass
**Classes:** `backdrop-blur-xl bg-white/10 dark:bg-black/20 border border-white/20 shadow-lg`

Use for:
- Cards and content containers
- Sidebars and navigation panels
- Dropdown menus and popovers
- Form containers
- Toolbar backgrounds
- Tab bar backgrounds

```tsx
<div className="backdrop-blur-xl bg-white/10 dark:bg-black/20 border border-white/20 shadow-lg rounded-xl p-4">
  {/* Standard glass content */}
</div>
```

### Tier 3: Prominent Glass
**Classes:** `backdrop-blur-2xl bg-white/15 dark:bg-black/30 border border-white/25 shadow-2xl`

Use for:
- Modal dialogs
- Command palettes
- Hero sections and landing areas
- Floating action panels
- Full-page overlays
- Critical notification banners

```tsx
<div className="backdrop-blur-2xl bg-white/15 dark:bg-black/30 border border-white/25 shadow-2xl rounded-2xl p-6">
  {/* Prominent glass content */}
</div>
```

---

## Backdrop Blur Variants

| Tailwind Class | Blur Radius | Visual Effect | Use Case |
|---------------|-------------|---------------|----------|
| `backdrop-blur-none` | 0px | No blur | Reset/override |
| `backdrop-blur-sm` | 4px | Very subtle | Tier 1, nested glass |
| `backdrop-blur` | 8px | Light frosting | Subtle overlays |
| `backdrop-blur-md` | 12px | Medium frosting | Secondary panels |
| `backdrop-blur-lg` | 16px | Clear frosting | Elevated cards |
| `backdrop-blur-xl` | 24px | Strong frosting | Tier 2, primary surfaces |
| `backdrop-blur-2xl` | 40px | Heavy frosting | Tier 3, modals |
| `backdrop-blur-3xl` | 64px | Maximum frosting | Full-screen overlays |

---

## Background Opacity Ranges

### Light Mode (white backgrounds)
| Opacity | Class | Transparency | Use Case |
|---------|-------|-------------|----------|
| 5% | `bg-white/5` | Nearly transparent | Subtle tier, nested |
| 8% | `bg-white/[0.08]` | Very light | Hover states |
| 10% | `bg-white/10` | Light | Standard tier |
| 12% | `bg-white/[0.12]` | Medium-light | Active states |
| 15% | `bg-white/[0.15]` | Medium | Prominent tier |
| 20% | `bg-white/20` | Visible | Maximum glass opacity |

### Dark Mode (black backgrounds)
| Opacity | Class | Transparency | Use Case |
|---------|-------|-------------|----------|
| 10% | `bg-black/10` | Nearly transparent | Subtle tier |
| 15% | `bg-black/[0.15]` | Light | Hover states |
| 20% | `bg-black/20` | Medium | Standard tier |
| 25% | `bg-black/25` | Medium-visible | Active states |
| 30% | `bg-black/30` | Visible | Prominent tier |

---

## Border Opacity

| Level | Class | Use Case |
|-------|-------|----------|
| Subtle | `border-white/10` | Tier 1, nested glass, minimal emphasis |
| Standard | `border-white/20` | Tier 2, most components |
| Prominent | `border-white/25` | Tier 3, modals, hero sections |
| Accent | `border-primary/30` | Active/selected states with brand color |

---

## Shadow System

| Level | Class | Use Case |
|-------|-------|----------|
| None | `shadow-none` | Flat elements, nested panels |
| Small | `shadow-sm` | Subtle elevation, tags |
| Medium | `shadow-md` | Cards in flat layouts |
| Large | `shadow-lg` | Tier 2 standard glass |
| X-Large | `shadow-xl` | Elevated floating elements |
| 2X-Large | `shadow-2xl` | Tier 3 prominent glass, modals |

### Colored Shadows (Premium Touch)
```css
shadow-lg shadow-primary/5    /* Subtle brand tint */
shadow-xl shadow-primary/10   /* Visible brand glow */
shadow-2xl shadow-black/20    /* Deep dark shadow */
```

---

## Component-Specific Glass Recipes

### Glass Card
```tsx
<div className="backdrop-blur-xl bg-white/10 dark:bg-black/20 border border-white/20 shadow-lg rounded-xl p-4 transition-all duration-200 hover:shadow-xl hover:bg-white/[0.12] dark:hover:bg-black/25">
  {children}
</div>
```

### Glass Modal
```tsx
{/* Backdrop */}
<div className="fixed inset-0 bg-black/40 backdrop-blur-sm z-30" />
{/* Modal */}
<div className="fixed inset-0 flex items-center justify-center z-40 p-4">
  <div className="backdrop-blur-2xl bg-white/15 dark:bg-black/30 border border-white/25 shadow-2xl rounded-2xl p-6 w-full max-w-lg">
    {children}
  </div>
</div>
```

### Glass Dropdown
```tsx
<div className="absolute mt-1 backdrop-blur-xl bg-white/10 dark:bg-black/20 border border-white/20 shadow-xl rounded-lg py-1 z-10 min-w-[200px]">
  {items.map(item => (
    <button className="w-full text-left px-3 py-2 hover:bg-white/10 dark:hover:bg-white/5 transition-colors duration-150">
      {item.label}
    </button>
  ))}
</div>
```

### Glass Sidebar
```tsx
<aside className="w-64 h-screen backdrop-blur-xl bg-white/10 dark:bg-black/20 border-r border-white/20 p-4 flex flex-col">
  {navigation}
</aside>
```

### Glass Tooltip
```tsx
<div className="absolute backdrop-blur-sm bg-white/5 dark:bg-black/10 border border-white/10 rounded-md px-2 py-1 text-sm shadow-md z-60">
  {tooltipContent}
</div>
```

---

## Nested Glass Rules

When placing glass elements inside other glass elements:

1. **Inner elements use a LOWER blur tier** than the parent
   - Parent: `backdrop-blur-xl` -> Child: `backdrop-blur-sm` or no blur
   - Parent: `backdrop-blur-2xl` -> Child: `backdrop-blur-xl` or `backdrop-blur-sm`

2. **Never double-stack heavy blur** — it compounds and kills performance
   - BAD: Parent `backdrop-blur-2xl` + Child `backdrop-blur-2xl`
   - GOOD: Parent `backdrop-blur-2xl` + Child `bg-white/5 border border-white/10` (no blur)

3. **Inner elements reduce background opacity**
   - Parent: `bg-white/10` -> Child: `bg-white/5`
   - This prevents the inner element from becoming opaque through stacking

---

## Performance Guidelines

1. **Limit glass elements:** No more than ~20 glass elements with backdrop-blur visible simultaneously
2. **Use will-change:** Add `will-change-transform` to glass elements that animate
3. **Simplify off-screen:** Remove backdrop-blur from elements scrolled out of viewport (virtualization)
4. **GPU compositing:** `backdrop-blur` triggers GPU compositing — this is good for performance but uses VRAM
5. **Test on target hardware:** Electron apps should test on minimum-spec hardware

## What NOT to Do

- **DO NOT** use colored backgrounds behind glass (use neutral gray/black/white gradients)
- **DO NOT** stack more than 2 glass layers (visual muddiness + performance hit)
- **DO NOT** use glass on text-heavy paragraphs (readability suffers)
- **DO NOT** apply backdrop-blur to elements larger than the viewport
- **DO NOT** animate backdrop-blur values (not GPU-accelerated, causes jank)
- **DO NOT** use glass without any background content to blur (glass needs something behind it)
