# Layout Patterns — Deep Reference Guide

## Overview

Layout patterns define the structural foundation of every page and component. Correct layout
ensures content is discoverable, responsive, and consistent across all screen sizes. These
patterns are battle-tested for Electron desktop apps and responsive web layouts.

---

## Sidebar + Content Layout

The most common layout for desktop applications. Fixed sidebar with flexible content area.

```tsx
function AppLayout({ children }: { children: React.ReactNode }) {
  const [collapsed, setCollapsed] = useState(false);

  return (
    <div className="flex h-screen overflow-hidden">
      {/* Sidebar */}
      <aside
        className={cn(
          "h-full backdrop-blur-xl bg-white/10 dark:bg-black/20",
          "border-r border-white/20 flex flex-col",
          "transition-all duration-300 ease-out",
          collapsed ? "w-16" : "w-64"
        )}
      >
        <div className="flex items-center justify-between p-4">
          {!collapsed && <span className="text-lg font-semibold">App Name</span>}
          <button
            onClick={() => setCollapsed(!collapsed)}
            aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
            className="p-1.5 rounded-md hover:bg-white/10 transition-colors"
          >
            <ChevronLeft className={cn("w-4 h-4 transition-transform", collapsed && "rotate-180")} />
          </button>
        </div>
        <nav className="flex-1 overflow-y-auto p-2">
          {/* Navigation items */}
        </nav>
      </aside>

      {/* Main content */}
      <main className="flex-1 overflow-y-auto">
        <div className="p-6 max-w-7xl mx-auto">
          {children}
        </div>
      </main>
    </div>
  );
}
```

### Key Details
- Sidebar: fixed width (`w-64` = 256px), collapsible to `w-16` (64px)
- Content: `flex-1` fills remaining space
- Sidebar scrolls independently: `overflow-y-auto`
- Transition on collapse: `transition-all duration-300`
- Max content width: `max-w-7xl` prevents overly wide lines on large monitors

---

## Dashboard Grid

Responsive grid layout for dashboard widgets, KPI cards, and chart panels.

```tsx
function Dashboard() {
  return (
    <div className="p-6 space-y-6">
      {/* KPI Cards Row */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <KPICard title="Revenue" value="$12,345" change="+12%" />
        <KPICard title="Orders" value="156" change="+8%" />
        <KPICard title="Customers" value="2,345" change="+3%" />
        <KPICard title="Avg Order" value="$79.13" change="-2%" />
      </div>

      {/* Charts Row */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <div className="lg:col-span-2">
          <ChartCard title="Revenue Over Time" />
        </div>
        <div>
          <ChartCard title="Category Breakdown" />
        </div>
      </div>

      {/* Full Width Table */}
      <div className="backdrop-blur-xl bg-white/10 dark:bg-black/20 border border-white/20 shadow-lg rounded-xl overflow-hidden">
        <DataTable />
      </div>
    </div>
  );
}
```

### Responsive Breakpoints
| Screen Size | Columns | Gap |
|-------------|---------|-----|
| Mobile (<640px) | 1 column | gap-4 |
| Tablet (640-1024px) | 2 columns | gap-4 |
| Desktop (>1024px) | 3-4 columns | gap-6 |

---

## Data Table Layout

Tables require special attention for readability, scrolling, and dense data display.

```tsx
function DataTable({ columns, data }: DataTableProps) {
  return (
    <div className="overflow-x-auto">
      <table className="w-full min-w-[800px]">
        {/* Sticky Header */}
        <thead className="sticky top-0 z-10 backdrop-blur-xl bg-white/10 dark:bg-black/20 border-b border-white/20">
          <tr>
            {columns.map(col => (
              <th
                key={col.key}
                className="text-left text-sm font-medium text-muted-foreground px-4 py-3"
                style={{ width: col.width }}
              >
                {col.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="divide-y divide-white/10">
          {data.map(row => (
            <tr
              key={row.id}
              className="hover:bg-white/5 dark:hover:bg-white/[0.03] transition-colors duration-100"
            >
              {columns.map(col => (
                <td key={col.key} className="px-4 py-3 text-sm">
                  {row[col.key]}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
```

### Table Best Practices
- **Sticky header:** `sticky top-0 z-10` with glass background
- **Min-width:** Prevent columns from crushing on small screens
- **Horizontal scroll:** Wrap in `overflow-x-auto` container
- **Row hover:** Subtle `hover:bg-white/5` for row tracking
- **Column widths:** Define explicit widths for predictable layout
- **Dense mode:** Reduce padding from `py-3` to `py-2` for compact tables
- **Alignment:** Numbers right-aligned (`text-right`), text left-aligned

---

## Form Layout

Consistent form layout with validation states and responsive columns.

