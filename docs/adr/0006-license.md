# ADR-0006: Relicense to GPL-3.0-or-later

**Status:** accepted
**Date:** 2026-11-06

## Context

The repository was GPL-2.0-only. The GPUI crates and `gpui-component` are
Apache-2.0-only. Apache-2.0 is compatible with GPLv3 but not with GPLv2, so
linking them into the application while staying GPL-2.0-only is not a valid
combination. `sherpa-onnx` is already Apache-2.0.

## Decision

Relicense the workspace and the application to `GPL-3.0-or-later`. Update the
root `LICENSE`, workspace Cargo metadata, package metadata, the desktop entry,
AppStream metainfo, README, and CONTEXT.

## Consequences

- The Apache-2.0 dependency graph is compatible.
- Contributors license new work under GPL-3.0-or-later.
- Previously published GPL-2.0 releases remain under their original terms; this
  change applies to the GPUI line from 0.3.0.
