# Anti-Patterns — Generic vs Premium Reference

## Overview

This guide contrasts generic/amateur UI patterns with the premium patterns expected in all
the maintainer projects. Use this as a quick-check when building or reviewing any component.
If you see anything from the "Generic" column in your code, replace it with the "Premium" equivalent.

---

## The Comparison Table

| What | Generic (BAD) | Premium (GOOD) |
|------|---------------|----------------|
| **Backgrounds** | Solid flat colors (`bg-white`, `bg-gray-900`) | Glassmorphism with blur (`backdrop-blur-xl bg-white/10 dark:bg-black/20`) |
| **Borders** | Hard opaque borders (`border border-gray-300`) | Semi-transparent white borders (`border border-white/20`) |
| **Shadows** | Basic flat box-shadow (`shadow-md`) | Layered shadows with subtle color tint (`shadow-lg shadow-primary/5`) |
| **Buttons** | Flat colored rectangles | Gradient + hover glow + press scale (`hover:brightness-110 active:scale-[0.98]`) |
| **Cards** | Plain white/dark boxes | Glass cards with subtle border and hover elevation |
| **Inputs** | Default browser styling | Custom glass background + animated focus ring + transition |
| **Modals** | Centered white box on dark overlay | Glass overlay + scale-from-0.95 entrance + backdrop blur |
| **Navigation** | Plain underlined tabs or pills | Animated sliding indicator + glass tab bar (`layoutId`) |
| **Loading** | Spinning circle or text "Loading..." | Shimmer skeleton screens matching the actual content layout |
| **Transitions** | Instant state changes / no transitions | 200ms ease-out on all interactive state changes |
| **Colors** | Hardcoded hex values (`#3B82F6`, `#EF4444`) | Semantic theme tokens (`text-primary`, `bg-destructive`) |
| **Typography** | Single font size for everything | Clear hierarchy with display/heading/body/caption scale |
| **Icons** | Mismatched sizes, weights, or icon sets | Consistent 20px size with matching stroke width, single icon library |
| **Empty States** | Plain text "No data found" | Illustration + descriptive message + call-to-action button |
| **Scrollbars** | Browser default fat scrollbar | Thin custom scrollbar that matches the theme colors |

---

## Detailed Anti-Pattern Breakdowns

### 1. Solid Backgrounds

```tsx
// GENERIC
<div className="bg-white dark:bg-gray-800 rounded-lg p-4">

// PREMIUM
<div className="backdrop-blur-xl bg-white/10 dark:bg-black/20 border border-white/20 shadow-lg rounded-xl p-4">
```

### 2. Hard Borders

```tsx
// GENERIC
<div className="border border-gray-200 dark:border-gray-700">

// PREMIUM
<div className="border border-white/20">
// Note: white/20 works in BOTH light and dark modes on glass surfaces
```

### 3. Flat Buttons

```tsx
// GENERIC
<button className="bg-blue-500 text-white px-4 py-2 rounded">
  Save
</button>

// PREMIUM
<button className="bg-primary text-primary-foreground px-4 py-2 rounded-lg
  hover:brightness-110 hover:shadow-lg hover:shadow-primary/20
  active:scale-[0.98] active:brightness-95
  focus-visible:ring-2 focus-visible:ring-primary/50 focus-visible:ring-offset-2
  transition-all duration-200">
  Save
</button>
```

### 4. Browser Default Inputs

```tsx
// GENERIC
<input className="border border-gray-300 rounded p-2 w-full" />

// PREMIUM
<input className="w-full px-3 py-2 rounded-lg
  backdrop-blur-sm bg-white/5 dark:bg-black/10
  border border-white/20 hover:border-white/30
  focus:border-primary focus-visible:ring-2 focus-visible:ring-primary/50
  transition-all duration-200
  placeholder:text-muted-foreground/50" />
```

### 5. Instant State Changes

```tsx
// GENERIC — jarring instant change
<div className={isOpen ? "block" : "hidden"}>

// PREMIUM — smooth transition
<div className={cn(
  "transition-all duration-200 ease-out",
  isOpen
    ? "opacity-100 translate-y-0"
    : "opacity-0 -translate-y-2 pointer-events-none"
)}>
```

### 6. "Loading..." Text

```tsx
// GENERIC
{isLoading && <p>Loading...</p>}

// PREMIUM — skeleton that matches the content layout
{isLoading ? (
  <div className="space-y-3 animate-pulse">
    <div className="h-6 w-48 bg-muted rounded" />
    <div className="h-4 w-full bg-muted rounded" />
    <div className="h-4 w-3/4 bg-muted rounded" />
  </div>
) : (
  <Content />
)}
```

### 7. Plain Empty States

```tsx
// GENERIC
<p className="text-gray-500 text-center p-4">No items found.</p>

// PREMIUM
<div className="flex flex-col items-center justify-center py-12 px-4 text-center">
  <div className="w-16 h-16 mb-4 rounded-full bg-muted flex items-center justify-center">
    <InboxIcon className="w-8 h-8 text-muted-foreground" />
  </div>
  <h3 className="text-lg font-medium mb-1">No items yet</h3>
  <p className="text-sm text-muted-foreground mb-4 max-w-sm">
    Get started by creating your first item. It only takes a moment.
  </p>
  <button className="px-4 py-2 bg-primary text-primary-foreground rounded-lg
    hover:brightness-110 active:scale-[0.98] transition-all duration-200">
    Create Item
  </button>
</div>
```

### 8. Default Scrollbars

```css
/* GENERIC: browser default scrollbars */

/* PREMIUM: thin themed scrollbars */
.custom-scrollbar::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.15);
  border-radius: 3px;
}
.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.25);
}

/* Tailwind plugin or global CSS */
@layer utilities {
  .scrollbar-thin {
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.15) transparent;
  }
}
```

### 9. Hardcoded Colors

```tsx
// GENERIC — breaks when theme changes
<span className="text-[#3B82F6]">Link text</span>
<div style={{ backgroundColor: '#1F2937' }}>

// PREMIUM — adapts to any theme
<span className="text-primary">Link text</span>
<div className="bg-card">
```

### 10. Mismatched Icons

```tsx
// GENERIC — mixed icon sets, inconsistent sizing
<FaUser size={24} />
<MdSettings style={{ fontSize: 18 }} />
<svg width="20" height="20">...</svg>

// PREMIUM — single icon library, consistent size and stroke
import { User, Settings, Search } from 'lucide-react';

<User className="w-5 h-5" />
<Settings className="w-5 h-5" />
<Search className="w-5 h-5" />
// All icons: same library (lucide-react), same size (20px = w-5 h-5), same stroke width
```

---

## Quick Self-Review Checklist

Before submitting ANY component, verify none of these are present:

1. No solid `bg-white` or `bg-gray-*` on container surfaces (use glass)
2. No `border-gray-*` (use `border-white/20`)
3. No hardcoded hex colors anywhere
4. No missing transitions on interactive elements
5. No browser-default inputs, selects, or scrollbars
6. No "Loading..." text (use skeletons)
7. No empty states without illustration and CTA
8. No inconsistent icon sizes or mixed icon libraries
9. No missing hover/focus/active states on clickable elements
10. No `hidden`/`block` toggle without opacity/transform transition

If any of these exist, fix them before declaring the component complete.