```tsx
function FormLayout() {
  return (
    <form className="space-y-6 max-w-2xl">
      {/* Section with heading */}
      <div className="space-y-4">
        <h3 className="text-lg font-medium">Personal Information</h3>

        {/* Two-column row (collapses to 1 on mobile) */}
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <FormField label="First Name" name="firstName" required />
          <FormField label="Last Name" name="lastName" required />
        </div>

        {/* Full-width field */}
        <FormField label="Email" name="email" type="email" required />

        {/* Textarea */}
        <FormField label="Notes" name="notes" type="textarea" rows={4} />
      </div>

      {/* Action buttons */}
      <div className="flex items-center justify-end gap-3 pt-4 border-t border-white/10">
        <button type="button" className="px-4 py-2 text-sm font-medium rounded-lg hover:bg-accent transition-colors">
          Cancel
        </button>
        <button type="submit" className="px-4 py-2 text-sm font-medium bg-primary text-primary-foreground rounded-lg hover:brightness-110 active:scale-[0.98] transition-all">
          Save
        </button>
      </div>
    </form>
  );
}
```

### Form Spacing Rules
- Label above input (never beside on mobile)
- `gap-4` between fields within a section
- `space-y-6` between sections
- Consistent `px-3 py-2` padding on all inputs
- Inline validation appears immediately below the field
- Two-column forms collapse to single column below `sm` breakpoint

---

## Split View (Resizable Panels)

For master-detail views, code editors, or side-by-side comparisons.

```tsx
function SplitView({ left, right }: SplitViewProps) {
  const [leftWidth, setLeftWidth] = useState(50); // percentage

  return (
    <div className="flex h-full">
      {/* Left panel */}
      <div style={{ width: `${leftWidth}%` }} className="min-w-[250px] overflow-auto">
        {left}
      </div>

      {/* Drag handle */}
      <div
        className="w-1 cursor-col-resize bg-border hover:bg-primary/50 transition-colors flex-shrink-0"
        onMouseDown={startResize}
      />

      {/* Right panel */}
      <div style={{ width: `${100 - leftWidth}%` }} className="min-w-[250px] overflow-auto">
        {right}
      </div>
    </div>
  );
}
```

### Split View Rules
- **Min-width constraints:** Prevent panels from becoming unusably small
- **Drag handle:** Visible 4px handle with hover highlight
- **Cursor:** `cursor-col-resize` on the handle
- **Persist size:** Save split ratio to localStorage

---

## Card Grid (Auto-Fill)

Self-adjusting grid that fills available space with consistent card sizes.

```tsx
function CardGrid({ items }: CardGridProps) {
  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(280px,1fr))] gap-4">
      {items.map(item => (
        <div
          key={item.id}
          className="backdrop-blur-xl bg-white/10 dark:bg-black/20 border border-white/20 shadow-lg rounded-xl p-4 flex flex-col transition-all duration-200 hover:shadow-xl hover:bg-white/[0.12]"
        >
          <h4 className="text-lg font-medium">{item.title}</h4>
          <p className="text-sm text-muted-foreground mt-1 flex-1">{item.description}</p>
          <div className="mt-4 pt-3 border-t border-white/10">
            {item.actions}
          </div>
        </div>
      ))}
    </div>
  );
}
```

### Card Grid Details
- `auto-fill` with `minmax(280px, 1fr)` — cards never go below 280px
- All cards equal height via `flex flex-col` with `flex-1` on expandable content
- Consistent gap: `gap-4` for tight grids, `gap-6` for spacious grids

---

## Virtualization (Large Lists)

For lists with 1000+ items, use virtualization to maintain smooth scrolling.

```tsx
import { FixedSizeList } from 'react-window';

function VirtualizedList({ items }: { items: Item[] }) {
  const Row = ({ index, style }: { index: number; style: React.CSSProperties }) => (
    <div style={style} className="flex items-center px-4 border-b border-white/10 hover:bg-white/5 transition-colors">
      <span className="text-sm">{items[index].name}</span>
    </div>
  );

  return (
    <FixedSizeList
      height={600}
      width="100%"
      itemCount={items.length}
      itemSize={48}
      overscanCount={5}
    >
      {Row}
    </FixedSizeList>
  );
}
```

### Virtualization Rules
- Use `react-window` for fixed row height lists
- Use `react-virtuoso` for variable height lists
- `overscanCount={5}` — render 5 extra rows above/below viewport
- Fixed row height = better performance than variable height
- Wrap in a container with explicit height

---

## Z-Index Scale

Consistent z-index values prevent stacking context conflicts:

| Level | Z-Index | Tailwind Class | Use For |
|-------|---------|---------------|---------|
| Base | 0 | `z-0` | Default content |
| Raised | 1 | `z-[1]` | Slightly elevated elements |
| Dropdown | 10 | `z-10` | Dropdowns, popovers, menus |
| Sticky | 20 | `z-20` | Sticky headers, fixed toolbars |
| Modal Backdrop | 30 | `z-30` | Modal/dialog overlay |
| Modal | 40 | `z-40` | Modal/dialog content |
| Toast | 50 | `z-50` | Toast notifications |
| Tooltip | 60 | `z-[60]` | Tooltips (always on top) |
| Dev Tools | 9999 | `z-[9999]` | Debug overlays only |

### Rules
- Never use arbitrary z-index values outside this scale
- Modals are ALWAYS above sticky headers
- Toasts are ALWAYS above modals
- Tooltips are ALWAYS above everything
- Create new stacking contexts with `isolation-isolate` when needed
