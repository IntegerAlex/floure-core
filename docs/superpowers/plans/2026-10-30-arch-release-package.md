# Arch Release Package Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a native Arch `.pkg.tar.zst` to tagged/manual GitHub releases beside the existing `.deb` and `.rpm` assets.

**Architecture:** Keep Tauri's Ubuntu release job for `.deb`, `.rpm`, and AppImage. Add a small `makepkg` recipe that builds Floure from the release tag in an Arch Linux container, then upload its package to the already-created GitHub release. Do not publish to AUR or add full package builds to pull-request CI.

**Tech Stack:** GitHub Actions, Arch `makepkg`/PKGBUILD, Cargo, pnpm, Tauri 2, GitHub CLI.

**Spec:** User request and approval in the conversation (2026-10-30): native Arch package; attach `.pkg.tar.zst` to releases with existing `.deb`/`.rpm`; release-only packaging; no AUR publishing.

## Global Constraints

- Build the Arch package for `x86_64` from the release tag. Map a semver prerelease hyphen to `_` in Arch `pkgver`, while keeping the original version for the GitHub source tag and app metadata.
- Preserve the existing Ubuntu 22.04 `.deb`, `.rpm`, and AppImage release flow.
- Build Arch packages only for releases, not every PR.
- Do not publish or configure credentials for AUR.
- Keep user-facing package output as a native package with declared runtime dependencies and desktop integration.

## Review Focus

- Release tag differs from committed app version: ensure PKGBUILD stamps the original release version before compilation.
- Prerelease release version contains `-`: normalize only Arch `pkgver`, while retaining the original tag and app version.
- `makepkg` is run as root in CI: create/use an unprivileged builder and verify the package file is produced.
- Arch's default debug split creates a second package: disable it or ensure CI handles it explicitly; CI currently expects one package.
- Missing runtime libraries or desktop assets: declare required Arch dependencies and verify binary, desktop entry, and icon are in the package archive.
- Arch job races release creation/upload: order it after the existing release job and upload with the resolved tag.
- CI accidentally runs native packaging for each PR: keep the new build job release-triggered only.

---

### Task 1: Add the native Arch package recipe

**Files:**
- Create: `packaging/arch/PKGBUILD`
- Create: `packaging/arch/floure.desktop`

**Interfaces:**
- Produces: `floure-${pkgver}-${pkgrel}-x86_64.pkg.tar.zst` via `makepkg`.
- Consumes: release source archive `https://github.com/IntegerAlex/floure-core/archive/refs/tags/v${_source_version}.tar.gz`, extracted under `floure-core-${_source_version}`.

- [x] Add a PKGBUILD for `floure`, `x86_64`, GPL-2.0-only, with Arch runtime dependencies for WebKitGTK, GTK3, Ayatana appindicator, librsvg, OpenSSL, and ALSA; declare optional typing/clipboard helpers and disable the unwanted debug split package.
- [x] In `prepare()`, reverse the Arch prerelease normalization and stamp `tauri.conf.json` and `Cargo.toml` from the original release version.
- [x] In `build()`, install the frozen pnpm lockfile, build the frontend, then run `cargo build --release --locked` for `application/src-tauri`.
- [x] In `package()`, install the executable to `/usr/bin/floure`, desktop entry and Tauri icons to standard locations, and the GPL license under `/usr/share/licenses/floure`.
- [x] Verify PKGBUILD metadata with `makepkg --printsrcinfo` and ensure the desktop entry names `floure` as its executable and icon.

### Task 2: Build and attach the Arch package on releases

**Files:**
- Modify: `.github/workflows/release.yml`

**Interfaces:**
- Consumes: `resolve-release` outputs `tag` and `version`, plus the completed Ubuntu `release` job.
- Produces: a `.pkg.tar.zst` uploaded to the matching GitHub release and retained as a workflow artifact.

- [x] Add a release-only Arch build job that depends on `resolve-release` and the current `release` job.
- [x] In an Arch Linux build environment, install build dependencies, check out the resolved tag, map release `-` to `_` for PKGBUILD `pkgver`, create an unprivileged builder, and run `makepkg --syncdeps --noconfirm`.
- [x] Fail if no `.pkg.tar.zst` is produced; upload the package as a workflow artifact and attach it to the resolved GitHub release using `GITHUB_TOKEN` and `gh release upload --clobber`.
- [x] Verify the workflow still triggers only on tags, manual releases, and the existing release-keyword path; PR CI remains unchanged.
- [x] Validate workflow syntax and review job dependency/permissions for release upload.

### Task 3: Document Linux package installation

**Files:**
- Modify: `README.md`

- [x] Add a concise Linux downloads note: `.deb`/`.rpm`/Arch `.pkg.tar.zst` are attached to GitHub releases; Arch users install with `sudo pacman -U ./floure-*.pkg.tar.zst`.
- [x] Verify the documented filenames and command match the PKGBUILD output.

### Final verification

- [x] Run `makepkg --printsrcinfo` in an Arch environment.
- [x] Run `actionlint .github/workflows/release.yml` if available; otherwise parse/inspect the YAML and confirm expression syntax.
- [x] Confirm the release workflow retains the existing `.deb`, `.rpm`, and AppImage assets and the Arch asset is uploaded only after package build succeeds.
