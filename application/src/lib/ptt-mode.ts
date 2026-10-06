// ── PTT activation mode + audio-feedback prefs (localStorage-backed) ──

export type PttMode = "hold" | "toggle";

const PTT_MODE_KEY = "stt-ptt-mode";
const SOUND_ENABLED_KEY = "stt-sound-enabled";
const SOUND_VOLUME_KEY = "stt-sound-volume";

export function getPttMode(): PttMode {
  if (typeof window === "undefined") return "hold";
  return localStorage.getItem(PTT_MODE_KEY) === "toggle" ? "toggle" : "hold";
}

export function setPttMode(mode: PttMode) {
  if (typeof window === "undefined") return;
  localStorage.setItem(PTT_MODE_KEY, mode);
}

export function isSoundEnabled(): boolean {
  if (typeof window === "undefined") return true;
  const v = localStorage.getItem(SOUND_ENABLED_KEY);
  return v === null ? true : v === "1";
}

export function setSoundEnabled(on: boolean) {
  if (typeof window === "undefined") return;
  localStorage.setItem(SOUND_ENABLED_KEY, on ? "1" : "0");
}

/// 0..1 gain multiplier for the PTT blips. Defaults to 1 (full volume).
export function getSoundVolume(): number {
  if (typeof window === "undefined") return 1;
  const raw = Number(localStorage.getItem(SOUND_VOLUME_KEY));
  if (!Number.isFinite(raw)) return 1;
  return Math.min(1, Math.max(0, raw));
}

export function setSoundVolume(v: number) {
  if (typeof window === "undefined") return;
  localStorage.setItem(SOUND_VOLUME_KEY, String(Math.min(1, Math.max(0, v))));
}
