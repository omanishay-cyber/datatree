# Animation — Deep Reference Guide

## Overview

Animation brings premium feel to every interaction. The right animation makes an app feel
responsive, alive, and polished. The wrong animation makes it feel sluggish or distracting.
This guide provides ready-to-use animation patterns for every common UI scenario.

---

## Framer Motion Variants Library

### Page Transitions

```tsx
import { motion, AnimatePresence } from 'framer-motion';

// Fade In
const fadeIn = {
  initial: { opacity: 0 },
  animate: { opacity: 1 },
  exit: { opacity: 0 },
  transition: { duration: 0.2 }
};

// Slide Up (most common for pages)
const slideUp = {
  initial: { opacity: 0, y: 20 },
  animate: { opacity: 1, y: 0 },
  exit: { opacity: 0, y: -10 },
  transition: { duration: 0.3, ease: 'easeOut' }
};

// Slide Right (for drill-in navigation)
const slideRight = {
  initial: { opacity: 0, x: -20 },
  animate: { opacity: 1, x: 0 },
  exit: { opacity: 0, x: 20 },
  transition: { duration: 0.25, ease: 'easeOut' }
};

// Usage with AnimatePresence
function PageWrapper({ children, key }: { children: React.ReactNode; key: string }) {
  return (
    <AnimatePresence mode="wait">
      <motion.div key={key} {...slideUp}>
        {children}
      </motion.div>
    </AnimatePresence>
  );
}
```

### Stagger Children

```tsx
const staggerContainer = {
  initial: {},
  animate: {
    transition: {
      staggerChildren: 0.05,
      delayChildren: 0.1
    }
  }
};

const staggerItem = {
  initial: { opacity: 0, y: 10 },
  animate: {
    opacity: 1,
    y: 0,
    transition: { duration: 0.2, ease: 'easeOut' }
  }
};

// Usage
function StaggeredList({ items }: { items: Item[] }) {
  return (
    <motion.ul variants={staggerContainer} initial="initial" animate="animate">
      {items.map(item => (
        <motion.li key={item.id} variants={staggerItem}>
          {item.content}
        </motion.li>
      ))}
    </motion.ul>
  );
}
```

### Modal Animation

```tsx
const modalBackdrop = {
  initial: { opacity: 0 },
  animate: { opacity: 1 },
  exit: { opacity: 0 },
  transition: { duration: 0.2 }
};

const modalContent = {
  initial: { opacity: 0, scale: 0.95, y: 10 },
  animate: {
    opacity: 1,
    scale: 1,
    y: 0,
    transition: { type: 'spring', stiffness: 300, damping: 25 }
  },
  exit: {
    opacity: 0,
    scale: 0.95,
    y: 10,
    transition: { duration: 0.15 }
  }
};

function Modal({ isOpen, onClose, children }: ModalProps) {
  return (
    <AnimatePresence>
      {isOpen && (
        <>
          <motion.div
            className="fixed inset-0 bg-black/40 backdrop-blur-sm z-30"
            onClick={onClose}
            {...modalBackdrop}
          />
          <motion.div
            className="fixed inset-0 flex items-center justify-center z-40 p-4"
            {...modalContent}
          >
            <div className="backdrop-blur-2xl bg-white/15 dark:bg-black/30 border border-white/25 shadow-2xl rounded-2xl p-6 w-full max-w-lg">
              {children}
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}
```

### List Item Animations

```tsx
const listItemVariants = {
  initial: { opacity: 0, x: -10 },
  animate: {
    opacity: 1,
    x: 0,
    transition: { duration: 0.2 }
  },
  exit: {
    opacity: 0,
    x: 10,
    transition: { duration: 0.15 }
  }
};

// Animate individual items entering/leaving
function AnimatedList({ items }: { items: Item[] }) {
  return (
    <AnimatePresence>
      {items.map((item, i) => (
        <motion.div
          key={item.id}
          variants={listItemVariants}
          initial="initial"
          animate="animate"
          exit="exit"
          transition={{ delay: i * 0.05 }}
        >
          {item.content}
        </motion.div>
      ))}
    </AnimatePresence>
  );
}
```

### Layout Animations (Shared Element Transitions)

```tsx
// Tab indicator that slides between tabs
function TabBar({ tabs, activeTab }: TabBarProps) {
  return (
    <div className="relative flex gap-1 p-1 backdrop-blur-xl bg-white/10 dark:bg-black/20 border border-white/20 rounded-lg">
      {tabs.map(tab => (
        <button
          key={tab.id}
          onClick={() => setActiveTab(tab.id)}
          className="relative px-4 py-2 text-sm font-medium z-10"
        >
          {activeTab === tab.id && (
            <motion.div
              layoutId="activeTab"
              className="absolute inset-0 bg-primary/20 rounded-md"
              transition={{ type: 'spring', stiffness: 300, damping: 25 }}
            />
          )}
          {tab.label}
        </button>
      ))}
    </div>
  );
}
```

### Expand/Collapse Animation

```tsx
function Accordion({ title, children }: AccordionProps) {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <div className="border border-white/20 rounded-lg overflow-hidden">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="w-full flex items-center justify-between p-4"
      >
        <span className="font-medium">{title}</span>
        <motion.span
          animate={{ rotate: isOpen ? 180 : 0 }}
          transition={{ duration: 0.2 }}
        >
          <ChevronDown className="w-4 h-4" />
        </motion.span>
      </button>
      <AnimatePresence>
        {isOpen && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.2, ease: 'easeInOut' }}
          >
            <div className="px-4 pb-4">{children}</div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
```

---

## Spring Presets

