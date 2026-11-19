# ADR-0007: Package the GPUI binary without Tauri

**Status:** accepted
**Date:** 2026-11-06

## Context

Tauri supplied bundling, AppImage, and the release upload action. Removing it
leaves a single binary that needs its desktop assets, runtime libraries, and a
release workflow. `sherpa-onnx` was linked with the `shared` feature, which
produced `.so` files that no package installed; that bug would have shipped in
every format.

## Decision

- Link `sherpa-onnx` with its default `static` feature.
- Build the binary once with `cargo build --release -p floure-app`.
- Produce `.deb` with `cargo-deb`, `.rpm` with `cargo-generate-rpm`, Arch with
  `packaging/arch/PKGBUILD`, and a portable `.tar.gz` with
  `packaging/make-portable.sh`.
- Drop AppImage for the first GPUI release. The tarball covers distro-agnostic
  users without FUSE.
- Ship the desktop entry, AppStream metainfo, 32–512 px icons plus a scalable
  SVG, and the licence in every package.
- Derive runtime dependencies from the actual binary (`$auto` / `auto-req`),
  not a hand-maintained list.

## Consequences

- Packages are self-contained and do not depend on the source tree.
- Windows/macOS packaging is deferred; the static-link change is Linux-first.
- The release workflow uploads four artifacts to
  `IntegerAlex/floure-core` and checks that the tag, Cargo, deb, rpm, and
  PKGBUILD versions agree.
