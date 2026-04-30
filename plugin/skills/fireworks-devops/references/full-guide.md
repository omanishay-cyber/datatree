# fireworks-devops — Full Guide

> This file contains the full reference material previously embedded in SKILL.md.
> Loaded on demand when the skill needs deeper context.

## 3. Electron Packaging Checklist

### electron-builder Configuration

```yaml
# electron-builder.yml
appId: com.yourcompany.appname
productName: "App Name"
copyright: "Copyright (C) 2026 Your Company"

directories:
  output: dist
  buildResources: build

files:
  - "dist/**/*"
  - "node_modules/**/*"
  - "!node_modules/**/test/**"
  - "!node_modules/**/*.map"

asar: true
asarUnpack:
  - "**/*.node"        # Native modules must be outside ASAR
  - "**/sql.js/dist/*" # WASM files must be outside ASAR

extraResources:
  - from: "resources/"
    to: "resources/"
    filter: ["**/*"]
```

### Windows (NSIS)

```yaml
win:
  target:
    - target: nsis
      arch: [x64]
  icon: "build/icon.ico"
  publisherName: "Your Company"

nsis:
  oneClick: false
  perMachine: false
  allowToChangeInstallationDirectory: true
  installerIcon: "build/icon.ico"
  uninstallerIcon: "build/icon.ico"
  installerHeaderIcon: "build/icon.ico"
  createDesktopShortcut: true
  createStartMenuShortcut: true
  shortcutName: "App Name"
```

### macOS (DMG)

```yaml
mac:
  target:
    - target: dmg
      arch: [x64, arm64]
  icon: "build/icon.icns"
  category: "public.app-category.business"
  hardenedRuntime: true
  gatekeeperAssess: false
  entitlements: "build/entitlements.mac.plist"
  entitlementsInherit: "build/entitlements.mac.inherit.plist"

dmg:
  contents:
    - x: 130
      y: 220
    - x: 410
      y: 220
      type: link
      path: /Applications
```

### Auto-Updater

```yaml
publish:
  provider: github
  owner: your-username
  repo: your-repo
  releaseType: release
```

```typescript
// In main process
import { autoUpdater } from 'electron-updater';

autoUpdater.checkForUpdatesAndNotify();
autoUpdater.on('update-available', (info) => { /* notify user */ });
autoUpdater.on('update-downloaded', (info) => { /* prompt restart */ });
```

### Icons Checklist

```
Windows:  icon.ico   (256x256, 128x128, 64x64, 48x48, 32x32, 16x16 — all in one .ico)
macOS:    icon.icns  (1024x1024, 512x512, 256x256, 128x128, 64x64, 32x32, 16x16)
Linux:    icon.png   (512x512 minimum, 1024x1024 preferred)
Tray:     tray.png   (16x16 or 22x22, with @2x variant for Retina)
```

### ASAR Rules

```
INSIDE ASAR (default — everything in `files`):
  - JavaScript/TypeScript compiled output
  - HTML templates
  - CSS/Tailwind output
  - Small static assets (icons, small images)

OUTSIDE ASAR (via asarUnpack or extraResources):
  - Native modules (.node files)
  - WASM files (sql.js, etc.)
  - Large binary assets (videos, large databases)
  - Files that need filesystem path access
  - Config files that users might edit
```

> See `references/electron-packaging.md` for detailed build optimization, code signing, and notarization.

---

## 4. Release Management

### Semantic Versioning

```
MAJOR.MINOR.PATCH

MAJOR: Breaking changes (user must change their workflow)
  - Removed features, changed database schema, incompatible API
  - Example: 1.x.x → 2.0.0

MINOR: New features (backward compatible)
  - New functionality, new UI sections, new export formats
  - Example: 1.2.x → 1.3.0

PATCH: Bug fixes (backward compatible)
  - Fixed calculations, UI glitches, crash fixes
  - Example: 1.2.3 → 1.2.4

Pre-release: alpha/beta/rc
  - 2.0.0-alpha.1, 2.0.0-beta.3, 2.0.0-rc.1
```

### Release Workflow

