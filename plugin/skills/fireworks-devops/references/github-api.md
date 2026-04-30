# GitHub API & gh CLI — Deep Reference

> gh CLI commands, Trees API for batch operations, releases, PR automation, and GitHub Actions patterns.

---

## gh CLI Essential Commands

### Pull Requests

```bash
# Create a PR
gh pr create --title "feat: add feature" --body "Description here"

# Create PR with template
gh pr create --title "feat: add feature" --body "$(cat <<'EOF'
## Summary
- Added X feature

## Test plan
- [ ] Test case 1
- [ ] Test case 2
EOF
)"

# List open PRs
gh pr list

# View a specific PR
gh pr view 123

# View PR diff
gh pr diff 123

# Check out a PR locally
gh pr checkout 123

# Merge a PR
gh pr merge 123 --merge           # merge commit
gh pr merge 123 --squash          # squash and merge
gh pr merge 123 --rebase          # rebase and merge

# Review a PR
gh pr review 123 --approve
gh pr review 123 --request-changes --body "Please fix X"
gh pr review 123 --comment --body "Looks good but consider X"

# Close a PR without merging
gh pr close 123

# List PR checks
gh pr checks 123

# View PR comments
gh api repos/{owner}/{repo}/pulls/123/comments
```

### Issues

```bash
# Create an issue
gh issue create --title "Bug: X is broken" --body "Steps to reproduce..."

# List issues
gh issue list
gh issue list --label "bug"
gh issue list --assignee "@me"

# View an issue
gh issue view 456

# Close an issue
gh issue close 456 --reason completed

# Add labels
gh issue edit 456 --add-label "priority:high,bug"

# Assign
gh issue edit 456 --add-assignee "@me"
```

### Releases

```bash
# Create a release
gh release create v1.2.0 \
  --title "v1.2.0 — Feature Name" \
  --notes "Release notes here"

# Create with auto-generated notes
gh release create v1.2.0 --generate-notes

# Create a draft release
gh release create v1.2.0 --draft --title "v1.2.0"

# Create a pre-release
gh release create v1.2.0-beta.1 --prerelease

# Upload assets to a release
gh release upload v1.2.0 ./dist/app-setup.exe ./dist/app.dmg

# List releases
gh release list

# Download release assets
gh release download v1.2.0

# Delete a release (CAUTION)
gh release delete v1.2.0
```

### Repository

```bash
# Clone a repo
gh repo clone owner/repo

# Create a repo
gh repo create my-app --private --source=. --push

# View repo info
gh repo view

# Fork a repo
gh repo fork owner/repo
```

---

## Trees API (Batch Commits)

For creating commits that change many files at once without checking them out locally.

### When to Use

- Automated code generation that produces many files
- Batch updates (version bumps across many files)
- CI/CD workflows that need to commit generated files
- When you want to create a commit without a working directory

### How It Works

```bash
# Step 1: Create blobs for each file
BLOB_SHA=$(gh api repos/{owner}/{repo}/git/blobs \
  -f content="file content here" \
  -f encoding="utf-8" \
  --jq '.sha')

# Step 2: Create a tree with all the blobs
TREE_SHA=$(gh api repos/{owner}/{repo}/git/trees \
  -f "base_tree={base_tree_sha}" \
  -f "tree[][path]=path/to/file.ts" \
  -f "tree[][mode]=100644" \
  -f "tree[][type]=blob" \
  -f "tree[][sha]=${BLOB_SHA}" \
  --jq '.sha')

# Step 3: Create a commit pointing to the tree
COMMIT_SHA=$(gh api repos/{owner}/{repo}/git/commits \
  -f "message=chore: batch update files" \
  -f "tree=${TREE_SHA}" \
  -f "parents[]={parent_commit_sha}" \
  --jq '.sha')

# Step 4: Update the branch reference
gh api repos/{owner}/{repo}/git/refs/heads/main \
  -X PATCH \
  -f "sha=${COMMIT_SHA}"
```

---

## PR Automation

### Auto-Assign Reviewers

```yaml
# .github/CODEOWNERS
# Automatically request review from these users/teams
*.ts       @team/frontend
*.css      @team/design
/src/main/ @team/backend
```

### Label Automation

```yaml
# .github/labeler.yml (with actions/labeler)
frontend:
  - src/renderer/**
  - src/components/**

backend:
  - src/main/**
  - src/handlers/**

documentation:
  - docs/**
  - '**/*.md'

dependencies:
  - package.json
  - package-lock.json
```

### PR Template

```markdown
<!-- .github/pull_request_template.md -->
## Summary
<!-- What does this PR do? Why? -->

## Changes
<!-- List of changes -->

## Test plan
<!-- How to verify this works -->
- [ ] Tested in development
- [ ] Tested in production build
- [ ] Tested both light and dark themes

## Screenshots
<!-- If UI changes, add before/after -->

## Checklist
- [ ] Code follows project conventions
- [ ] TypeScript compiles without errors
- [ ] Tests pass
- [ ] Documentation updated (if needed)
```

---

## GitHub Actions

### Workflow Triggers

```yaml
on:
  push:
    branches: [main, 'release/*']        # Push to specific branches
    tags: ['v*']                          # Push tags matching pattern
    paths: ['src/**', 'package.json']     # Only when these paths change
    paths-ignore: ['docs/**', '*.md']     # Ignore these paths

  pull_request:
    branches: [main]                      # PRs targeting main
    types: [opened, synchronize, reopened]

  schedule:
    - cron: '0 0 * * 1'                  # Weekly on Monday at midnight

  workflow_dispatch:                       # Manual trigger
    inputs:
      environment:
        description: 'Deploy environment'
        required: true
        default: 'staging'
        type: choice
        options: [staging, production]
```

### Job Matrix

```yaml
jobs:
  test:
    strategy:
      fail-fast: false                    # Don't cancel other jobs if one fails
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
        node: [18, 20, 22]
        exclude:                          # Skip specific combinations
          - os: windows-latest
            node: 18
    runs-on: ${{ matrix.os }}
```

### Artifacts

```yaml
# Upload build artifacts
- uses: actions/upload-artifact@v4
  with:
    name: build-${{ matrix.os }}
    path: dist/
    retention-days: 30

# Download artifacts in a later job
- uses: actions/download-artifact@v4
  with:
    name: build-ubuntu-latest
    path: dist/
```

### Secrets

```yaml
# Access secrets in workflow
env:
  GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}   # Auto-provided
  NPM_TOKEN: ${{ secrets.NPM_TOKEN }}     # Custom secret

# Never echo secrets
- run: echo "Token is ${{ secrets.MY_SECRET }}"  # BAD — shows in logs
- run: some-command --token "$MY_TOKEN"           # GOOD — use env var
  env:
    MY_TOKEN: ${{ secrets.MY_SECRET }}
```

### Caching

```yaml
# Cache npm dependencies
- uses: actions/cache@v4
  with:
    path: ~/.npm
    key: ${{ runner.os }}-npm-${{ hashFiles('**/package-lock.json') }}
    restore-keys: |
      ${{ runner.os }}-npm-

# Cache Electron binaries
- uses: actions/cache@v4
  with:
    path: ~/.cache/electron
    key: ${{ runner.os }}-electron-${{ hashFiles('**/package-lock.json') }}
```