| Preset | Stiffness | Damping | Mass | Feel | Use For |
|--------|-----------|---------|------|------|---------|
| **Gentle** | 120 | 14 | 1 | Slow, smooth, no bounce | Page transitions, modals |
| **Snappy** | 300 | 20 | 1 | Quick, minimal bounce | Tab indicators, toggles |
| **Bouncy** | 400 | 10 | 1 | Fast with visible bounce | Notifications, badges, attention |
| **Stiff** | 500 | 30 | 1 | Very quick, no bounce | Micro-interactions, tooltips |

```tsx
// Define as reusable presets
const springPresets = {
  gentle: { type: 'spring' as const, stiffness: 120, damping: 14 },
  snappy: { type: 'spring' as const, stiffness: 300, damping: 20 },
  bouncy: { type: 'spring' as const, stiffness: 400, damping: 10 },
  stiff:  { type: 'spring' as const, stiffness: 500, damping: 30 },
};
```

---

## CSS Transitions (Simple State Changes)

### Hover Effects
```css
/* Standard hover transition */
.interactive {
  @apply transition-all duration-200 ease-out;
}

/* Color-only hover (faster) */
.color-change {
  @apply transition-colors duration-200;
}

/* Subtle lift on hover */
.lift-hover {
  @apply transition-all duration-200 hover:-translate-y-0.5 hover:shadow-lg;
}
```

### Button Press
```css
/* Scale down on press */
.press-scale {
  @apply active:scale-[0.98] transition-transform duration-100;
}

/* Full button interaction set */
.premium-button {
  @apply transition-all duration-200 ease-out
         hover:brightness-110 hover:shadow-lg
         active:scale-[0.98] active:brightness-95
         focus-visible:ring-2 focus-visible:ring-primary/50 focus-visible:ring-offset-2;
}
```

### Focus Ring
```css
/* Keyboard-only focus ring */
.focus-ring {
  @apply focus-visible:ring-2 focus-visible:ring-primary/50
         focus-visible:ring-offset-2 focus-visible:ring-offset-background
         transition-shadow duration-150;
}
```

### Shimmer Loading (Skeleton)
```css
@keyframes shimmer {
  0% { background-position: -200% 0; }
  100% { background-position: 200% 0; }
}

.skeleton {
  @apply bg-gradient-to-r from-muted via-muted-foreground/10 to-muted
         bg-[length:200%_100%] animate-[shimmer_1.5s_ease-in-out_infinite]
         rounded-md;
}
```

```tsx
// Skeleton component
function Skeleton({ className }: { className?: string }) {
  return (
    <div className={cn(
      "bg-gradient-to-r from-muted via-muted-foreground/10 to-muted",
      "bg-[length:200%_100%] animate-[shimmer_1.5s_ease-in-out_infinite]",
      "rounded-md",
      className
    )} />
  );
}

// Usage
<Skeleton className="h-4 w-48" />  // Text line
<Skeleton className="h-10 w-full" /> // Input
<Skeleton className="h-32 w-full rounded-xl" /> // Card
```

### Color Transitions
```css
/* Smooth theme color changes */
.theme-transition {
  @apply transition-colors duration-200;
}

/* Background + text color together */
.state-transition {
  @apply transition-[background-color,color,border-color] duration-200;
}
```

---

## GPU-Safe Animation Properties

### Safe to Animate (GPU Composited)
- `transform` (translate, scale, rotate, skew)
- `opacity`
- `filter` (blur, brightness, etc.)
- `backdrop-filter`
- `clip-path`

### Avoid Animating (Triggers Layout Reflow)
- `width`, `height`
- `top`, `right`, `bottom`, `left`
- `margin`, `padding`
- `border-width`
- `font-size`
- `line-height`

### Workarounds
```tsx
// BAD: animating height
<motion.div animate={{ height: isOpen ? 200 : 0 }} />

// GOOD: animating scaleY (GPU composited)
<motion.div
  animate={{ scaleY: isOpen ? 1 : 0 }}
  style={{ transformOrigin: 'top' }}
/>

// ACCEPTABLE: animating height with layout animation (Framer handles optimization)
<motion.div animate={{ height: 'auto' }} />
```

---

## Reduced Motion Accessibility

Always respect users who prefer reduced motion:

```tsx
// Tailwind utility classes
<div className="motion-reduce:transition-none motion-reduce:animate-none">
  {/* Content with motion-safe defaults */}
</div>

// Framer Motion: check preference
import { useReducedMotion } from 'framer-motion';

function AnimatedComponent() {
  const prefersReducedMotion = useReducedMotion();

  return (
    <motion.div
      animate={{ opacity: 1, y: 0 }}
      transition={prefersReducedMotion ? { duration: 0 } : { duration: 0.3 }}
    >
      {content}
    </motion.div>
  );
}
```

### Motion-Safe Pattern
```tsx
// Only animate when user allows motion
<div className="motion-safe:animate-bounce motion-safe:transition-all">
  {/* Animated content */}
</div>
```

---

## Animation Timing Guidelines

| Interaction Type | Duration | Easing | Rationale |
|-----------------|----------|--------|-----------|
| Hover/Focus | 150-200ms | ease-out | Instant feedback needed |
| Button press | 100ms | ease-out | Must feel tactile |
| Dropdown open | 200ms | ease-out | Quick but visible |
| Modal open | 200-300ms | spring | Important transition, draws attention |
| Modal close | 150ms | ease-in | Exit should be faster than enter |
| Page transition | 200-300ms | ease-out | Smooth but not slow |
| Stagger delay | 30-50ms | - | Enough to perceive sequence |
| Loading skeleton | 1.5s loop | ease-in-out | Slow enough to not distract |
| Toast appear | 300ms | spring (bouncy) | Attract attention |
| Toast dismiss | 200ms | ease-in | Quick exit |

**Golden rule: Enter animations are slower than exit animations. Users want to see new content arrive but do not want to wait for old content to leave.**
