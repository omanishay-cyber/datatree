# Dependency Vulnerability Scanning Reference

Procedures for auditing npm dependencies for security, licensing, and maintenance health.

---

## npm audit

**Basic usage**:
```bash
# Run audit and get human-readable output
npm audit

# Get JSON output for programmatic processing
npm audit --json

# Only show production dependencies
npm audit --omit=dev

# Auto-fix where possible
npm audit fix

# Force-fix (may include breaking major version bumps)
npm audit fix --force
```

**Interpreting results**:
- **Critical**: Remote code execution, credential theft -- fix immediately
- **High**: Significant security impact -- fix within 24 hours
- **Moderate**: Limited impact or harder to exploit -- fix within a week
- **Low**: Minimal impact -- fix in next regular maintenance cycle

**CI Integration**: Add `npm audit --audit-level=high` to CI pipeline. Fail the build if high or critical vulnerabilities are found.

---

## Known CVE Checking

**Sources**:
- **NVD (National Vulnerability Database)**: https://nvd.nist.gov/
- **GitHub Advisory Database**: https://github.com/advisories
- **Snyk Vulnerability Database**: https://security.snyk.io/
- **npm Security Advisories**: Integrated into `npm audit`

**Cross-reference process**:
1. Run `npm audit --json` to get advisory IDs
2. Look up each advisory in GitHub Advisory Database for full details
3. Check if the vulnerable code path is actually used in your project
4. Determine if the vulnerability is exploitable in your context (desktop app vs web server)

**Electron-specific considerations**: Many npm advisories target server-side vulnerabilities (e.g., HTTP header injection). For Electron desktop apps, assess whether the attack vector exists in your context.

---

## Outdated Check

```bash
# List outdated packages
npm outdated

# Output format: Package | Current | Wanted | Latest | Location
```

**Priority for updates**:
1. **Security patches** (patch version with security fix) -- update immediately
2. **Dependencies of dependencies** with vulnerabilities -- update parent package
3. **Major version bumps** -- schedule for next development cycle, test thoroughly
4. **Minor/patch updates** -- batch into regular maintenance updates

**Update strategy**:
```bash
# Update to latest within semver range (safe)
npm update

# Update a specific package to latest
npm install package-name@latest

# Check what would change before updating
npm outdated --long
```

---

## License Audit

```bash
# Install license checker
npx license-checker --summary

# Get detailed per-package info
npx license-checker --json

# Check for problematic licenses
npx license-checker --failOn "GPL-2.0;GPL-3.0;AGPL-3.0"
```

**License categories for proprietary projects**:
- **Safe**: MIT, BSD-2-Clause, BSD-3-Clause, ISC, Apache-2.0, 0BSD, Unlicense
- **Review Required**: MPL-2.0 (file-level copyleft), LGPL (dynamic linking usually OK)
- **Incompatible with Proprietary**: GPL-2.0, GPL-3.0, AGPL-3.0 -- these require you to release your source code
- **Unknown**: Packages without a license field -- investigate manually

**Action items**:
- Flag any GPL/AGPL dependency for removal or replacement
- Document all MPL/LGPL dependencies with usage justification
- Ensure all dependencies have a declared license

---

## Maintenance Check

**Health indicators for a dependency**:
- **Last publish date**: >12 months ago = potential maintenance risk
- **Open issues**: High issue count with no recent responses = abandoned
- **Open PRs**: Unreviewed PRs for months = no active maintainer
- **Bus factor**: Single maintainer = high risk of abandonment
- **Download trends**: Declining downloads = community moving away
- **TypeScript support**: @types package or built-in types = well-maintained
- **Security policy**: SECURITY.md present = responsible disclosure process exists

**Check commands**:
```bash
# View package info including last publish date
npm view <package> time.modified

# View maintainers
npm view <package> maintainers

# View repository
npm view <package> repository.url
```

**Risk threshold**: If a package is unmaintained (>12 months no updates) AND has known vulnerabilities, find a replacement.

---

## Alternative Finding

When a dependency is problematic (vulnerable, unmaintained, or too heavy):

1. **Check npm for alternatives**: Search for packages with similar keywords
2. **Compare options**: Use https://npmtrends.com/ for download comparison
3. **Evaluate candidates** on:
   - Bundle size (check with https://bundlephobia.com/)
   - Maintenance activity
   - TypeScript support
   - License compatibility
   - Security track record
4. **Consider removing the dependency** entirely:
   - Can the functionality be implemented in <50 lines?
   - Does Node.js/Electron now provide this built-in?
   - Is the dependency only used in one place?

**Common replacements**:
- `moment` -> `date-fns` or native `Intl.DateTimeFormat`
- `lodash` -> native JS methods (map, filter, reduce, structuredClone)
- `uuid` -> `crypto.randomUUID()`
- `node-fetch` -> native `fetch()` (available in Node 18+/Electron)
- `rimraf` -> `fs.rm(path, { recursive: true })`

---

## Lock File Hygiene

**Always commit package-lock.json**:
- Ensures reproducible builds across all machines
- Prevents supply-chain attacks from altered dependency resolution
- Enables `npm ci` for clean, deterministic installs

**Review lock file changes in PRs**:
- Unexpected new dependencies may indicate supply chain compromise
- Check for dependency resolution changes (different versions than expected)
- Look for integrity hash changes on existing packages (tampering indicator)

**Best practices**:
```bash
# Use npm ci in CI/CD (clean install from lock file)
npm ci

# Never run npm install in CI -- it can modify the lock file

# After adding a new dependency, review the lock file diff
git diff package-lock.json

# Verify package integrity
npm audit signatures
```

**Lock file red flags**:
- Packages from unexpected registries
- Integrity hash changes without version changes
- New transitive dependencies from unknown publishers
- Dependency resolution pointing to git URLs instead of npm registry
