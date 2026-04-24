# fireworks-design — Full Guide

> This file contains the full reference material previously embedded in SKILL.md.
> Loaded on demand when the skill needs deeper context.

## 6. Color System (Semantic Tokens)

| Token | Light Mode | Dark Mode | Usage |
|-------|-----------|-----------|-------|
| `background` | white/near-white | dark gray/near-black | Page background |
| `foreground` | gray-900 | gray-50 | Primary text |
| `primary` | Brand blue | Brand blue (lighter) | CTAs, active states |
| `primary-foreground` | white | white | Text on primary bg |
| `secondary` | gray-100 | gray-800 | Secondary buttons, tags |
| `secondary-foreground` | gray-900 | gray-100 | Text on secondary bg |
| `accent` | gray-100 | gray-800 | Hover backgrounds |
| `accent-foreground` | gray-900 | gray-100 | Text on accent bg |
| `destructive` | red-500 | red-400 | Delete, errors |
| `destructive-foreground` | white | white | Text on destructive bg |
| `muted` | gray-100 | gray-800 | Disabled backgrounds |
| `muted-foreground` | gray-500 | gray-400 | Secondary text, captions |
| `border` | gray-200 | gray-700 | Borders, dividers |
| `ring` | primary/50 | primary/50 | Focus rings |
| `card` | white | gray-900 | Card backgrounds |
| `popover` | white | gray-900 | Popover backgrounds |

**Rules:**
- NEVER use hardcoded hex colors — always semantic tokens
- Use `/opacity` modifiers for transparent variants: `bg-primary/10`
- Glass borders use `border-white/20` (both themes)
- Check contrast ratios: 4.5:1 for body text, 3:1 for large text

---

## 7. Component Composition

### cn() Utility Pattern
```typescript
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

// Usage
<div className={cn(
  "base-classes here",
  variant === "primary" && "bg-primary text-primary-foreground",
  variant === "ghost" && "hover:bg-accent",
  disabled && "opacity-50 pointer-events-none",
  className // always spread user's className last
)} />
```

### Compound Components Pattern
```typescript
const TabsContext = React.createContext<TabsContextValue | null>(null);

function Tabs({ children, value, onValueChange }: TabsProps) {
  return (
    <TabsContext.Provider value={{ value, onValueChange }}>
      <div role="tablist">{children}</div>
    </TabsContext.Provider>
  );
}

function Tab({ value, children }: TabProps) {
  const ctx = React.useContext(TabsContext);
  return (
    <button
      role="tab"
      aria-selected={ctx?.value === value}
      onClick={() => ctx?.onValueChange(value)}
    >
      {children}
    </button>
  );
}
```

### forwardRef Pattern
```typescript
const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, ...props }, ref) => {
    return (
      <button
        ref={ref}
        className={cn(buttonVariants({ variant, size }), className)}
        {...props}
      />
    );
  }
);
Button.displayName = 'Button';
```

See `references/react-patterns.md` for complete patterns library.

---

## 8. Dual-Theme Verification Checklist

Before declaring ANY UI work complete, verify ALL 10 items:

- [ ] **1. Text Contrast** — All text readable in both light and dark (4.5:1 ratio minimum)
- [ ] **2. Background Layers** — Glass tiers visible and distinct in both themes
- [ ] **3. Borders** — Borders visible but subtle in both themes (white/20 works for glass)
- [ ] **4. Interactive States** — Hover, focus, active, disabled all visible in both themes
- [ ] **5. Icons** — All icons have appropriate contrast in both themes
- [ ] **6. Shadows** — Shadows visible in light, not overpowering in dark
- [ ] **7. Form Elements** — Inputs, selects, checkboxes styled and visible in both
- [ ] **8. Status Colors** — Success (green), warning (yellow), error (red), info (blue) distinct in both
- [ ] **9. Scrollbars** — Custom scrollbars match theme, visible but unobtrusive
- [ ] **10. Empty States** — Placeholder content visible and properly themed

---

## 9. Accessibility Quick-Check

### Keyboard Navigation
- All interactive elements reachable via Tab key
- Logical tab order follows visual layout (no tabindex > 0)
- Escape closes modals, dropdowns, popovers
- Enter/Space activates buttons and links
- Arrow keys navigate within composite widgets (tabs, menus, lists)

