# Typography — Deep Reference Guide

## Overview

Typography establishes visual hierarchy, guides the user's eye, and sets the premium feel of
the entire application. Every text element must use the correct scale level, weight, and color
from the system below. No arbitrary font sizes.

---

## Full Type Scale

| Level | Tailwind Classes | Size | Weight | Line Height | Letter Spacing | Use For |
|-------|-----------------|------|--------|-------------|----------------|---------|
| **Display** | `text-4xl font-bold tracking-tight leading-tight` | 36px | 700 | 1.1 | -0.02em | Page titles, hero headings |
| **H1** | `text-3xl font-bold tracking-tight leading-snug` | 30px | 700 | 1.2 | -0.015em | Major section titles |
| **H2** | `text-2xl font-semibold leading-snug` | 24px | 600 | 1.25 | normal | Section headers |
| **H3** | `text-xl font-semibold leading-normal` | 20px | 600 | 1.4 | normal | Subsection headers |
| **H4** | `text-lg font-medium leading-normal` | 18px | 500 | 1.4 | normal | Card titles, group labels |
| **Body Large** | `text-base font-normal leading-relaxed` | 16px | 400 | 1.625 | normal | Long-form reading text |
| **Body** | `text-base font-normal leading-normal` | 16px | 400 | 1.5 | normal | Standard body text |
| **Body Small** | `text-sm font-normal leading-normal` | 14px | 400 | 1.4 | normal | Compact body text, table cells |
| **Caption** | `text-sm text-muted-foreground leading-normal` | 14px | 400 | 1.4 | normal | Help text, timestamps, labels |
| **Tiny** | `text-xs text-muted-foreground/70 leading-normal` | 12px | 400 | 1.3 | 0.02em | Badges, counters, fine print |

---

## Font Stacks

### UI Font (Primary)
```css
font-family: system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
```
Tailwind: `font-sans` (configured in tailwind.config)

### Export Font (Excel/PDF)
```css
font-family: 'Calibri', 'Segoe UI', sans-serif;
```
Used in: Excel exports, PDF generation, printed reports

### Code Font (Monospace)
```css
font-family: 'Cascadia Code', 'JetBrains Mono', 'Fira Code', 'Consolas', monospace;
```
Tailwind: `font-mono`

---

## Font Weight Reference

| Tailwind Class | CSS Weight | Use For |
|---------------|-----------|---------|
| `font-normal` | 400 | Body text, paragraphs |
| `font-medium` | 500 | Card titles, labels, nav items |
| `font-semibold` | 600 | Section headers, important labels |
| `font-bold` | 700 | Page titles, CTAs, emphasis |

Rules:
- Never use `font-thin` (100), `font-extralight` (200), or `font-light` (300) — too faint on glass
- Never use `font-extrabold` (800) or `font-black` (900) — too heavy for UI text
- Limit to 2-3 weights per view to maintain visual coherence

---

## Responsive Typography

For smaller screens, scale down text sizes gracefully:

```tsx
// Display: scales from 2xl on mobile to 4xl on desktop
<h1 className="text-2xl md:text-3xl lg:text-4xl font-bold tracking-tight">
  Page Title
</h1>

// Heading: scales from xl to 2xl
<h2 className="text-xl md:text-2xl font-semibold">
  Section Header
</h2>

// Body: stays consistent, adjusts line-height
<p className="text-sm md:text-base leading-relaxed">
  Body content
</p>
```

### Breakpoint Scale
| Level | Mobile (<640px) | Tablet (640-1024px) | Desktop (>1024px) |
|-------|----------------|--------------------|--------------------|
| Display | text-2xl | text-3xl | text-4xl |
| H1 | text-xl | text-2xl | text-3xl |
| H2 | text-lg | text-xl | text-2xl |
| H3 | text-base | text-lg | text-xl |
| Body | text-sm | text-base | text-base |

---

## Text Truncation Patterns

### Single Line Truncation
```tsx
<span className="truncate block max-w-[200px]">
  Very long text that will be truncated with ellipsis
</span>
```

### Multi-Line Clamp
```tsx
// 2-line clamp
<p className="line-clamp-2">
  Long paragraph that will show exactly two lines and then truncate
</p>

// 3-line clamp
<p className="line-clamp-3">
  Slightly more content visible before truncation happens
</p>
```

### Expandable Truncation
```tsx
<p className={cn(
  "transition-all duration-200",
  expanded ? "line-clamp-none" : "line-clamp-2"
)}>
  {longText}
</p>
<button onClick={() => setExpanded(!expanded)} className="text-sm text-primary">
  {expanded ? "Show less" : "Show more"}
</button>
```

---

## Dark Mode Text Colors

All text colors must maintain minimum contrast ratios:

| Element | Light Mode | Dark Mode | Contrast Ratio |
|---------|-----------|-----------|----------------|
| Primary text | `text-foreground` (gray-900) | `text-foreground` (gray-50) | 15:1+ |
| Secondary text | `text-muted-foreground` (gray-500) | `text-muted-foreground` (gray-400) | 4.5:1+ |
| Disabled text | `text-muted-foreground/50` | `text-muted-foreground/50` | 3:1 (min) |
| Link text | `text-primary` | `text-primary` | 4.5:1+ |
| Error text | `text-destructive` | `text-destructive` | 4.5:1+ |
| Success text | `text-green-600` | `text-green-400` | 4.5:1+ |
| Warning text | `text-yellow-600` | `text-yellow-400` | 4.5:1+ |

### On Glass Surfaces
When text sits on glass backgrounds, ensure sufficient contrast:
- Use `text-foreground` for primary text (not gray variants)
- Use `text-muted-foreground` for secondary text
- NEVER use low-opacity text on low-opacity backgrounds (contrast death)
- Test with the glass over both light and dark backgrounds behind it

---

## Typography Anti-Patterns

| Bad | Good | Why |
|-----|------|-----|
| All text same size | Clear hierarchy with 3+ levels | Users cannot scan without hierarchy |
| Hardcoded `text-[17px]` | Standard scale `text-base` | Arbitrary sizes break the rhythm |
| `font-light` on glass | `font-normal` or `font-medium` | Light weight disappears on glass |
| Centered body text | Left-aligned body text | Centered text is harder to read |
| ALL CAPS body text | ALL CAPS for labels/badges only | ALL CAPS reduces readability |
| `leading-none` on paragraphs | `leading-relaxed` on paragraphs | Tight line-height kills readability |
| `text-gray-400` on white bg | `text-muted-foreground` (token) | Hardcoded colors break themes |
