# Task Templates — Fireworks Taskmaster

Detailed task templates with full checklists, step-by-step procedures, and examples for each task type.

---

## Bug Fix Template

### Overview

Bug fixes follow a disciplined reproduce-first, test-first workflow. Never jump to fixing before you can reliably reproduce the bug and have a test that proves it exists.

### Full Procedure

```
PHASE 1: REPRODUCE
  1. Read the bug report carefully — what is expected vs. actual?
  2. Reproduce the bug in the current environment
  3. Document exact reproduction steps:
     - Starting state
     - Actions taken (in order)
     - Expected result
     - Actual result
     - Environment (OS, browser, Node version, etc.)
  4. If cannot reproduce: ask for more details, check if already fixed

PHASE 2: WRITE FAILING TEST
  5. Write a test that exercises the exact failing scenario
  6. Run the test — confirm it FAILS (red)
  7. If the test passes, your test is wrong or the bug is elsewhere

PHASE 3: ROOT CAUSE ANALYSIS
  8. Trace execution from the entry point to the failure
  9. Identify the ROOT cause — not just where it crashes
  10. Ask: "Why does this happen?" at least 3 times (5 Whys technique)
  11. Document the root cause in a brief sentence

PHASE 4: IMPLEMENT FIX
  12. Fix the root cause (not the symptoms)
  13. Keep the fix minimal — change as few lines as possible
  14. Run the failing test — confirm it PASSES (green)
  15. Run the full test suite — confirm no regressions

PHASE 5: VERIFY
  16. Manually reproduce original steps — bug is gone
  17. Check edge cases near the fix
  18. Run tsc --noEmit
  19. Check both light and dark themes (if UI-related)

PHASE 6: DOCUMENT
  20. Write commit message explaining root cause and fix
  21. Update any relevant documentation
  22. If bug was in a pattern used elsewhere, check for similar bugs
```

### Bug Fix Task Template (YAML)

```yaml
id: T-XXX
subject: "Fix: [brief description of the bug]"
type: bug
priority: must  # bugs are usually must or should
estimate: 25min
acceptance_criteria:
  - Bug no longer reproduces with original steps
  - Failing test now passes
  - No regressions in test suite
  - Root cause documented in commit message
steps:
  - Reproduce the bug (capture exact steps)
  - Write a failing test
  - Identify root cause (use 5 Whys)
  - Implement minimal fix
  - Verify test passes and no regressions
  - Document root cause in commit
definition_of_done:
  - [ ] Bug reproduced and documented
  - [ ] Failing test written and confirmed red
  - [ ] Root cause identified (not just symptoms)
  - [ ] Fix implemented targeting root cause
  - [ ] Failing test now passes (green)
  - [ ] Full test suite passes (no regressions)
  - [ ] Manual verification complete
  - [ ] tsc --noEmit passes
  - [ ] Both themes checked (if UI bug)
  - [ ] Root cause documented in commit message
```

### Example Bug Fix Task

```yaml
id: T-012
subject: "Fix: Product search returns stale results after category change"
type: bug
priority: must
estimate: 25min
description: |
  When user switches category in the product list, the search input
  retains its value but the results don't re-filter. User sees products
  from the old category that match the search term.
root_cause: |
  useEffect dependency array for search query doesn't include
  selectedCategory. The search runs on mount and when query changes,
  but not when category changes.
acceptance_criteria:
  - Changing category re-runs search with current query
  - Search results always match both query AND category
  - No flash of stale results during transition
```

---

## Feature Template

### Overview

Features follow a design-first, TDD workflow. Understand what you are building, plan the approach, then implement test-first.

### Full Procedure