```bash
# 1. Update version in package.json
npm version minor  # or major, patch

# 2. Generate/update changelog
# (Manual or automated — see references/documentation.md)

# 3. Create git tag
git tag -a v1.3.0 -m "Release v1.3.0: Add barcode scanner support"

# 4. Push with tags
git push origin main --tags

# 5. Create GitHub release
gh release create v1.3.0 \
  --title "v1.3.0 — Barcode Scanner Support" \
  --notes-file CHANGELOG.md \
  --latest

# 6. Upload build artifacts
gh release upload v1.3.0 dist/*.exe dist/*.dmg dist/*.AppImage
```

### Rollback Plan

```
If release has critical bugs:
  1. Assess severity: crash? data loss? cosmetic?
  2. If data loss or crash: immediately release hotfix
     git checkout -b hotfix/v1.3.1 v1.3.0
     # fix the bug
     npm version patch
     git push origin hotfix/v1.3.1 --tags
  3. If cosmetic: note in known issues, fix in next release
  4. Never delete a published release — create a new one
  5. If auto-updater pushed the bad version:
     - Push hotfix ASAP (users will auto-update again)
     - Consider disabling auto-update for the bad version
```

---

## 5. CI/CD Pipeline Design

### GitHub Actions Workflow Structure

```yaml
name: Build and Test

on:
  push:
    branches: [main, 'release/*']
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
        node-version: [20.x]

    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: ${{ matrix.node-version }}
          cache: 'npm'
      - run: npm ci
      - run: npm run lint
      - run: npm run typecheck
      - run: npm test

  build:
    needs: test
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
      - run: npm ci
      - run: npm run build
      - uses: actions/upload-artifact@v4
        with:
          name: build-${{ matrix.os }}
          path: dist/
```

### Release Workflow (Tag-Triggered)

```yaml
name: Release

on:
  push:
    tags: ['v*']

jobs:
  release:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
      - run: npm ci
      - run: npm run build:electron
      - uses: softprops/action-gh-release@v2
        with:
          files: dist/*
          draft: true
```

> See `references/ci-cd.md` for caching strategies, test matrix, and artifact management.

---

## 6. Documentation Sync

### Documentation Checklist (Every Release)

```
[ ] README.md reflects current features and setup instructions
[ ] CHANGELOG.md updated with new entries
[ ] API documentation matches current code (if public API)
[ ] Migration guide written (if breaking changes)
[ ] Screenshots updated (if UI changed significantly)
[ ] Contributing guide up to date (if workflow changed)
```

### Changelog Format (Keep a Changelog)

```markdown
# Changelog

All notable changes to this project will be documented in this file.

## [1.3.0] - 2026-03-25

### Added
- Barcode scanner support with UPC-A, EAN-13, Code-128 detection
- PDF export for invoices with customizable templates

### Changed
- Improved inventory search performance by 3x
- Updated Electron from v39 to v40

### Fixed
- Profit margin calculation now accounts for discounts correctly
- Dark mode text contrast in the reports section

### Removed
- Deprecated legacy CSV import (use XLSX import instead)
```

> See `references/documentation.md` for JSDoc/TSDoc guidelines, migration guide format, and sync automation.

---

## 7. Verification Gate

Before completing ANY DevOps task, verify ALL applicable items:

### Commit Verification

```
[ ] Commit message follows conventional commit format
[ ] Type is correct (feat vs fix vs refactor)
[ ] Subject line < 72 characters, imperative mood
[ ] Body explains WHY, not just WHAT
[ ] Co-Author line present
[ ] No secrets in the diff (API keys, passwords, tokens)
[ ] No large binary files committed (images, databases)
[ ] Specific files staged (not `git add -A`)
```

### PR Verification

```
[ ] Title < 70 characters, conventional commit format
[ ] Body has Summary section with bullet points
[ ] Body has Test Plan with checklist
[ ] All CI checks pass (or explained why failing)
[ ] No merge conflicts with target branch
[ ] Changes reviewed in diff (no accidental inclusions)
[ ] Screenshots included for UI changes
```

### Release Verification

```
[ ] Version bumped correctly (semver rules)
[ ] Changelog updated for this version
[ ] All tests pass on all target platforms
[ ] Build artifacts generated for all platforms
[ ] Auto-updater tested (if applicable)
[ ] Release notes are clear and user-facing
[ ] Rollback plan documented
```

