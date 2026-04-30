# fireworks-python -- Modern Packaging

> pyproject.toml, uv, src layout, locking, publishing.
> The current state of Python packaging, minus the historical baggage.

---

## 1. Why Packaging Is Confusing

Python packaging has accreted decades of tools: setup.py, setup.cfg, pip, virtualenv, venv, pipenv, poetry, conda, pdm, hatch, flit, uv. Each solved a real problem; each overlapped with others.

The modern consensus (2024+) converges on:

- **pyproject.toml** for all metadata and tool config.
- **uv** for dependency management and virtual environments (10-100x faster than pip/poetry).
- **hatchling** or **setuptools** as the build backend.
- **src/ layout** for new projects.

If you're starting a new project, go straight here. Don't study the history.

---

## 2. pyproject.toml: The Single Source of Truth

The canonical file structure:

```toml
[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[project]
name = "mypackage"
version = "0.1.0"
description = "What this package does in one line"
readme = "README.md"
requires-python = ">=3.11"
license = "Apache-2.0"
authors = [
    {name = "Your Name", email = "you@example.com"},
]
keywords = ["api", "example"]
classifiers = [
    "Development Status :: 4 - Beta",
    "Programming Language :: Python :: 3.11",
    "Programming Language :: Python :: 3.12",
    "License :: OSI Approved :: Apache Software License",
]
dependencies = [
    "httpx>=0.25",
    "pydantic>=2.0",
    "sqlalchemy[asyncio]>=2.0",
]

[project.optional-dependencies]
dev = [
    "pytest>=8.0",
    "pytest-asyncio>=0.23",
    "pytest-cov>=4.1",
    "mypy>=1.8",
    "ruff>=0.3",
]
docs = [
    "mkdocs>=1.5",
    "mkdocs-material>=9.5",
]

[project.scripts]
mypackage = "mypackage.cli:main"

[project.urls]
Homepage = "https://example.com/mypackage"
Documentation = "https://docs.example.com/mypackage"
Repository = "https://github.com/example/mypackage"

[tool.hatch.build.targets.wheel]
packages = ["src/mypackage"]

[tool.mypy]
strict = true
python_version = "3.11"

[tool.ruff]
line-length = 100
target-version = "py311"

[tool.ruff.lint]
select = ["E", "F", "I", "N", "UP", "B", "SIM", "RUF", "S", "ASYNC"]
ignore = ["E501"]  # line length handled by formatter

[tool.ruff.format]
docstring-code-format = true

[tool.pytest.ini_options]
asyncio_mode = "auto"
testpaths = ["tests"]
addopts = "-v --tb=short"

[tool.coverage.run]
source = ["src/mypackage"]
branch = true

[tool.coverage.report]
fail_under = 80
exclude_lines = [
    "pragma: no cover",
    "raise NotImplementedError",
    "if TYPE_CHECKING:",
]
```

Every tool in the stack reads from this file. No `setup.py`, no `setup.cfg`, no `tox.ini`.

### Choosing a Build Backend

| Backend | Pick When |
|---------|-----------|
| hatchling | New project, want simple, no legacy quirks |
| setuptools | Existing project or need compiled extensions (C/Cython) |
| flit | Pure-Python package, extreme simplicity |
| maturin | Building Rust extensions via pyo3 |
| scikit-build-core | Building CMake-based C++ extensions |

Hatchling is the current default for new projects. It's maintained by the PyPA, supports editable installs, and has less historical baggage than setuptools.

---

## 3. The src/ Layout

```
mypackage/
  pyproject.toml
  src/
    mypackage/
      __init__.py
      core.py
      _internal.py
  tests/
    __init__.py
    test_core.py
  README.md
  LICENSE
```

### Why src/?

When the package is in the root:

```
mypackage/         # repo root
  mypackage/       # package root
    __init__.py
```

You can accidentally `import mypackage` from the repo root without installing. Tests pass locally but fail in CI where the package is installed from a wheel.

With src/:

```
mypackage/
  src/
    mypackage/
      __init__.py
```

`import mypackage` fails unless the package is installed. Your local dev workflow and your CI workflow use the exact same import machinery. No surprises.

### hatch Configuration for src/

```toml
[tool.hatch.build.targets.wheel]
packages = ["src/mypackage"]
```

This tells hatch: "the package to include in the wheel is at src/mypackage, and its import name is mypackage".

---

## 4. uv: The Modern Package Manager

`uv` is written in Rust and is 10-100x faster than pip and poetry. It is becoming the default for new projects.

### Install uv

```bash
# macOS / Linux
curl -LsSf https://astral.sh/uv/install.sh | sh

# Windows (PowerShell)
powershell -ExecutionPolicy ByPass -c "irm https://astral.sh/uv/install.ps1 | iex"

# Cross-platform via pipx
pipx install uv
```

### Core Workflow

