# Submitting Anish.Mneme to the winget public catalog

These manifests live at `winget/Anish/Mneme/0.3.2/`. To make `winget install Anish.Mneme`
work for everyone on Windows, you submit them as a PR to the official catalog:
[microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs).

This file is a one-page checklist for that submission.

## What gets submitted

Three files, validated with `winget validate`:

- `Anish.Mneme.yaml` — version manifest (schema 1.6.0)
- `Anish.Mneme.locale.en-US.yaml` — default locale (publisher, description, tags, license)
- `Anish.Mneme.installer.yaml` — installer (x64 zip, portable nested binary, SHA256)

The arm64 row is intentionally omitted because `mneme-v0.3.2-windows-arm64.zip` is
not yet on the GitHub release. Once CI publishes it, open a follow-up PR adding the
second `Architecture: arm64` block under `Installers:` with its own SHA256.

## Step 1 — Fork microsoft/winget-pkgs

```powershell
gh repo fork microsoft/winget-pkgs --clone --remote
cd winget-pkgs
git checkout -b add-anish-mneme-0.3.2
```

## Step 2 — Drop the manifests in the right path

The catalog uses `manifests/<first-letter-lower>/<Publisher>/<PackageName>/<Version>/`.
For `Anish.Mneme` 0.3.2 that path is `manifests/a/Anish/Mneme/0.3.2/`.

```powershell
$src = "D:\Mneme Dome\Mneme-Home-Handoff-2026-04-30-2027\source\winget\Anish\Mneme\0.3.2"
$dst = "manifests\a\Anish\Mneme\0.3.2"
New-Item -ItemType Directory -Path $dst -Force | Out-Null
Copy-Item "$src\*.yaml" $dst
```

## Step 3 — Validate locally (optional but recommended)

```powershell
winget validate --manifest manifests\a\Anish\Mneme\0.3.2
```

You should see `Manifest validation succeeded.` with no warnings. (We already
cleared the only warning — `Scope` is not allowed for portable nested installers,
so it's omitted.)

## Step 4 — Commit and push

```powershell
git add manifests/a/Anish/Mneme/0.3.2
git commit -m "New version: Anish.Mneme version 0.3.2"
git push -u origin add-anish-mneme-0.3.2
```

## Step 5 — Open the PR

```powershell
gh pr create `
  --repo microsoft/winget-pkgs `
  --base master `
  --head omanishay-cyber:add-anish-mneme-0.3.2 `
  --title "New version: Anish.Mneme version 0.3.2" `
  --body-file PR_BODY.md
```

### PR body template

The winget-pkgs PR template asks you to tick a few boxes. Paste this into `PR_BODY.md`
before running `gh pr create`:

```markdown
## New version of Anish.Mneme

- PackageIdentifier: `Anish.Mneme`
- Version: `0.3.2`
- Architecture: `x64` (arm64 follow-up PR planned once CI publishes the build)

### Manifest checklist

- [x] Have you signed the [Contributor License Agreement](https://cla.opensource.microsoft.com/microsoft/winget-pkgs)?
- [x] Have you checked that there aren't other open [pull requests](https://github.com/microsoft/winget-pkgs/pulls) for the same manifest update/change?
- [x] This PR only modifies one (1) manifest
- [x] Have you [validated](https://docs.microsoft.com/windows/package-manager/package/manifest#validation) your manifest locally with `winget validate --manifest <path>`?
- [x] Have you tested your manifest locally with `winget install --manifest <path>`?
- [x] Does your manifest conform to the [1.6 schema](https://github.com/microsoft/winget-cli/blob/master/doc/ManifestSpecv1.6.md)?

### Installer notes

- InstallerType: `zip`
- NestedInstallerType: `portable`
- Inner binary: `bin\mneme.exe`
- Command alias: `mneme`
- Source: `https://github.com/omanishay-cyber/mneme/releases/download/v0.3.2/mneme-v0.3.2-windows-x64.zip`
```

## Step 6 — What happens after you open the PR

1. **Automated CI** (`AzurePipelines` + `winget-validation`) runs `winget validate` and
   pulls the installer URL to verify the SHA256 matches. Failures show up in the
   "Checks" tab — if any fail, push a fix to the same branch and they re-run.
2. **Label workflow** auto-tags with `New-Version`, `Version-Update`, etc.
3. **Moderator review** by the Microsoft `wingetbot` team. Typical turnaround is
   1–3 business days. They may ask for tweaks (uncommon for a clean validate).
4. **Merge** — the manifest lands in `master` and the catalog index regenerates
   within ~30 minutes. After that, anyone on Windows can run:

   ```powershell
   winget install Anish.Mneme
   ```

   and later:

   ```powershell
   winget upgrade Anish.Mneme
   ```

## Local install before the PR (optional smoke test)

You can install straight from the local manifests without going through the catalog:

```powershell
winget install --manifest "D:\Mneme Dome\Mneme-Home-Handoff-2026-04-30-2027\source\winget\Anish\Mneme\0.3.2"
mneme doctor
```

If that works, the catalog PR will too.

## Updating to a future version

Each new release ships under its own version folder, e.g. `manifests/a/Anish/Mneme/0.3.3/`.
Bump `PackageVersion` in all three files and recompute `InstallerSha256` from the new
release zip:

```powershell
gh release download v0.3.3 --pattern 'mneme-v0.3.3-windows-x64.zip' `
  --output C:\Users\Anish\Desktop\temp\winget-x64.zip --repo omanishay-cyber/mneme --clobber
Get-FileHash C:\Users\Anish\Desktop\temp\winget-x64.zip -Algorithm SHA256
```