---

## 8. Anti-Premature-Completion

**"I committed the code" is NOT done.**

Done means ALL of the following:
- Commit follows conventional commit format with proper type and scope
- PR has a descriptive title AND body with summary and test plan
- CI passes (or failure is understood and documented)
- No secrets exposed in the diff
- Documentation updated if needed (CHANGELOG, README)
- Rollback plan exists for releases

### Common Premature Completion Traps

- "I pushed the branch" — Did you create the PR with a proper description?
- "CI is green" — Did you check it actually ran the right tests?
- "I tagged the release" — Did you update the changelog? Upload artifacts?
- "The build passed" — Did you test the built artifact on a clean machine?
- "I merged the PR" — Did you delete the source branch? Update docs?

---

## 9. 3-Strike Rule

If 3 CI failures occur on different issues, stop fixing individual symptoms and review the entire pipeline.

### Recovery Process

```
Strike 1: Fix the specific failing step
  - Read the error message carefully
  - Fix the root cause, not the symptom
  - Re-run CI

Strike 2: Check for environmental issues
  - Is the CI environment different from local?
  - Are there caching issues?
  - Are dependencies resolved correctly?
  - Re-run CI

Strike 3: STOP. Full pipeline review.
  - Read the entire workflow file
  - Compare CI environment with local environment
  - Check all environment variables and secrets
  - Review recent changes to the pipeline
  - Consider running CI steps locally with `act`
  - Ask the user if the pipeline was recently changed
```

### Whack-a-Mole Indicators

- Fixing one test breaks another
- Different failures on each CI run (flaky tests)
- Works locally but fails in CI (environment mismatch)
- CI passes but the built artifact doesn't work

When you see these patterns, the problem is systemic, not specific.

---

### PR Intent Declaration
Before committing, declare intent and verify diff matches:
1. State: "This change SHOULD modify [files] to achieve [goal]"
2. State: "These files SHOULD NOT change: [critical files]"
3. Run diff and verify actual changes match declared intent
4. Risk classification: auth/secrets/database/payments = HIGH RISK, always verify
5. Exit codes: 0=clean, 2=high-risk detected, 3=drift from intent

### Breaking Change Detection
Before any commit touching shared interfaces:
- Check IPC channel signatures (added/removed/modified params)
- Check Zustand store shapes (exported interface changes)
- Check exported function signatures in shared modules
- WARN but don't block — developer decides

---

## 10. Reference Links

### Internal References
- `references/git-workflow.md` — Merge vs rebase, conflict resolution, stash, cherry-pick, bisect
- `references/github-api.md` — gh CLI commands, Trees API, releases, PR automation, Actions
- `references/electron-packaging.md` — Build config, code signing, auto-updater, ASAR, optimization
- `references/ci-cd.md` — GitHub Actions templates, test matrix, caching, artifacts, release workflow
- `references/documentation.md` — Changelog format, migration guides, JSDoc/TSDoc, sync checklist

### External Tools
- **gh CLI** — GitHub's official CLI for PR, issue, release, and API operations
- **electron-builder** — Build and package Electron apps for all platforms
- **electron-updater** — Auto-update framework for Electron apps
- **semantic-release** — Automated version management and changelog generation
- **commitlint** — Lint commit messages against conventional commit format
- **husky** — Git hooks made easy (pre-commit, commit-msg, etc.)
- **lint-staged** — Run linters on staged files only
- **act** — Run GitHub Actions locally for testing

