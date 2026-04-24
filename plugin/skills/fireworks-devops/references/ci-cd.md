# CI/CD — Deep Reference

> GitHub Actions workflow templates for Electron apps. Test matrix, caching, artifact management, and release automation.

---

## Full CI Workflow for Electron App

```yaml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true

jobs:
  lint-and-typecheck:
    name: Lint & Typecheck
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: 'npm'

      - run: npm ci

      - name: TypeScript check
        run: npx tsc --noEmit

      - name: Lint
        run: npm run lint

  test:
    name: Test (${{ matrix.os }})
    needs: lint-and-typecheck
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: 'npm'

      - run: npm ci

      - name: Run tests
        run: npm test
        env:
          CI: true

      - name: Upload test results
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: test-results-${{ matrix.os }}
          path: test-results/
          retention-days: 7

  build:
    name: Build (${{ matrix.os }})
    needs: test
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: 'npm'

      # Cache Electron binaries
      - uses: actions/cache@v4
        with:
          path: |
            ~/.cache/electron
            ~/AppData/Local/electron/Cache
            ~/Library/Caches/electron
          key: electron-${{ matrix.os }}-${{ hashFiles('package-lock.json') }}
          restore-keys: electron-${{ matrix.os }}-

      - run: npm ci

      - name: Build Electron app
        run: npm run build:electron
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}

      - name: Upload build artifacts
        uses: actions/upload-artifact@v4
        with:
          name: build-${{ matrix.os }}
          path: |
            dist/*.exe
            dist/*.dmg
            dist/*.AppImage
            dist/*.deb
            dist/*.snap
          retention-days: 30
```

---

## Test Matrix Strategies

### Standard Matrix

```yaml
strategy:
  fail-fast: false
  matrix:
    os: [ubuntu-latest, windows-latest, macos-latest]
    node: [18, 20, 22]
```

### Matrix with Exclusions

```yaml
strategy:
  matrix:
    os: [ubuntu-latest, windows-latest, macos-latest]
    node: [18, 20]
    exclude:
      # Skip Node 18 on Windows (known compatibility issue)
      - os: windows-latest
        node: 18
```

### Matrix with Includes (Extra Combinations)

```yaml
strategy:
  matrix:
    os: [ubuntu-latest, windows-latest]
    node: [20]
    include:
      # Also test on macOS with ARM
      - os: macos-14          # ARM runner
        node: 20
        arch: arm64
```

---

## Caching Strategies

### npm Cache

```yaml
- uses: actions/cache@v4
  with:
    path: ~/.npm
    key: npm-${{ runner.os }}-${{ hashFiles('**/package-lock.json') }}
    restore-keys: |
      npm-${{ runner.os }}-
```

### Electron Binary Cache

```yaml
- uses: actions/cache@v4
  with:
    path: |
      ~/.cache/electron
      ~/AppData/Local/electron/Cache
      ~/Library/Caches/electron
    key: electron-${{ runner.os }}-${{ hashFiles('package-lock.json') }}
```

### Vite / Build Cache

```yaml
- uses: actions/cache@v4
  with:
    path: node_modules/.vite
    key: vite-${{ runner.os }}-${{ hashFiles('vite.config.*') }}-${{ hashFiles('src/**') }}
    restore-keys: |
      vite-${{ runner.os }}-${{ hashFiles('vite.config.*') }}-
      vite-${{ runner.os }}-
```

### Cache Tips

```
- Cache key MUST include something that changes when deps change (lockfile hash)
- Use restore-keys for partial cache matches (faster than no cache)
- Monitor cache hit rate in Actions logs
- Cache size limit: 10 GB per repo (oldest caches evicted first)
- Caches from base branches are available to PRs
```

---

## Release Workflow (Tag-Triggered)

```yaml
name: Release

on:
  push:
    tags: ['v*']

permissions:
  contents: write    # Needed to create releases

jobs:
  build-and-release:
    name: Build (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: 'npm'

      - run: npm ci

      - name: Build and package
        run: npm run build:electron
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          # Code signing (Windows)
          CSC_LINK: ${{ secrets.WIN_CSC_LINK }}
          CSC_KEY_PASSWORD: ${{ secrets.WIN_CSC_KEY_PASSWORD }}
          # Code signing (macOS)
          APPLE_ID: ${{ secrets.APPLE_ID }}
          APPLE_APP_SPECIFIC_PASSWORD: ${{ secrets.APPLE_APP_SPECIFIC_PASSWORD }}
          APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}

      - name: Upload to GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          draft: true
          files: |
            dist/*.exe
            dist/*.dmg
            dist/*.zip
            dist/*.AppImage
            dist/*.deb
            dist/*.snap
            dist/latest*.yml
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

  publish-release:
    name: Publish Release
    needs: build-and-release
    runs-on: ubuntu-latest
    steps:
      - name: Publish draft release
        run: |
          gh release edit "${{ github.ref_name }}" \
            --repo "${{ github.repository }}" \
            --draft=false
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

---

## Artifact Management

### Upload Patterns

```yaml
# Upload specific files
- uses: actions/upload-artifact@v4
  with:
    name: installer-windows
    path: dist/App-Setup-*.exe
    retention-days: 30
    if-no-files-found: error    # Fail if no files match

# Upload directory
- uses: actions/upload-artifact@v4
  with:
    name: full-build
    path: dist/
    retention-days: 7

# Upload with compression
- uses: actions/upload-artifact@v4
  with:
    name: build-output
    path: dist/
    compression-level: 9        # Maximum compression
```

### Download Patterns

```yaml
# Download specific artifact
- uses: actions/download-artifact@v4
  with:
    name: installer-windows
    path: ./downloaded/

# Download all artifacts
- uses: actions/download-artifact@v4
  with:
    path: ./all-artifacts/
```

### Artifact Best Practices

```
- Name artifacts descriptively: include OS, arch, purpose
- Set retention-days to avoid accumulating old artifacts
- Use if-no-files-found: error to catch build failures early
- Compress large artifacts before uploading
- Download only what you need in downstream jobs
- Clean up artifacts after release is published
```
