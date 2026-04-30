---
name: fireworks-react
version: 1.0.0
author: mneme
description: Use when building React components, optimizing renders, managing state with Zustand, implementing error boundaries, using hooks, Server Components, Suspense, concurrent features, or reviewing React PRs. Covers React 18/19, Zustand, Tailwind cn(), Framer Motion, TypeScript strict mode.
triggers:
  - react
  - component
  - hook
  - useState
  - useEffect
  - zustand
  - suspense
  - error boundary
  - server component
  - jsx
  - tsx
tags:
  - react
  - frontend
  - hooks
  - zustand
  - typescript
  - jsx
  - tsx
  - components
  - suspense
  - error-boundary
---

# FIREWORKS-REACT -- Enterprise React 18/19 Superbrain

> The definitive React skill for premium software projects.
> Absorbs and supersedes the base `react` skill with 3x coverage.

---

## 1. Development Protocol

Every React task follows this pipeline -- no exceptions:

```
ANALYZE --> ARCHITECT --> IMPLEMENT --> TYPE-CHECK --> VERIFY --> SHIP
```

### Step-by-Step Pipeline

1. **ANALYZE** -- Read the requirement. Identify: new component? refactor? performance fix? state change?
2. **ARCHITECT** -- Decide component boundaries, state ownership, data flow direction
3. **IMPLEMENT** -- Write code following all patterns in this skill
4. **TYPE-CHECK** -- Run `tsc --noEmit` -- zero errors before proceeding
5. **VERIFY** -- Visual check in BOTH light and dark themes. Test interactive states.
6. **SHIP** -- Only after all gates pass

### Pre-Flight Checklist (Before Writing ANY Component)

- [ ] Where does state live? (local, Zustand, lifted, server)
- [ ] Does this need an error boundary?
- [ ] Does this need lazy loading?
- [ ] Are there existing components to compose with?
- [ ] Does the design need dark mode variants?
- [ ] Are animations required? (Framer Motion vs CSS)
- [ ] Will this component receive callbacks? (memoization needed?)

---

## 2. Component Architecture Decision Tree

```
What are you building?
|
+-- Pure display, no state? ---------> Functional component (simplest)
|
+-- Receives callbacks from parent? --> React.memo wrapper (prevent re-renders)
|
+-- Heavy/rarely-used route? --------> React.lazy + Suspense boundary
|
+-- Async data loading? -------------> Suspense + use() (React 19) or useAsync hook
|
+-- Can crash from bad data? --------> Wrap in ErrorBoundary
|
+-- Renders into document.body? -----> createPortal (modals, toasts, tooltips)
|
+-- Shares implicit state? ----------> Compound component pattern
|
+-- Reusable across types? ----------> Generic component with <T>
```

### Component File Structure (recommended)

```
components/
  ProductCard/
    ProductCard.tsx        # Component implementation
    ProductCard.test.tsx   # Tests
    index.ts               # Named export barrel
```

### Export Pattern

```tsx
// ProductCard.tsx
export function ProductCard({ product }: ProductCardProps) { ... }

// index.ts -- named export, NEVER default
export { ProductCard } from './ProductCard';
export type { ProductCardProps } from './ProductCard';
```

### Compound Component Pattern

```tsx
const AccordionContext = createContext<{
  openItems: Set<string>;
  toggle: (id: string) => void;
} | null>(null);

function useAccordionContext() {
  const ctx = useContext(AccordionContext);
  if (!ctx) throw new Error('Accordion.Item must be inside Accordion');
  return ctx;
}

export function Accordion({ children, multiple = false }: AccordionProps) {
  const [openItems, setOpenItems] = useState<Set<string>>(new Set());
  const toggle = useCallback((id: string) => {
    setOpenItems(prev => {
      const next = new Set(multiple ? prev : []);
      if (prev.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  }, [multiple]);

  return (
    <AccordionContext.Provider value={{ openItems, toggle }}>
      <div className="divide-y divide-white/10">{children}</div>
    </AccordionContext.Provider>
  );
}

Accordion.Item = function AccordionItem({ id, title, children }: AccordionItemProps) {
  const { openItems, toggle } = useAccordionContext();
  const isOpen = openItems.has(id);
  return (
    <div>
      <button onClick={() => toggle(id)} className="w-full text-left p-3">
        {title}
      </button>
      <AnimatePresence>
        {isOpen && (
          <motion.div initial={{ height: 0 }} animate={{ height: 'auto' }} exit={{ height: 0 }}>
            <div className="p-3">{children}</div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
```

---

---

## Full Reference

For complete patterns, examples, and advanced usage, see [`references/full-guide.md`](./references/full-guide.md).
Read that file when you need deeper context than the summary above.
