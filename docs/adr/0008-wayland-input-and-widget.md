# ADR-0008: Wayland via evdev and a layer-shell status widget

**Status:** accepted
**Date:** 2026-11-15

## Context

Wayland gives clients no global key grab. Floure shipped `floure --toggle` plus
the tray, which works but asks every user to edit a compositor config. The
widget half of the redesign was never built: the new spec called a layer-shell
surface "optional, unavailable on GNOME/KDE", and `gpui-pre` 0.3.11's
`WindowKind::LayerShell` was unproven for this app.

## Decision

- **Global shortcut:** read `/dev/input/event*` with `evdev` (Linux only,
  opt-in). It is not a grab — keystrokes still reach the focused app — so it
  needs the `input` group and reports `NeedsInputGroup` with the exact
  `sudo usermod -aG input $USER` command instead of failing quietly. evdev wins
  whenever it is usable; X11 keeps `global-hotkey`; the loopback command stays
  the always-available fallback.
- **Widget:** a second GPUI window, `WindowKind::LayerShell` on the overlay
  layer, anchored bottom, `KeyboardInteractivity::None` and `focus: false`, so
  it can never take keyboard focus. It is mapped only while an utterance is in
  flight. Compositors without `zwlr_layer_shell_v1` fail at `open_window`; the
  reason is recorded and surfaced in Diagnostics, with the in-app status line
  as the indicator.
- **Typing on Wayland:** try `wtype`, then `dotool`, then `ydotool`. Clipboard
  behaviour is unchanged and is never forced over typing.

## Consequences

- A group membership is a truthful trigger without a compositor edit, and the
  cost (read access to every input device) is explicit and reversible.
- The widget's click region is the full 240×56 surface: `gpui-pre` exposes no
  input-region API, so clicks there do not reach the window behind it while the
  widget is visible.
- No GTK dependency; the app stays a single binary.
