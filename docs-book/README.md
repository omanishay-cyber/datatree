# docs-book/

Mneme's documentation site, built with [mdBook](https://rust-lang.github.io/mdBook/) and deployed to https://omanishay-cyber.github.io/mneme/ via `.github/workflows/pages.yml`.

## Build locally

```bash
# One-time: install mdBook
cargo install mdbook --version 0.4.40

# Build
cd docs-book
mdbook build              # output → docs-book/book/

# Serve with live reload at http://localhost:3000
mdbook serve --open
```

## Structure

```text
docs-book/
├── book.toml          # mdBook config: title, search, theme
├── src/
│   ├── SUMMARY.md    # Chapter outline (single source of truth for nav)
│   ├── intro.md      # Hero / landing page
│   ├── install/      # Per-OS install pages
│   ├── getting-started/
│   ├── concepts/     # Resolver, embeddings, self-ping, architecture, vision
│   ├── cli/          # CLI reference
│   ├── mcp/          # MCP tool catalogue
│   ├── hooks/        # 3-layer self-ping enforcement
│   ├── releases/     # v0.4.0 page + full changelog
│   ├── troubleshooting.md
│   └── contributing.md
├── theme/
│   └── custom.css    # Brand colors + premium typography overrides
└── README.md
```

## Deployment

GitHub Pages source must be set to **"GitHub Actions"** (Repo Settings → Pages → Source). Once enabled, every push to `main` that touches `docs-book/`, `README.md`, or `CHANGELOG.md` triggers `pages.yml`, which:

1. Installs mdBook 0.4.40 on the runner
2. `mdbook build` from `docs-book/`
3. Uploads the `book/` artifact
4. Deploys to GitHub Pages

The legacy `docs/index.html` remains in the repo as the v0.3.x snapshot but stops being served once the workflow takes over.

## Theme

`theme/custom.css` extends mdBook's built-in `ayu` theme with:

- Brand-gradient titles (`#4191E1 → #22D3EE → #41E1B5`)
- Glassmorphism on the sidebar + nav (`backdrop-filter: blur(...)`)
- Inter for body text + JetBrains Mono for code
- Premium table styles + code-block accents
- Subtle entry animation on H1 (respects `prefers-reduced-motion`)
- Print-friendly fallback

## Editing

Markdown only. mdBook's link preprocessor resolves `{{#include path}}` directives if you want to pull in CHANGELOG / README snippets to keep the docs in lockstep.

The "Edit this page" button on every doc points at the correct `docs-book/src/` path on GitHub.

## License

Same as the parent repo — Apache-2.0.