```bash
# Create a venv
uv venv
# Activates: source .venv/bin/activate  (or .venv\Scripts\activate on Windows)

# Install the project + dev deps
uv pip install -e ".[dev]"

# Run a command in the venv without activating
uv run pytest
uv run mypy src

# Add a new dependency
uv pip install httpx
# Then manually add "httpx>=0.25" to pyproject.toml -- uv doesn't auto-edit yet

# Lock dependencies
uv pip compile pyproject.toml -o requirements.txt
uv pip compile pyproject.toml --extra dev -o requirements-dev.txt

# Sync with a lockfile
uv pip sync requirements.txt
```

### uv Project Mode (Newer)

```bash
uv init mypackage
cd mypackage
uv add httpx pydantic
uv add --dev pytest mypy
uv run python -m mypackage
uv sync
```

In project mode, uv manages the lockfile automatically and edits pyproject.toml for you. Closer to cargo/npm semantics.

---

## 5. Virtual Environments

### venv (stdlib)

```bash
python -m venv .venv
source .venv/bin/activate        # Unix
.venv\Scripts\activate            # Windows
```

### uv venv (faster)

```bash
uv venv
source .venv/bin/activate
```

### conda (scientific stack)

For NumPy/SciPy/PyTorch stacks where you want pre-built binaries, conda-forge can be simpler. But for pure Python, venv + uv is lighter and faster.

### Don't Install Packages Globally

```bash
# WRONG (usually)
sudo pip install httpx

# RIGHT
python -m venv .venv
source .venv/bin/activate
pip install httpx
```

The global Python site-packages is managed by your OS. Installing into it risks conflicts with system tools. Always use a venv.

### pipx for CLI Tools

Tools like ruff, mypy, black are CLIs. Install them globally via pipx so they're available everywhere without polluting any specific venv:

```bash
pipx install ruff
pipx install mypy
pipx install uv
```

Each pipx-installed tool gets its own isolated venv.

---

## 6. Dependency Locking

A lockfile pins the exact versions (and hashes) of every transitive dependency. Your deployments are reproducible; your CI matches production.

### uv

```bash
# Lock all production dependencies
uv pip compile pyproject.toml -o requirements.txt

# Lock with dev extras
uv pip compile pyproject.toml --extra dev -o requirements-dev.txt

# Install from lockfile
uv pip sync requirements.txt
```

The lockfile includes SHA256 hashes. `uv pip sync` verifies every package before install.

### poetry

```bash
poetry install          # creates poetry.lock
poetry install --sync   # removes packages not in the lock
```

### pip-tools (older, still works)

```bash
pip-compile pyproject.toml
pip-sync requirements.txt
```

### Should You Commit the Lockfile?

- **Applications** (deployed as a service): YES. Commit the lockfile.
- **Libraries** (published to PyPI for others to depend on): NO. Only commit pyproject.toml. Lockfiles pin to one resolution; library users must be free to resolve differently.

---

## 7. Publishing to PyPI

### Build the Distribution

```bash
uv build        # or: python -m build
# Creates dist/mypackage-0.1.0.tar.gz and dist/mypackage-0.1.0-py3-none-any.whl
```

### Upload to TestPyPI First

```bash
uv publish --index-url https://test.pypi.org/simple/ --publish-url https://test.pypi.org/legacy/
# Or: twine upload --repository testpypi dist/*
```

### Upload to Real PyPI

```bash
uv publish
# Or: twine upload dist/*
```

### Use API Tokens

Never upload with your password. Generate a token in the PyPI account settings and put it in `~/.pypirc`:

```ini
[pypi]
username = __token__
password = pypi-AgEIcHlwaS5vcmcCJG...

[testpypi]
username = __token__
password = pypi-AgENdGVzdC5weXBpLm9yZwIj...
```

### Trusted Publishing (Modern)

For GitHub Actions, use "trusted publishing" (OIDC). Configure once in PyPI; no tokens needed in CI:

```yaml
# .github/workflows/publish.yml
jobs:
  publish:
    permissions:
      id-token: write
    steps:
      - uses: actions/checkout@v4
      - uses: astral-sh/setup-uv@v3
      - run: uv build
      - uses: pypa/gh-action-pypi-publish@release/v1
```

More secure than storing tokens. Current best practice.

---

## 8. Versioning