### ARIA Requirements
- Icon-only buttons: `aria-label="descriptive action"`
- Loading states: `aria-busy="true"` on the loading container
- Dynamic content updates: `aria-live="polite"` for non-urgent, `"assertive"` for urgent
- Custom selects: `role="listbox"` with `role="option"` children
- Modals: `role="dialog"` with `aria-modal="true"` and `aria-labelledby`
- Expandable sections: `aria-expanded` on the trigger

### Focus Management
- Modal opens: focus first focusable element inside
- Modal closes: return focus to the trigger element
- Focus trap inside modals (Tab cycles within modal boundaries)
- Skip-to-content link as first focusable element on the page
- `focus-visible:` for keyboard focus rings (not on mouse click)

### Contrast Ratios
- Body text: 4.5:1 minimum against background
- Large text (18px+ or 14px+ bold): 3:1 minimum
- UI components and graphics: 3:1 minimum
- Use theme tokens to guarantee ratios — never hardcode colors

See `references/accessibility.md` for complete WCAG AA patterns.

---

## 10. Verification Gates (Vinicius Pattern)

Every design task must pass through ALL 4 gates before completion:

### Gate 1: TypeScript Compilation
```bash
npx tsc --noEmit
```
Must pass with zero errors. No `any` types. No implicit anys.

### Gate 2: Dual-Theme Visual Verification
- Run the dev server (`npm run dev` or equivalent)
- Switch to light theme — verify all elements render correctly
- Switch to dark theme — verify all elements render correctly
- No broken layouts, missing borders, invisible text, or clipped content

### Gate 3: Keyboard Accessibility
- Tab through every interactive element in the component
- Verify focus rings are visible on all focusable elements
- Test Enter/Space activation on buttons and links
- Test Escape to close any overlays
- Verify logical tab order matches visual layout

