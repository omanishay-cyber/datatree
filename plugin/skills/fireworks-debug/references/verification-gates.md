# Verification Gates — Full Reference

> Every fix MUST pass through all 6 gates. No gate can be skipped.
> No gate can be assumed. Every gate requires EVIDENCE.

---

## Gate 1: STATIC ANALYSIS

### Command
```bash
npx tsc --noEmit
```

### What Counts As Pass
- Zero new errors introduced by your changes.
- Pre-existing errors are acceptable ONLY if they existed before your change. Verify by checking the error locations — if the error is in a file you did not touch, it is pre-existing.
- If your change introduces even ONE new TypeScript error, the gate fails.

### How To Handle Pre-Existing Errors
- Count the errors before your change (if possible, check the last known count).
- Count the errors after your change.
- If the count increased, you introduced new errors — fix them.
- If the count is the same or decreased, you pass this gate.
- Never suppress errors with `@ts-ignore` or `as any` to pass this gate.

### Evidence Format
```
GATE 1 — STATIC ANALYSIS: PASS
- Command: `npx tsc --noEmit`
- Result: 0 errors (or: N pre-existing errors, 0 new)
- New errors introduced: none
```

---

## Gate 2: DEV SERVER

### Command
```bash
npm run dev
```

### What To Check
1. **Terminal output**: The dev server starts without crashing. No error messages in the terminal.
2. **Vite output**: Look for `ready in Xms` or equivalent success message.
3. **Electron output**: The BrowserWindow opens and displays content.
4. **Browser/Renderer console**: Open DevTools (Ctrl+Shift+I), check the Console tab. No red errors.
5. **Main process console**: Check the terminal where `npm run dev` is running. No uncaught exceptions.

### Common Failures
- Port already in use: kill the old process or use a different port.
- Module not found: check your imports, run `npm install` if needed.
- Syntax error: `tsc --noEmit` should have caught this — go back to Gate 1.
- Environment variable missing: check `.env` file.

### Evidence Format
```
GATE 2 — DEV SERVER: PASS
- Command: `npm run dev`
- Terminal: Server started successfully, no errors
- Renderer console: No errors (checked DevTools Console)
- Main process: No uncaught exceptions
```

---

## Gate 3: CONTENT VERIFICATION

### What This Gate Means
This is the most commonly skipped gate and the #1 cause of "false done" declarations.

"No errors" does NOT mean "correct." A page can render without errors but show wrong data, missing elements, broken layouts, or stale content.

### What To Check
1. Navigate to the affected area of the app.
2. Verify the ACTUAL content is correct:
   - Are the right items displayed?
   - Is the data accurate?
   - Are the correct labels/titles showing?
   - Is the layout correct?
   - Do interactive elements respond correctly?
3. Verify the content matches expectations:
   - If you fixed a display bug, is the display now correct (not just "not broken")?
   - If you added a feature, does it produce the correct output?
   - If you changed data processing, is the processed data correct?

### Bad vs Good Evidence
- BAD: "The page loads without errors." (This is Gate 2, not Gate 3.)
- BAD: "I made the change and it should work." (No verification at all.)
- GOOD: "The product list shows 5 items: Wine, Beer, Vodka, Rum, Gin — matching the database."
- GOOD: "The total calculation shows $127.50, which matches the manual calculation of (3 x $25.00) + (1 x $52.50)."
- GOOD: "The search returns 3 results for 'cabernet' — previously it returned 0."

### Evidence Format
```
GATE 3 — CONTENT VERIFICATION: PASS
- Area checked: [specific page/component]
- Expected: [what should appear]
- Observed: [what actually appeared]
- Match: yes
```

---

## Gate 4: ROUTE HEALTH

### What To Check
1. List ALL routes/pages affected by your change.
2. Navigate to EACH affected route.
3. Verify each renders correctly — no blank pages, no missing components, no layout breaks.
4. If your change affects a shared component (header, sidebar, layout), check ALL routes that use it.

### How To Identify Affected Routes
- Direct: routes that use the modified component/function directly.
- Indirect: routes that use a shared component you modified.
- Global: if you modified a store, context, or utility, check routes that consume them.

### Evidence Format
```
GATE 4 — ROUTE HEALTH: PASS
- Affected routes:
  - /dashboard: renders correctly, all widgets load
  - /products: product list displays, search works
  - /settings: settings page loads, form is interactive
- Shared components checked: Header (visible on all routes), Sidebar (navigation works)
```

---

## Gate 5: PLAN AUDIT

### What This Gate Means
Re-read the ORIGINAL task description (not your memory of it). Check off every deliverable with specific evidence.

### Process
1. Scroll up to the original task/bug report.
2. Copy each requirement or deliverable.
3. For each deliverable, cite the specific evidence that it is complete.
4. If ANY deliverable lacks evidence, the gate fails — go back and complete it.

### Common Failures
- Partially completed: you fixed 3 of 4 issues but missed the 4th.
- Scope drift: you fixed a different issue than what was requested.
- Assumed completion: you made the change but did not verify the outcome.

### Evidence Format
```
GATE 5 — PLAN AUDIT: PASS
- Original task: [restate the task]
- Deliverables:
  1. [deliverable 1]: DONE — [evidence]
  2. [deliverable 2]: DONE — [evidence]
  3. [deliverable 3]: DONE — [evidence]
- All deliverables accounted for: yes
```

---

## Gate 6: END-TO-END

### What This Gate Means
Test the complete user flow from start to finish, as the user would experience it.

### Process
1. Describe the user flow in steps: "User opens app -> clicks X -> enters Y -> sees Z."
2. Execute each step.
3. Verify the intermediate state at each step.
4. Verify the final state matches expectations.
5. For visual features: check BOTH light and dark themes.

### What Makes A Good E2E Test
- Starts from the user's entry point (not from the middle of a flow).
- Covers the happy path (expected input, expected output).
- Covers at least one edge case (empty input, boundary value, error state).
- Ends with verification of the final state.

### Evidence Format
```
GATE 6 — E2E: PASS
- User flow:
  1. Open the app -> dashboard loads
  2. Navigate to Products -> product list renders with 5 items
  3. Click "Add Product" -> form appears
  4. Fill in name "Test Wine", price "25.00" -> click Save
  5. Product appears in list with correct name and price
  6. Click product -> detail view shows correct information
- Edge case: Empty name -> validation error appears (correct)
- Dark mode: verified, all elements visible and styled
- Final state: product successfully added and viewable
```

---

## Complete Evidence Template

Copy this template for every debugging session:

```
=== VERIFICATION REPORT ===

Task: [restate the original task/bug]
Date: [date]

GATE 1 — STATIC ANALYSIS: [PASS/FAIL]
- Command: `npx tsc --noEmit`
- New errors: [0 / list them]

GATE 2 — DEV SERVER: [PASS/FAIL]
- Server starts: [yes/no]
- Console errors: [none / list them]

GATE 3 — CONTENT VERIFICATION: [PASS/FAIL]
- Area: [what was checked]
- Expected: [what should appear]
- Observed: [what appeared]

GATE 4 — ROUTE HEALTH: [PASS/FAIL]
- Routes checked: [list]
- All render correctly: [yes/no]

GATE 5 — PLAN AUDIT: [PASS/FAIL]
- Deliverables: [list with evidence]
- All complete: [yes/no]

GATE 6 — E2E: [PASS/FAIL]
- Flow: [step-by-step]
- Result: [final state]

OVERALL: [ALL GATES PASS / GATE X FAILED]
=== END REPORT ===
```