Use [Semantic Versioning](https://semver.org/): `MAJOR.MINOR.PATCH`.

- MAJOR: breaking changes
- MINOR: new features, backwards-compatible
- PATCH: bug fixes, backwards-compatible

Pre-1.0, be liberal about breaking changes. After 1.0, be strict.

### Version Source

Option A: Hardcoded in pyproject.toml:

```toml
[project]
version = "0.1.0"
```

Option B: Dynamic, read from a module:

```toml
[project]
dynamic = ["version"]

[tool.hatch.version]
path = "src/mypackage/__init__.py"
```

```python
# src/mypackage/__init__.py
__version__ = "0.1.0"
```

Option C: Read from git tags (hatch-vcs):

```toml
[build-system]
requires = ["hatchling", "hatch-vcs"]

[project]
dynamic = ["version"]

[tool.hatch.version]
source = "vcs"
```

For applications, option A is simplest. For libraries with frequent releases, option B or C is cleaner.

---

## 9. Environment Variables in Development

Use a `.env` file for local development:

```
# .env (never commit -- add to .gitignore)
DATABASE_URL=postgresql://localhost/mydev
API_KEY=dev-key-xxx
LOG_LEVEL=DEBUG
```

Load with `python-dotenv` or `pydantic-settings`:

```python
from pydantic_settings import BaseSettings, SettingsConfigDict

class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_file=".env")
    database_url: str
    api_key: str
    log_level: str = "INFO"

settings = Settings()
```

Always commit a `.env.example` template with placeholder values so new developers know what's needed.

---

## 10. CI/CD Considerations

### Caching

Cache the virtual environment and uv's download cache:

```yaml
# GitHub Actions example
- uses: actions/cache@v4
  with:
    path: |
      ~/.cache/uv
      .venv
    key: uv-${{ hashFiles('pyproject.toml', 'uv.lock') }}
```

### Matrix Testing

```yaml
strategy:
  matrix:
    python-version: ["3.11", "3.12", "3.13"]
    os: [ubuntu-latest, macos-latest, windows-latest]
```

Test every combination you officially support.

### Pre-commit Hooks

```yaml
# .pre-commit-config.yaml
repos:
  - repo: https://github.com/astral-sh/ruff-pre-commit
    rev: v0.3.0
    hooks:
      - id: ruff
      - id: ruff-format
  - repo: https://github.com/pre-commit/mirrors-mypy
    rev: v1.8.0
    hooks:
      - id: mypy
        additional_dependencies: [pydantic>=2.0]
```

Install once: `pipx install pre-commit && pre-commit install`. Every commit runs lint + type check locally before pushing.

---

## 11. Building Compiled Extensions

### Rust via maturin

```toml
[build-system]
requires = ["maturin>=1.0"]
build-backend = "maturin"

[project]
name = "mypackage"
requires-python = ">=3.11"
```

```toml
# Cargo.toml in the same repo
[lib]
name = "mypackage"
crate-type = ["cdylib"]

[dependencies]
pyo3 = { version = "0.20", features = ["extension-module"] }
```

```bash
maturin develop       # build + install into current venv
maturin build --release
```

Rust extensions are the current best answer for "Python needs to be faster here". 10-100x speedups for CPU-bound code.

### C via setuptools

```toml
[build-system]
requires = ["setuptools>=61", "wheel"]
build-backend = "setuptools.build_meta"
```

```python
# setup.py (still needed for compiled extensions)
from setuptools import setup, Extension

setup(
    ext_modules=[
        Extension(
            "mypackage._fast",
            sources=["src/mypackage/_fast.c"],
        )
    ],
)
```

Setuptools is older but required for complex build customisation.

---

## 12. Common Gotchas

### Import Errors After Install

Symptom: `pip install -e .` succeeds but `import mypackage` fails.

Cause: usually a mismatch between the directory structure and the `packages` config.

Fix: verify `[tool.hatch.build.targets.wheel].packages` matches the actual directory:

```toml
[tool.hatch.build.targets.wheel]
packages = ["src/mypackage"]
```

### Tests Can't Find the Package

Symptom: `from mypackage import X` fails in tests.

Cause: the package isn't installed in the venv, only present on disk.

Fix: `pip install -e ".[dev]"` or `uv pip install -e ".[dev]"`.

### Different Versions in Dev vs Prod

Symptom: "works locally, fails in production".

Cause: lockfile not used, pyproject.toml has loose constraints, different resolutions.

Fix: always install from the lockfile in production. `uv pip sync requirements.txt`.

### Wheel Size Growing

Symptom: published wheel is huge.

Cause: accidentally including tests/docs/build artefacts.

Fix:

```toml
[tool.hatch.build.targets.wheel]
packages = ["src/mypackage"]
exclude = ["tests", "docs", "*.pyc", "__pycache__"]
```

Or use `MANIFEST.in` for more complex exclusions.

---

## 13. Anti-Patterns

| Anti-Pattern | Fix |
|--------------|-----|
| `setup.py` + `setup.cfg` for new project | Use pyproject.toml + hatchling |
| `requirements.txt` as dependency source | Use pyproject.toml; lock to requirements.txt |
| `sudo pip install` | Use a venv |
| Pinning exact versions in pyproject.toml | Use ranges in pyproject, exact in lockfile |
| Committing `.venv/` | `.gitignore` it |
| Not committing lockfile for apps | Commit it for reproducibility |
| Committing lockfile for libraries | Don't; users must resolve themselves |
| Separate poetry/pip/conda workflows | Standardise on one (uv for most cases) |
| Using `pip install` against system Python | pipx for CLIs, venv for projects |

---

## 14. Cross-References

- [full-guide.md](full-guide.md) -- project structure, src/ layout
- [pytest-patterns.md](pytest-patterns.md) -- pytest config in pyproject.toml
- [typing-deep-dive.md](typing-deep-dive.md) -- mypy config
- [gil-and-perf.md](gil-and-perf.md) -- building Rust extensions for performance