```
PHASE 1: CLARIFY
  1. Restate the feature in your own words
  2. List acceptance criteria (get user confirmation)
  3. Identify affected components and files
  4. List unknowns — create research tasks if needed
  5. Check for similar existing features to follow patterns

PHASE 2: DESIGN
  6. Sketch component structure (parent/child, data flow)
  7. Define state shape (Zustand store or local state)
  8. Define IPC channels if Electron main process involved
  9. Plan CSS approach (Tailwind classes, custom properties)
  10. Identify reusable components vs. new components

PHASE 3: IMPLEMENT (TDD)
  11. Write first test for simplest acceptance criterion
  12. Implement minimum code to pass the test
  13. Refactor if needed (keep tests green)
  14. Repeat for each acceptance criterion
  15. Add error handling and edge cases

PHASE 4: INTEGRATE
  16. Wire component into the application
  17. Add routing if needed
  18. Connect to state management
  19. Add IPC handlers if needed

PHASE 5: VERIFY
  20. Run tsc --noEmit
  21. Check light theme — all elements visible and styled
  22. Check dark theme — all elements visible and styled
  23. Check responsive behavior (if applicable)
  24. Check keyboard navigation and focus states
  25. Manual smoke test of the full feature flow

PHASE 6: POLISH
  26. Add transitions and animations
  27. Add loading states
  28. Add error states
  29. Check accessibility basics (labels, contrast, focus)
```

### Feature Task Template (YAML)

```yaml
id: T-XXX
subject: "Add: [brief description of the feature]"
type: feature
priority: must | should | could
estimate: 25min
acceptance_criteria:
  - [Criterion 1 — concrete, verifiable]
  - [Criterion 2]
  - [Criterion 3]
steps:
  - Clarify acceptance criteria
  - Design component structure and data flow
  - Implement with TDD
  - Integrate into application
  - Verify both themes and TypeScript
  - Polish (transitions, loading, error states)
definition_of_done:
  - [ ] All acceptance criteria met
  - [ ] tsc --noEmit passes
  - [ ] Light theme verified
  - [ ] Dark theme verified
  - [ ] No console errors or warnings
  - [ ] Loading states handled
  - [ ] Error states handled
  - [ ] Transitions smooth (no jank)
  - [ ] Keyboard accessible
```

### Example Feature Task

```yaml
id: T-020
subject: "Add: Quick-add product button in inventory toolbar"
type: feature
priority: should
estimate: 30min
description: |
  Add a prominent "+" button to the inventory toolbar that opens
  a minimal form for adding a product (name, price, quantity only).
  Full details can be edited later.
acceptance_criteria:
  - "+" button visible in inventory toolbar
  - Clicking opens a compact form overlay
  - Form has name, price, quantity fields
  - Submit adds product to database and refreshes list
  - Cancel closes form without changes
  - Form validates required fields
```

---

## Refactor Template

### Overview

Refactoring changes code structure without changing behavior. The key discipline is characterization tests — capture what the code does now, then reshape it while keeping those tests green.

### Full Procedure

```
PHASE 1: IDENTIFY
  1. Name the specific code smell or structural problem
  2. Explain why it matters (maintenance cost, bug risk, performance)
  3. Define the target state — what should the code look like after?
  4. Identify the scope — which files and functions are affected?

PHASE 2: CHARACTERIZE
  5. Write characterization tests that capture CURRENT behavior
     - Focus on inputs and outputs, not implementation
     - Cover normal paths, edge cases, and error paths
  6. Run tests — all must PASS (they test what IS, not what SHOULD BE)
  7. If existing tests cover the area well, document which ones

PHASE 3: REFACTOR
  8. Make ONE small structural change
  9. Run tests — must still pass
  10. Commit (or save checkpoint)
  11. Repeat steps 8-10 until target state reached
  12. Never change behavior and structure in the same step

PHASE 4: VERIFY
  13. All characterization tests pass
  14. All existing tests pass
  15. tsc --noEmit passes
  16. Manual smoke test of affected features
  17. Check both themes if UI was touched

PHASE 5: CLEAN UP
  18. Remove any temporary scaffolding
  19. Remove characterization tests that duplicate existing tests
  20. Update imports and re-exports
  21. Run final full test suite
```

### Refactor Task Template (YAML)