### Gate 4: No Hardcoded Colors
- Search the component for any hex values (#fff, #000, etc.)
- Search for any rgb(), rgba(), hsl() values
- All colors must use Tailwind theme tokens or CSS variables
- Exception: glass opacity values (white/10, black/20) are acceptable

### Evidence Requirement
Completion claims MUST include one of:
- Screenshot file path showing the component in both themes
- Detailed description of visual output including specific element states
- Console output showing tsc --noEmit passed

---

## 11. Anti-Premature-Completion Protocol

**These phrases are NOT valid evidence of completion:**
- "I see the component renders correctly"
- "The styling looks good"
- "It should work in both themes"
- "The animation is smooth"
- "Everything is properly accessible"

**Valid evidence requires ACTUAL verification:**
- "Ran `tsc --noEmit` — 0 errors"
- "Dev server running at localhost:3000 — checked light theme: card backgrounds show glass blur, text contrast verified. Checked dark theme: borders visible, glass opacity correct."
- "Tabbed through all 7 interactive elements — focus ring visible on each, Enter activates buttons, Escape closes dropdown."
- "No hardcoded hex values found in component — all colors use theme tokens."

If you cannot provide actual evidence, state clearly: "I have not yet verified this visually. The user should check [specific things] in both themes."

---

## 12. 3-Strike Rule

When iterating on a design implementation:

- **Strike 1:** First implementation attempt. If it doesn't meet requirements, analyze what went wrong and try a different approach.
- **Strike 2:** Second attempt with adjusted approach. If still not right, research the specific issue (check references, look at similar components).
- **Strike 3:** Third attempt informed by research. If STILL not meeting requirements, **STOP** and ask the user for direction.

**After 3 strikes:**
- Present what you've tried and why each approach fell short
- Show the current state of the code
- Ask specific questions: "Should I try X approach, or do you have a different design in mind?"
- Do NOT continue making random changes hoping something works

---

## 13. Reference Files

For deep knowledge on specific topics, consult these reference files:

| Reference | Path | Content |
|-----------|------|---------|
| Glassmorphism | `references/glassmorphism.md` | 3-tier system, blur variants, opacity ranges, performance tips |
| Typography | `references/typography.md` | Full type scale, font stacks, responsive sizing, truncation |
| Animation | `references/animation.md` | Framer Motion variants, spring presets, CSS transitions, keyframes |
| Accessibility | `references/accessibility.md` | WCAG AA, keyboard nav, ARIA patterns, focus management |
| Layout Patterns | `references/layout-patterns.md` | Sidebar, dashboard grid, data table, form, split view, z-index |
| React Patterns | `references/react-patterns.md` | Hooks, memo, compound components, cn(), error boundaries |
| Anti-Patterns | `references/anti-patterns.md` | Generic vs Premium comparison table, common mistakes |
| Electron Desktop | `references/electron-desktop.md` | Title bar, window management, context menus, native feel |

**Always read the relevant reference file before implementing.** The SKILL.md gives you the quick-reference; the reference files give you the deep knowledge needed for correct implementation.

---

## Quick Command Reference

When this skill is invoked:
1. Identify the design task type (new component, redesign, fix, review)
2. Read relevant reference files for the task
3. Follow the 5-step Design Protocol
4. Pass all 4 Verification Gates
5. Provide actual evidence of completion

**Remember: the user does premium work. Every pixel matters. Every interaction must feel polished. There is no "good enough" — only "premium" or "not done yet."**

---

## 14. Scope Boundaries

- **MINIMUM**: Every component must work in both light and dark themes. No exceptions — if it does not render correctly in both themes, it is not done.
- **MAXIMUM**: Do not over-animate — max 3 animated elements per view. More than 3 simultaneous animations creates visual noise and degrades the premium feel.

---

### Competitive Design Generation
For critical UI decisions, spawn 3 design approaches:
1. Three agents each propose a different design direction
2. Judges evaluate against: visual hierarchy, accessibility, animation performance, theme consistency
3. Adaptive strategy: SELECT_AND_POLISH (unanimous) / FULL_SYNTHESIS (split) / REDESIGN (all below bar)
4. Use for hero sections, navigation patterns, dashboard layouts — not for every button

---

## 15. shadcn/ui + Tailwind Implementation

### Setup

```bash
# Initialize shadcn/ui (configures both shadcn and Tailwind)
npx shadcn@latest init

# Add components
npx shadcn@latest add button card dialog form table tabs

# Tailwind-only setup (Vite)
npm install -D tailwindcss @tailwindcss/vite
```

### shadcn/ui Component Patterns (Radix UI)

shadcn/ui uses Radix UI primitives — copy-paste distribution model with full TypeScript support.

**Component catalog:**
- Form & input: Button, Input, Select, Checkbox, Date Picker, Form (react-hook-form + Zod)
- Layout & navigation: Card, Tabs, Accordion, Navigation Menu
- Overlays & dialogs: Dialog, Drawer, Popover, Toast, Command palette
- Feedback & status: Alert, Progress, Skeleton
- Display: Table, Data Table, Avatar, Badge

**Usage pattern:**
```tsx
import { Button } from "@/components/ui/button"
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card"

export function Dashboard() {
  return (
    <div className="container mx-auto p-6 grid gap-6 md:grid-cols-2 lg:grid-cols-3">
      <Card className="hover:shadow-lg transition-shadow">
        <CardHeader>
          <CardTitle className="text-2xl font-bold">Analytics</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <p className="text-muted-foreground">View your metrics</p>
          <Button variant="default" className="w-full">View Details</Button>
        </CardContent>
      </Card>
    </div>
  )
}
```

### Form with Validation

```tsx
import { useForm } from "react-hook-form"
import { zodResolver } from "@hookform/resolvers/zod"
import * as z from "zod"
import { Form, FormField, FormItem, FormLabel, FormControl, FormMessage } from "@/components/ui/form"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"

const schema = z.object({
  email: z.string().email(),
  password: z.string().min(8)
})

export function LoginForm() {
  const form = useForm({
    resolver: zodResolver(schema),
    defaultValues: { email: "", password: FIXTURE_VALUE /* redacted-for-docs */ }
  })

  return (
    <Form {...form}>
      <form onSubmit={form.handleSubmit(console.log)} className="space-y-6">
        <FormField control={form.control} name="email" render={({ field }) => (
          <FormItem>
            <FormLabel>Email</FormLabel>
            <FormControl><Input type="email" {...field} /></FormControl>
            <FormMessage />
          </FormItem>
        )} />
        <Button type="submit" className="w-full">Sign In</Button>
      </form>
    </Form>
  )
}
```

### Tailwind Utility-First Styling

**Core utilities:**
- Layout: `flex`, `grid`, `gap-*`, `container`, `mx-auto`
- Spacing: `p-*`, `m-*`, `space-y-*`, `space-x-*`
- Typography: `text-*`, `font-*`, `tracking-*`, `leading-*`
- Colors: Use semantic tokens (`text-foreground`, `bg-background`, `text-muted-foreground`)
- Borders: `border`, `rounded-*`, `border-*`
- Shadows: `shadow-*`, `ring-*`

**Arbitrary values:** `w-[calc(100%-2rem)]`, `top-[117px]`, `grid-cols-[1fr_2fr]`

### Responsive Layout Patterns

Mobile-first approach — base styles for mobile, layer breakpoints upward:

```tsx
<div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
  {/* Cards automatically reflow */}
</div>
```

Breakpoints: `sm` (640px), `md` (768px), `lg` (1024px), `xl` (1280px), `2xl` (1536px)

### Dark Mode Implementation

**With next-themes or manual toggle:**
```tsx
<div className="min-h-screen bg-background text-foreground">
  <Card className="bg-card border-border">
    <CardContent className="p-6">
      <h3 className="text-xl font-semibold text-foreground">Content</h3>
      <p className="text-muted-foreground">Description</p>
    </CardContent>
  </Card>
</div>
```

**Rules:**
- Use CSS variable-based tokens (shadcn default) — theme switches automatically
- Never hardcode `bg-white` / `bg-gray-900` — use `bg-background`
- Glass layers use `bg-white/10 dark:bg-black/20` (opacity-based, theme-aware)

### Accessible Components

All shadcn/ui components inherit Radix UI accessibility:
- **Dialog:** `role="dialog"`, `aria-modal="true"`, focus trap, Escape to close
- **Dropdown Menu:** Arrow key navigation, `role="menu"`, typeahead search
- **Form:** Associated labels, error messages linked via `aria-describedby`
- **Table:** Semantic `<thead>`, `<tbody>`, `<th scope="col">` automatically
- **Tabs:** `role="tablist"`, arrow key navigation, `aria-selected`

### Tailwind Customization

```css
/* @theme directive for custom tokens */
@theme {
  --color-brand: #4191E1;
  --font-display: 'Inter', sans-serif;
}
```

```javascript
// vite.config.ts — Tailwind plugin
import tailwindcss from '@tailwindcss/vite'
export default { plugins: [tailwindcss()] }
```

### Canvas-Based Visual Design

For generative/visual design work:
- Museum-quality compositions with minimal text
- Philosophy-driven design approach — visual communication over labels
- Systematic patterns, refined color palettes, spatial harmony
- See `references/canvas-design-system.md` for complete workflow

### Reference Files

| Reference | Content |
|-----------|---------|
| `references/shadcn-components.md` | Complete component catalog with examples |
| `references/shadcn-theming.md` | CSS variables, dark mode, color customization |
| `references/shadcn-accessibility.md` | ARIA patterns, keyboard nav, focus management |
| `references/tailwind-utilities.md` | Core utility classes reference |
| `references/tailwind-responsive.md` | Breakpoints, container queries, adaptive layouts |
| `references/tailwind-customization.md` | @theme, custom utilities, plugins, layers |
| `references/canvas-design-system.md` | Visual design philosophy and canvas workflows |

### External Resources

- shadcn/ui: https://ui.shadcn.com
- Tailwind CSS: https://tailwindcss.com
- Radix UI: https://radix-ui.com
- v0 (AI UI Generator): https://v0.dev

---

## 16. Related Skills

| Skill | Purpose |
|-------|---------|
| `fireworks-performance` | Render optimization — React profiling, bundle analysis, GPU-safe animations, memory leaks |
| `fireworks-review` | UI review lens — multi-perspective code review with design-specific checks |
| `fireworks-patterns` | Component patterns — strategic code reading, pattern transfer, design pattern selection |