### Standards and Specifications
- [Conventional Commits](https://www.conventionalcommits.org/) — Commit message specification
- [Semantic Versioning](https://semver.org/) — Version numbering standard
- [Keep a Changelog](https://keepachangelog.com/) — Changelog format standard
- [GitHub Flow](https://docs.github.com/en/get-started/quickstart/github-flow) — Branching model

---

## 11. Scope Boundaries

- **MINIMUM**: Every commit must follow conventional commit format. No exceptions, no shortcuts.
- **MAXIMUM**: Do not set up CI/CD for projects without tests. A pipeline that runs zero tests provides false confidence and wastes resources. Add tests first, then automate.

---

## 12. Git 2025 Features

### Reftables (Git 2.48+)

New reference storage format replacing loose ref files and packed-refs. 50-80% faster ref operations, atomic updates, better scalability for repos with many refs.

```bash
# Check current ref storage format
git config core.refStorage

# Migrate to reftables
git refs migrate --ref-storage=reftables

# Verify migration
git fsck --full
git log --oneline -5

# Roll back if needed
git refs migrate --ref-storage=files
```

**When to use:** Repositories with 10,000+ refs, high-frequency branch operations, CI/CD systems, monorepos.

### Sparse-Checkout (Enhanced in 2.48)

Check out only a subset of files. Essential for monorepos.

```bash
# Clone with sparse-checkout
git clone --filter=blob:none --sparse <repo-url>
cd <repo>

# Initialize and set directories (cone mode — recommended)
git sparse-checkout init --cone
git sparse-checkout set src/api src/shared docs
git sparse-checkout add tests/integration

# View current patterns
git sparse-checkout list

# Reapply after merge/rebase materializes unwanted files
git sparse-checkout reapply

# Disable
git sparse-checkout disable
```

### Partial Clone

Clone without downloading all objects initially.

```bash
# Blobless clone (fastest, smallest)
git clone --filter=blob:none <repo-url>

# Skip large files only
git clone --filter=blob:limit=10m <repo-url>

# Ultimate efficiency: partial clone + sparse-checkout
git clone --filter=blob:none --sparse <repo-url>
cd <repo>
git sparse-checkout set --cone src/api
git checkout main

# Convert existing repo to partial clone
git config extensions.partialClone origin
git config remote.origin.promisor true
git fetch --filter=blob:none

# Prefetch all missing objects
git fetch --unshallow
```

### Git Backfill (Experimental, 2.49+)

Background process to fetch missing objects in partial clones.

```bash
git backfill                        # Fetch missing blobs
git backfill --min-batch-size=1000  # Configure batch size
git backfill --sparse               # Respect sparse-checkout patterns
```

### Git Worktrees

Multiple working directories from one repository — shared `.git`, one fetch updates all.

```bash
# List worktrees
git worktree list

# Create worktree for existing branch
git worktree add ../project-feature feature-branch

# Create worktree with new branch
git worktree add -b new-feature ../project-new-feature

# Worktree for PR review while coding
git worktree add ../myproject-pr-123 origin/pull/123/head

# Remove and clean up
git worktree remove ../project-feature
git worktree prune
```

### Scalar (Large Repository Tool, Git 2.47+)

```bash
# Clone with all optimizations (sparse-checkout, partial clone, commit-graph, maintenance)
scalar clone --branch main <repo-url>

# Register existing repo for optimizations
scalar register <path>
```

### Git 2.49 Performance

- zlib-ng integration: 20-30% faster compression
- Path-walk API: 50-70% better delta compression
- Memory leak free status (achieved in 2.48)

### Performance Comparison

| Strategy | Size | Time |
|----------|------|------|
| Traditional clone | 5GB | 10 min |
| Sparse-checkout | 500MB | 3 min |
| Partial clone | 100MB | 1 min |
| Partial + sparse | 50MB | 30 sec |

### gh CLI Quick-Reference

```bash
# Pull Requests
gh pr checks 55 --repo owner/repo            # Check CI status
gh pr create --title "feat: ..." --body "..." # Create PR

# Workflow Runs
gh run list --repo owner/repo --limit 10      # List recent runs
gh run view <run-id> --repo owner/repo        # View run details
gh run view <run-id> --log-failed             # View failed step logs

# Issues
gh issue list --repo owner/repo --json number,title --jq '.[] | "\(.number): \(.title)"'

# API (advanced queries)
gh api repos/owner/repo/pulls/55 --jq '.title, .state, .user.login'
```

Most `gh` commands support `--json` for structured output and `--jq` for filtering. Always specify `--repo owner/repo` when not in a git directory.

---

## 13. Related Skills

- **fireworks-workflow** — Project lifecycle management (planning, task tracking, milestone delivery)
- **fireworks-security** — Security hardening in CI/CD pipelines (secrets management, supply chain, code signing)
- **fireworks-test** — CI test integration (test runners, coverage reporting, flaky test detection)