```yaml
id: T-XXX
subject: "Refactor: [what is being improved]"
type: refactor
priority: should | could
estimate: 25min
code_smell: "[Name the smell: duplication, long method, feature envy, etc.]"
target_state: "[What the code should look like after]"
acceptance_criteria:
  - Behavior is unchanged
  - Code smell is eliminated
  - All tests pass
  - TypeScript compiles cleanly
steps:
  - Identify smell and define target state
  - Write characterization tests
  - Refactor in small verified steps
  - Verify all tests pass
  - Clean up temporary scaffolding
definition_of_done:
  - [ ] Code smell identified and documented
  - [ ] Characterization tests written and passing
  - [ ] Refactoring complete — target state achieved
  - [ ] All characterization tests still pass
  - [ ] All existing tests still pass
  - [ ] tsc --noEmit passes
  - [ ] No behavior changes (verified manually)
  - [ ] Temporary scaffolding removed
  - [ ] Both themes checked (if UI refactor)
```

### Example Refactor Task

```yaml
id: T-035
subject: "Refactor: Extract product validation into shared utility"
type: refactor
priority: should
estimate: 20min
code_smell: "Duplication — product validation logic repeated in 3 components"
target_state: |
  Single validateProduct() function in src/utils/validation.ts
  used by AddProduct, EditProduct, and ImportProducts components.
acceptance_criteria:
  - All 3 components use shared validateProduct()
  - Validation behavior identical to current
  - No duplicated validation logic remains
```

---

## Research Template

### Overview

Research tasks answer specific questions before implementation begins. They prevent guessing and produce documented decisions with evidence.

### Full Procedure

```
PHASE 1: DEFINE
  1. Write the specific question to be answered
  2. Define what a "good answer" looks like (criteria)
  3. Set a time box (research can expand infinitely — cap it)
  4. List what you already know (avoid re-researching)

PHASE 2: SEARCH
  5. Check project documentation and memory files first
  6. Search codebase for existing patterns or prior art
  7. Check official library documentation (use Context7 MCP)
  8. Search for known issues or discussions
  9. Evaluate at least 2 different approaches

PHASE 3: EVALUATE
  10. Score each option against project constraints:
      - Fits tech stack (Electron, React, TypeScript, Tailwind)
      - Maintenance burden (dependencies, complexity)
      - Performance impact
      - Security implications
      - Learning curve
  11. Identify trade-offs for each option

PHASE 4: SYNTHESIZE
  12. Rank options from best to worst with reasoning
  13. Make a clear recommendation
  14. List remaining unknowns and risks
  15. Estimate implementation effort for recommended option

PHASE 5: REPORT
  16. Write findings in structured format:
      - Question
      - Options evaluated
      - Recommendation with rationale
      - Trade-offs and risks
      - Next steps
```

### Research Task Template (YAML)

```yaml
id: T-XXX
subject: "Research: [the question to answer]"
type: research
priority: must | should
estimate: 20min
timebox: 20min  # Hard stop — report what you have
question: "[Specific, answerable question]"
success_criteria:
  - Question answered with evidence
  - At least 2 options evaluated
  - Recommendation includes trade-offs
  - Unknowns explicitly listed
steps:
  - Define question and success criteria
  - Search docs, codebase, and web
  - Evaluate options against project constraints
  - Synthesize and recommend
  - Document findings
definition_of_done:
  - [ ] Question clearly stated
  - [ ] At least 2 options evaluated
  - [ ] Each option scored against constraints
  - [ ] Clear recommendation with rationale
  - [ ] Trade-offs documented
  - [ ] Unknowns listed
  - [ ] Time box respected
```

### Example Research Task

```yaml
id: T-040
subject: "Research: Best approach for PDF export in Electron"
type: research
priority: must
estimate: 20min
timebox: 20min
question: |
  What is the best way to generate PDF reports from our React
  components in Electron? Need: styled output, tables, charts,
  headers/footers, and page numbers.
options_to_evaluate:
  - Electron's built-in printToPDF
  - Puppeteer/Playwright headless PDF
  - jsPDF + html2canvas
  - React-PDF (@react-pdf/renderer)
constraints:
  - Must work offline (Electron app)
  - Must support our Tailwind styling
  - Must handle tables with 100+ rows
  - Bundle size matters (Electron app already large)
```
