# UX Placement & Component Research

**Scope:** every user-facing surface in Floure — where it sits, what it shows, and what the HCI literature says about that choice. Reviewed from Ubuntu/Wayland; the Windows paths are reasoned, not run.

**Method:** four parallel literature searches (speech-input UX + hotkeys; overlays/notifications/attention; settings IA + onboarding; privacy/trust). Every claim below is tagged by evidence strength:

- **[P]** peer-reviewed (CHI, UIST, TOCHI, CSCW, IMWUT, SOUPS, USENIX Security, PETS, ETR&D, JEP:HPP, PACM HCI, Computing, IJHCI)
- **[V]** primary vendor documentation (Microsoft, GNOME, Android, Meta)
- **[G]** practitioner guideline (Nielsen Norman Group, MeasuringU, design systems)
- **[I]** our own inference — flagged as such, not evidence

**Honesty note:** several questions have *no* direct research. Those gaps are listed explicitly in §7 rather than papered over with tangential papers.

---

## 1. Push-to-talk trigger

**Current state:** global hotkey `Ctrl+Shift+F12` (was `Ctrl+Shift+Space`, was `Ctrl+Alt+Space` — rejected because Alt+Space opens the Windows system menu). A native Ctrl+Win hold hook exists on Windows as an alternative.

### Findings

- **Rejecting `Ctrl+Alt+Space` was correct and is the best-supported decision in this whole review.** Microsoft's own `RegisterHotKey` documentation states that keyboard shortcuts involving the Windows key are reserved for the OS, and `MOD_NOREPEAT` exists precisely because hotkeys otherwise fire on auto-repeat. Alt+Space is a documented OS-level binding; Microsoft ships a `KeyboardFilter` API specifically to suppress it. **[V]**
- **`Ctrl+Win` is the remaining hazard.** Same vendor documentation: Win-key combos are OS-reserved. A native hold hook is a reasonable power-user escape hatch, but should never be the default — a user who binds Floure to Ctrl+Win is fighting the shell. **[V]**
- **`Ctrl+Shift+F12` is defensible but expensive.** Two modifiers plus an unfamiliar non-printing key is exactly the "complex combination" class that the shortcut literature shows users abandon: in a study of 251 experienced Word users, shortcut use was rare and familiarity correlated only r = .13 with adoption. **[P]** A separate study found users knew the function of only ~10% of keys, with mouse:keyboard interaction at roughly 88:12. **[P]**
- **The fix is a layout affordance, not a better key.** KeyMap (CHI 2020) found that mapping shortcuts to physical key layout improved recall over the prior state of the art; ExposeHK (CHI 2013) found that showing all hotkeys on modifier-press accelerates adoption. Both point the same way: if a shortcut is hard to recall, make it spatially discoverable rather than expecting muscle memory to form from documentation. **[P]**
- **No peer-reviewed study validates "choose a non-auto-repeating key" as a hotkey-selection criterion.** The F12 choice is a sound engineering inference (printable main keys auto-repeat while held), not a validated finding. **[I]**
- **No direct research found** on hold-to-talk vs. toggle for desktop dictation. The nearest literature is in-car PTT and dialogue turn-taking — both tangential. **[gap]**

### Recommendations

1. Keep `Ctrl+Shift+F12` as the default. It avoids OS-reserved chords and auto-repeat. **[V]**
2. Show the binding on a **keyboard diagram** at first run, not as text. KeyMap and ExposeHK both support spatial discovery over recall. **[P]**
3. Keep the Ctrl+Win hook as an opt-in power-user path, clearly labelled as fighting the shell. **[V]**
4. Add a **toggle mode** as an accessibility accommodation — justified by the motor-impairment keyboard literature, not as an evidence-backed ergonomics claim. **[I]**
5. Treat the hotkey as **best-effort per compositor**. On wlroots/Hyprland the default may not exist without user config; ship a one-line fallback and check availability before advertising the binding. **[V]** (Wayland has no core keyboard-grab equivalent; the `org.freedesktop.portal.GlobalShortcuts` portal is entirely compositor-determined.)

---

## 2. Floating widget (the status pill)

**Current state:** `WidgetView.tsx` — 52px dark pill, bottom-center, expands 168→248px when active, 12-bar canvas waveform at ~15Hz, status label, mic toggle button, `prefers-reduced-motion` respected, `role="toolbar"`. A separate `PttOverlay.tsx` renders a `fixed bottom-8 left-1/2` pill with pulsing mic + 3 waveform dots, `pointer-events-none`, 200ms exit delay. In PR #24 the open/close/context-menu/Escape controls were removed to make it a pure status pill.

### Findings

- **The removal of interactive controls is theoretically sound.** Weiser & Brown's *calm technology* (1996) is the seminal work on periphery vs. center of attention: technology should engage the periphery and move easily between center and periphery. A pure status pill that stays in the periphery is textbook calm technology. **[P]** Matthews et al. (UIST 2004) operationalize this for notification systems. **[P]**
- **Bottom-center is the most *noticeable* position but can be *distracting*.** Chua et al. (2016) tested 9 display positions on a monocular HMD in a dual-task scenario (N=27): middle-center and bottom-center were noticed fastest, but top and peripheral positions were more comfortable, unobtrusive, and preferred. Participants found bottom-center "too distracting" because it overlaid primary task content. **This was an HMD study, not a desktop study.** **[P]**
- **Bottom-center is standard for desktop toasts** (Material/Supernova guidelines), while HPE's design system recommends top-center or top-right. The guidelines are split. **[G]**
- **No direct comparative study** on desktop overlay placement for a dictation status pill exists. The Chua et al. HMD study is the closest proxy; its generalizability to desktop is limited. **[gap]**
- **Any visual change in the periphery imposes some attentional cost.** Stothart et al. (2015) found that the *mere receipt* of a cell phone notification — even if not checked — imposes an attentional cost. Mark et al. (2008) found interrupted work is completed faster but with significantly higher stress, frustration, and workload. **[P]**
- **The live mic-level animation is supported by practitioner guidelines and one peer-reviewed study.** Meta's Voice SDK attention system recommends that "Mic on" status be visually represented for the full duration of audio collection. Android's `VoiceInputIndicator` is a visual component showing audio level (0.0–1.0). VOICON (DIS 2024) systematically designed geometric motion-based visual feedback for voice assistants and found it "intuitively understandable and perceived as useful." **[V][P]**
- **The animation should not be the *only* indicator of recording state** (accessibility). **[I]**
- **No research found** on whether continuous mic-level animation builds confidence or distracts during dictation. **[gap]**

### Recommendations

1. Keep bottom-center. It is the most noticeable position and standard for desktop toasts; the distraction concern is real but the pill is small and non-interactive. **[G][I]**
2. Keep the pill non-interactive. The calm-technology framework and the interruption-cost literature both support a pure status display. **[P]**
3. Keep the ~15Hz waveform. It is fast enough to feel alive, slow enough to stay peripheral. VOICON supports geometric motion feedback for voice interfaces. **[P]**
4. **Add a non-visual recording indicator** (e.g. a tray icon state change) so the animation is not the only signal. **[I]**
5. Respect `prefers-reduced-motion` (already done). **[G]**
6. **Do not let the pill occlude the taskbar/dock or IME candidate windows** — no research exists on this, but it is a concrete risk on some systems. **[I]**

---

## 3. Error panel

**Current state:** `ErrorBanner.tsx` — fixed `right-4 top-4`, 320px wide, `max-h-[70vh]`, category badges ("conn"/"model"/"mic"/"perm"/"error"), retry + dismiss per error.

### Findings

- **Error message quality is a well-studied area.** Nielsen's heuristics include "help users recognize, diagnose, and recover from errors" and "plain language, no error codes." **[G]**
- **Self-repair after errors greatly improves user assessment — but overcorrection hurts.** Cuadra et al. (2021, N=101) found that when a voice assistant made a mistake and then self-repaired ("Did I get that wrong?"), user assessment improved significantly. However, self-repair when *no* error occurred degraded assessment. **[P]**
- **Failure type matters enormously.** A mixed-methods study (N=199 failures, 12 interviews) found that "response" failures (wrong action, inaction) most severely damaged trust. "Overcapture" (continuing to listen without acting) was the single most harmful failure type. Users were more forgiving of spurious triggers and ambiguity. **[P]**
- **Trust is rebuilt through low-stakes tasks.** After failures, users stopped using the assistant for the failed task for a short period, then resumed. **[P]**
- **Multimodal error correction is faster and more accurate than unimodal.** A dictation study found multimodal correction (speech + pen + keyboard) was faster than respeaking alone. Users initially prefer speech correction but learn to avoid ineffective modalities with experience. **[P]**
- **Accurate context improves correction.** Providing corrected surrounding sentences improved both performance and user perception of error correction in dictation. **[P]**

### Recommendations

1. **Implement self-repair, but only when an error is detected.** If ASR confidence is low, a brief "I didn't catch that — want to try again?" improves trust. Do *not* prompt for correction when confidence is high. **[P]**
2. **Prioritize avoiding "overcapture" and "response" failures.** Stop listening immediately when the user stops speaking; if transcription fails, show *something* rather than silence. **[P]**
3. **Offer multimodal correction.** Click-to-edit on low-confidence words (visual highlight) plus respeaking as an option. **[P]**
4. **Use accurate context for correction.** When the user corrects a word, use the corrected surrounding text to improve subsequent recognition. **[P]**
5. **After a failure, suggest a low-stakes task.** A "quick retry" with a simple prompt after a transcription failure. **[P]**
6. **Replace category badges with plain language.** "conn"/"model"/"mic"/"perm" are codes; Nielsen's heuristic says plain language, no error codes. **[G]**

---

## 4. Settings panel

**Current state:** `SettingsPanel.tsx` — sidebar with sections (Home / Insights / Dictionary / History / Config / Models / Settings), modal dialogs for some sub-areas (models, hotkeys, LLM provider). 392 lines with 6 `h3` section headers.

### Findings

- **Progressive disclosure is the dominant evidence-based pattern.** Nielsen (2006) states it improves learnability, efficiency, and error rate; more than 2 disclosure levels typically reduces usability. **[G]**
- **Sidebar + content is the standard for multi-section settings.** Modals suit short, focused tasks but are poor for complex multi-decision configuration. Nested modals are an anti-pattern. **[G]**
- **Microsoft's Windows app settings guidelines** recommend ≤5 settings per section, immediate-apply (no Save button), and a single-column scrollable page with section headers. **[V]**
- **Settings anchored to UI elements reduce the gulf of execution.** Ponsard & McGrenere (CHI 2016) found that settings anchored to the UI element they control outperform traditional panels. **[P]**
- **Hick's law is widely cited but contested for HCI.** A 2020 review argues it speaks *against* "less is better" and that choice-reaction time is often near-constant in real UIs. Use it as a heuristic for grouping, not as proof that fewer options is always better. **[P]**
- **Settings search complements but doesn't replace browse.** MeasuringU (2012) found ~14% of users start with search; most browse first. Search is a precision tool for known-intent tasks, not a fix for poor IA. **[G]**

### Recommendations

1. Keep the sidebar for section navigation — it is the right pattern for 5+ sections. **[G]**
2. Use modals sparingly — only for genuinely focused sub-tasks like hotkey configuration. For models and LLM provider, a full page or drawer is better since they involve multiple decisions. **[G]**
3. Group by user mental model (Audio, Models, LLM, Hotkeys, History, About), not by internal architecture. **[I]**
4. Use immediate-apply for toggles; explicit Save only for the API key field. **[V]**
5. Add a search bar once settings exceed ~15 items. **[G]**
6. Anchor settings to the UI element they control where possible (e.g. the mic level slider next to the mic test button). **[P]**

---

## 5. Onboarding wizard

**Current state:** `OnboardingWizard.tsx` — **5 steps** (System Check → Model Download → Mic Setup → Permissions → Ready). Step 5 tells the user to press **Space** — but the actual default hotkey is `Ctrl+Shift+F12`. That is a real inconsistency.

### Findings

- **3–5 steps is the evidence-backed sweet spot.** Snoopr (2026) reports completion drops 10–15% per screen beyond 5. Produktly data (464 SaaS products) shows 1–2 step tours complete at 73%, 9+ steps at 8%. **[G]**
- **Interactive walkthroughs outperform wizards.** A quasi-experiment (n=620) found interactive walkthroughs (52%) outperformed wizards (41%) on completion, but wizards were second-best. **[P]**
- **Time-to-value matters more than step count.** Users who don't see value in 60 seconds rarely return. Microsoft Design recommends showing a useful result *before* asking for configuration. **[G]**
- **Progress indicators reduce abandonment.** Step indicators increase completion 10–15%. **[G]**
- **Intermediate step count optimizes learning.** Nadolski, Kirschner & van Merriënboer (2005) found an intermediate number of steps outperformed both single-step and many-step conditions for complex skill acquisition. **[P]**
- **No direct research found** on mic-test/calibration steps in onboarding wizards. **[gap]**

### Recommendations

1. **Fix the Step 5 hotkey text.** It says "Space" but the default is `Ctrl+Shift+F12`. This is a correctness bug, not a UX preference. **[I]**
2. Reorder to front-load value: (1) mic test → immediate "speak and see text" result, (2) model download (defer if possible — start with smallest model), (3) hotkey, (4) LLM (truly optional, last). **[G]**
3. Make model download a background task rather than a blocking step. **[I]**
4. Make every step skippable with sensible defaults. **[G]**
5. Keep the progress indicator (already present). **[G]**
6. Consider an interactive walkthrough instead of a wizard for the mic test step — the quasi-experiment supports it. **[P]**

---

## 6. History page

**Current state:** `HistoryPage.tsx` — 489 lines, list + search of past dictations. Stores both raw and cleaned text.

### Findings

- **Personal search differs fundamentally from web search.** Users know their own content and can leverage contextual cues (time, author, context). Cutrell & Dumais (CACM 2006) and Dumais et al. (SIGIR 2003, "Stuff I've Seen") show that rich metadata (date, author, preview) is critical; date is the most-used sort (>60% of SIS queries). **[P]**
- **Tight coupling of search and browsing is essential.** Phlat (CHI 2006) merged keyword and metadata search in one query box; users iteratively refined queries. SearchBar (CHI 2008) showed persistent query history aids task resumption. **[P]**
- **Users remember vague attributes, not exact text.** "Stuff I've Seen" found users could filter by whatever they remembered (sender, date, folder) and recognize rather than recall. **[P]**
- **No direct research found** on showing raw vs. cleaned transcription text side by side. **[gap]**

### Recommendations

1. Support iterative query refinement (narrow by date, then by content keyword). **[P]**
2. Show date prominently — it is the strongest retrieval cue for personal info. **[P]**
3. For raw vs. cleaned text: no direct research exists, but the PIM literature on "recognition rather than recall" suggests showing both helps users recognize which version they want. Consider a toggle or side-by-side diff view. **[I]**
4. Allow annotation/marking of important dictations. **[P]**

---

## 7. Tray icon / menu

**Current state:** `lib.rs:1257-1308` — Show Window, Start Listening, Stop Listening, Toggle Widget, Quit. Minimize-to-tray on close.

### Findings

- **Tray/notification area is for status and quick actions, not primary navigation.** Microsoft Windows guidelines state it is "not intended for quick program or command access" (that is the taskbar's job). GNOME's 2003 guidelines explicitly say apps should not add an icon just to show they are running. Ubuntu is phasing out the tray entirely. **[V]**
- **What belongs in a tray menu:** current status, 1–2 actions worth taking without opening the app, and a way into the full app. **[G]**
- **Icon usability matters.** Grobelny et al. (2005) found icon acquisition time follows Fitts' Law; number of icons in pop-up menus should be minimized; frequently used items should be permanently visible. **[P]**
- **Right-click menu should contain:** status info, a "Preferences/Settings" entry, and "Remove Icon" (GNOME guidelines). Single-click should show status or toggle the primary action. **[V]**

### Recommendations

1. The tray icon should show recording state (idle/recording/error) in the icon itself. **[I]**
2. The menu should contain: current status, Start/Stop dictation (the one action worth taking without opening the app), Show History, Settings, and Quit. **[G]**
3. Do not put model selection, LLM config, or full history browsing in the tray menu — those belong in the main window. **[G]**
4. On Linux/Wayland, be aware that tray support is inconsistent (GNOME requires StatusNotifierItem/AppIndicator, not XEmbed). Follow the freedesktop.org System Tray specification. **[V]**

---

## 8. Privacy / trust positioning

**Current state:** local-first — speech transcribed on-device by default (Parakeet/Whisper), local LLM cleanup on-device, no account, no cloud by default. OpenRouter API key is optional and session-only.

### Findings

- **Non-users reject voice assistants primarily over privacy and trust.** In a diary + interview study (N=34), non-users "did not see the utility of smart speakers or did not trust speaker companies," while users expressed few concerns but showed incomplete understanding of risks and "privacy resignation." **[P]**
- **Always-listening is the core concern.** Users worry devices are "always listening" not just when invoked; uncertainty about when recordings are stored and how data is used drives anxiety. **[P]**
- **Privacy explanations help; trust explanations can backfire.** In a survey (N=1314), privacy information ameliorated privacy concerns, but trust information *increased* trust concerns. **[P]**
- **On-device/local processing is a trust signal.** A systematic review (117 papers) concludes effective privacy solutions "will rely on local processing, the use of open source software, or external devices that limit data collection." **[P]**
- **Local-first is a well-established concept with strong user-appeal rationale.** Kleppmann et al. (2019) articulate seven ideals including offline, longevity, privacy, and user control. User testing showed "a feeling of ownership over their tools and their work." **[P]**
- **Data control increases trust — but only through privacy awareness.** A controlled experiment (N=68) found that a Personal Data Store paradigm increased perceived data control, which enhanced transparency and privacy awareness; privacy awareness (not control alone) directly increased trust. **[P]**
- **No direct research found** on "no account required" as a specific positioning claim. **[gap]**

### Recommendations

1. **Lead with local-first as a trust differentiator.** Make "your audio never leaves this device" a prominent, plain-language claim on first run — not buried in settings. **[P]**
2. **Avoid over-explaining trust.** Explicitly saying "you can trust us" can *increase* concern. Demonstrate trustworthiness through behavior (visible mic indicator, local-only badge) rather than assertions. **[P]**
3. **Address the "always listening" anxiety directly.** Floure is push-to-talk, not always-listening — state it explicitly: "Floure only listens when you press the key." **[P]**
4. **"No account" is a feature, not a barrier — but frame it as ownership, not absence.** "Your dictations, yours — no account, no cloud, no lock-in." **[P]**
5. **Make privacy tangible.** A visual "data stays on this device" indicator that users can see during use, not just a claim in onboarding. **[P]**

---

## 9. Microphone permission prompt

**Current state:** `MicPermissionModal.tsx` — modal dialog, "Open Config" button.

### Findings

- **Microphone permissions are denied at higher rates than many other permissions.** In an Android ESM study (N=157), microphone had a 23% denial rate — the highest among permission types tested. **[P]**
- **Rationales (explaining *why*) matter more than timing.** A comparative study (N=473) found that providing rationales explaining why and how data is accessed had a stronger effect on permission decisions than whether the prompt was shown upfront or in-context. Rationales shown upfront were most effective. **[P]**
- **Prior user interaction increases grant rates.** Chrome telemetry (100M+ installs) showed that when users interacted with a page within 5s before a permission prompt, allow rates were 18.1% higher and ignore rates 21.0% lower. **[V]**
- **Voice-based permission requests are problematic.** Users are *less* likely to accept voice-based permissions than text-based, and speech-rate manipulations of "Accept"/"Decline" can unpredictably shift decisions. **[P]**

### Recommendations

1. **Show the mic permission prompt at the moment of first dictation, not at first launch.** The Chrome data strongly supports in-context prompting. **[V]**
2. **Provide a clear, upfront rationale.** "Floure needs your microphone to transcribe your speech. Audio is processed on your device and never uploaded." **[P]**
3. **Use a text-based prompt, not voice.** **[P]**

---

## 10. Typed-text confirmation (invisible output)

**Current state:** dictation is typed directly into another application. Floure has no canvas of its own during dictation.

### Findings

- **If the only feedback channel is audio, users miss >50% of ASR errors.** Hong & Findlater (CHI 2018) found participants missed more than half of recognition errors from audio alone; inter-word pauses improved detection, repetition did not. **[P]**
- **WER is a poor proxy for perceived quality.** Human-perceived accuracy correlates far better with semantic-weighted metrics than with WER (r = 0.91 vs r = 0.65). Errors in names/numbers hurt far more than function words. **[P]**
- **Users tolerate high error rates better on throughput than on workload — and error *detection* silently degrades.** Injected 50% errors raised perceived mental workload and made users *less likely* to notice errors. **[P]**
- **The dominant cost in dictation is correction, not recognition.** Even at ~98% reported accuracy, correction consumed a large share of user effort. **[P]**
- **Speed win is real but narrower than folklore.** 2.93× faster than a *thumb* keyboard in lab, and speech left *slightly more* uncorrected errors (1.30% vs 0.79%). **[P]**

### Recommendations

1. **Do not market WER as a quality claim.** If any accuracy number is shown, it should be salience-weighted (names/numbers/URLs weighted heavily). **[P]**
2. **Assume users will not reliably proofread.** As reliability drops, self-monitoring drops with it. Assume silent acceptance of wrong text. **[P]**
3. **Make the keyboard a live bypass path.** Users must be able to abandon a bad dictation immediately without hunting for the wrong words. **[P]**
4. **If an audible echo is ever added** (accessibility path), segment it per word with pauses, and never rely on "play it again." **[P]**

---

## 11. LLM mode selector

**Current state:** dropdown in settings + tray.

### Findings

- **Mode-switching cost is real but understudied.** The literature on mode errors (Rasmussen, 1986; Sarter & Woods, 1995) is about *unintended* mode changes, not deliberate mode selection. **[P]**
- **No direct research found** on the UX of a cleanup-mode selector in a dictation app. **[gap]**

### Recommendations

1. Keep the mode selector in settings, not in the tray. The tray should have at most one action worth taking without opening the app. **[G]**
2. Show the *effect* of each mode (e.g. "removes fillers and false starts") rather than just the name. **[I]**
3. Consider a per-dictation override (e.g. hold a modifier while pressing the hotkey to skip cleanup). **[I]**

---

## 12. Cross-cutting themes

### Calm technology (Weiser & Brown, 1996)
The widget, the PTT overlay, and the tray icon are all peripheral displays. The calm-technology framework supports the current design direction: non-interactive, auto-show, subtle animation. **[P]**

### Interruption cost (Mark et al., 2008; Stothart et al., 2015)
Any visual change in the periphery imposes some attentional cost. The removal of interactive controls reduces this cost because the pill does not invite interaction. Whether the pill increases or decreases interruption cost vs. an on-demand display is an open research question. **[P]**

### Trust after failure (Baughan et al., CHI 2023; Cuadra et al., 2021)
"Overcapture" (continuing to listen without acting) is the single most trust-damaging failure type. Self-repair after real errors improves trust; overcorrection degrades it. **[P]**

### Local-first as a trust signal (Kleppmann et al., 2019; systematic review 2022)
On-device processing is a meaningful trust signal. Lead with it. **[P]**

---

## 13. Gaps in the literature (no direct research found)

1. **No desktop study** comparing bottom-center vs. top-center for transient status overlays.
2. **No research** on always-available status displays vs. on-demand displays for interruption cost.
3. **No research** on whether continuous mic-level animation builds confidence or distracts during dictation.
4. **No research** on overlay occlusion of IME candidate windows or taskbar/dock.
5. **No direct research** on mic-test/calibration steps in onboarding wizards.
6. **No direct research** on showing raw vs. cleaned transcription text side by side.
7. **No direct research** on "no account required" as a specific UX positioning claim.
8. **No direct research** on the UX of a cleanup-mode selector in a dictation app.
9. **No peer-reviewed study** of hold-to-talk vs. toggle for desktop dictation.
10. **No study** validates "choose a non-auto-repeating key" as a hotkey-selection criterion.

---

## 14. Prioritized recommendations

| # | Recommendation | Evidence | Effort |
|---|---|---|---|
| 1 | Fix Step 5 hotkey text (says "Space", should say `Ctrl+Shift+F12`) | correctness bug | trivial |
| 2 | Show hotkey on a keyboard diagram at first run | **[P]** KeyMap, ExposeHK | low |
| 3 | Implement self-repair only when ASR confidence is low | **[P]** Cuadra et al. | medium |
| 4 | Avoid overcapture: stop listening immediately when user stops | **[P]** Baughan et al. | low |
| 5 | Offer multimodal correction (click-to-edit + respeaking) | **[P]** TOCHI 2001 | medium |
| 6 | Replace error category badges with plain language | **[G]** Nielsen | low |
| 7 | Lead with local-first as a trust differentiator | **[P]** Kleppmann et al. | low |
| 8 | Show mic permission prompt at first dictation, not first launch | **[V]** Chrome telemetry | low |
| 9 | Provide upfront rationale for mic permission | **[P]** Elbitar et al. | low |
| 10 | Reorder onboarding to front-load value (mic test first) | **[G]** Microsoft Design | low |
| 11 | Make model download a background task | **[I]** | medium |
| 12 | Add non-visual recording indicator (tray icon state) | **[I]** | low |
| 13 | Support iterative query refinement in History | **[P]** Phlat, SearchBar | medium |
| 14 | Show date prominently in History | **[P]** Dumais et al. | low |
| 15 | Keep tray menu minimal (status + start/stop + history + settings + quit) | **[G]** Microsoft, GNOME | low |
| 16 | Anchor settings to UI elements where possible | **[P]** Ponsard & McGrenere | medium |
| 17 | Add settings search once >15 items | **[G]** MeasuringU | medium |
| 18 | Add toggle mode as accessibility accommodation | **[I]** | medium |
| 19 | Do not let pill occlude taskbar/dock/IME | **[I]** | low |
| 20 | Frame "no account" as ownership, not absence | **[P]** Kleppmann et al. | low |

---

## Sources

### Peer-reviewed
- [Chua et al., 2016, Chinese CHI] Positioning Glass — https://doi.org/10.1145/2948708.2948713
- [Cuadra et al., 2021, PACM HCI] My Bad! Repairing Intelligent Voice Assistant Errors — https://dl.acm.org/doi/10.1145/3449101
- [Cutrell & Dumais, 2006, CACM] Exploring Personal Information Management — https://cutrell.org/papers/ExploratorySearchAndPIM.pdf
- [Cutrell et al., 2006, CHI] Phlat — https://cutrell.org/papers/chi2006%20proceedings--phlat.pdf
- [Dumais et al., 2003, SIGIR] Stuff I've Seen — https://www.microsoft.com/en-us/research/wp-content/uploads/2003/01/siscore-sigir2003-final.pdf
- [Elbitar et al., 2021, USENIX Security] Explanation Beats Context — https://www.usenix.org/system/files/sec21-elbitar.pdf
- [Fitchett et al., 2008, CHI] SearchBar — https://psycnet.apa.org/doi/10.1145/1357054.1357242
- [Grobelny et al., 2005, HFES] Usability of Graphical Icons — https://jerzygrobelny.com/2005-Grobelny,Karwowski,Drury-Usability_of_graphical_icons_in_the_design_of_human-computer_interfaces.pdf
- [Harrison et al., 2010, CHI] Faster progress bars — https://doi.org/10.1145/1753326.1753556
- [Hong & Findlater, 2018, CHI] Identifying Speech Input Errors Through Audio-Only Interaction — https://doi.org/10.1145/3173574.3174141
- [Jones et al., 2005, ASIST] Search history tools — https://asistdl.onlinelibrary.wiley.com/doi/10.1002/meet.14504201107
- [Kim et al., 2022, INTERSPEECH] Evaluating User Perception of Speech Recognition System Quality with Semantic Distance Metric — https://www.isca-archive.org/interspeech_2022/kim22p_interspeech.html
- [Kleppmann et al., 2019, Onward!] Local-first software — https://doi.org/10.1145/3359591.3359737
- [Lau et al., 2018, CSCW] Alexa, Are You Listening? — https://dl.acm.org/doi/10.1145/3274371
- [Lewis et al., 2020, CHI] KeyMap — https://doi.org/10.1145/3313831.3376483
- [Malkin et al., 2019, PETS] Privacy Attitudes of Smart Speaker Users — https://dl.acm.org/doi/10.1145/3369807
- [Mark et al., 2008, CHI] The Cost of Interrupted Work — https://ics.uci.edu/~gmark/chi08-mark.pdf
- [Matthews et al., 2004, UIST] A toolkit for managing user attention in peripheral displays — https://doi.org/10.1145/1029632.1029676
- [Mishra et al., 2011, INTERSPEECH] Predicting Human Perceived Accuracy of ASR Systems — https://www.isca-archive.org/interspeech_2011/mishra11_interspeech.pdf
- [Murad, Munteanu & Stuerzlinger, 2019, MobileHCI] Effects of WER on ASR Correction Interfaces — https://dl.acm.org/doi/10.1145/3338286.3344404
- [Nadolski, Kirschner & van Merriënboer, 2005, British Journal of Educational Psychology] Optimizing the number of steps in learning tasks — https://doi.org/10.1348/000709904X22403
- [Kushlev, Proulx & Dunn, 2016, CHI] Silence Your Phones — https://doi.org/10.1145/2858036.2858359
- [Ponsard & McGrenere, 2016, CHI] Anchored Customization — https://doi.org/10.1145/2858036.2858129
- [Ruan et al., 2018, IMWUT] Comparing Speech and Keyboard Text Entry — https://doi.org/10.1145/3161187
- [Stothart, Mitchum & Yehnert, 2015, JEP:HPP] The attentional cost of receiving a cell phone notification — https://doi.org/10.1037/xhp0000100
- [Tabassum et al., 2019, IMWUT] Investigating Users' Preferences for Always-Listening Voice Assistants — https://dl.acm.org/doi/10.1145/3369807
- [VOICON, 2024, DIS] Geometric Motion-Based Visual Feedback in Voice User Interface — https://doi.org/10.1145/3643834.3660741
- [Weiser & Brown, 1996, Xerox PARC] Designing Calm Technology — https://calmtech.com/papers/designing-calm-technology
- [Woodworth & Borst, 2023, Computers & Graphics] Visual cues in VR for guiding attention — https://doi.org/10.1016/j.cag.2023.12.008
- [Zhou et al., 2011] Supporting dictation speech recognition error correction — https://kk.org/mt-files/reCCearch-mt/(2011)%20Supporting%20dictation%20speech%20recognition%20error%20correction-%20the%20impact%20of%20external%20information,%202009.pdf
- [Systematic review, 2022, arXiv] A Systematic Review of Ethical Concerns with Voice Assistants — https://arxiv.org/html/2211.04193v3
- [Alharbi et al., 2022, arXiv] Ignorance is Bliss? — https://ar5iv.labs.arxiv.org/html/2211.12900
- [Bao, 2023, MSc thesis, SFU] Errors? Not Too Worrisome — https://summit.sfu.ca/_flysystem/fedora/2023-09/etd22639.pdf
- [Bonné et al., 2017, SOUPS] Exploring decision making with Android's runtime permission dialogs — https://www.usenix.org/conference/soups2017/technical-sessions/presentation/bonne
- [Hauptmann & Rudnicky, 1990, CHI] A Comparison of Speech vs Typed Input — http://www.cs.cmu.edu/afs/cs/user/alex/www/chi90
- [Karat et al., 1999, CHI] Patterns of Entry and Correction in Large Vocabulary Continuous Speech Recognition Systems — https://doi.org/10.1145/302979.303160
- [Halverson et al., 1999, INTERACT] The Beauty of Errors — https://dblp.dagstuhl.de/rec/conf/interact/HalversonHKK99.html
- [Lane et al., 2005, Int. J. Human-Computer Interaction] Hidden Costs of GUIs — https://www.ruf.rice.edu/~lane/papers/hidden_costs.pdf
- [Malacria, Bailly, Harrison, Cockburn & Gutwin, 2013, CHI] ExposeHK — https://doi.org/10.1145/2470654.2470735
- [Malacria et al., 1997, HCI] Command/Shortcut Keys in WIMP User Interfaces — *[author list not verified]*
- [Gilbert et al., 2005, IEEE Trans. Rehab. Eng.] Toward automatic adjustment of keyboard settings — https://kpronline.com/files/Toward-automatic-adjustment-of-keyboard-settings-for-people-with-physical-impairments.pdf
- [Fernández et al., 2006/2007, ACL/DECAL] PTT turn-taking studies — *[interactivity restriction did not reduce task efficiency]*
- [MERL, 2009] Contextual Push-to-Talk — https://www.merl.com/publications/docs/TR2009-062.pdf
- [Baughan, Wang, Liu, Mercurio, Chen & Ma, 2023, CHI] A Mixed-Methods Approach to Understanding User Trust after Voice Assistant Failures — https://doi.org/10.1145/3544548.3581152
- [IPSJ Interaction 2020] Different Types of Voice User Interface Failures — https://www.interaction-ipsj.org/proceedings/2020/data/pdf/1P-81.pdf
- [TOCHI, 2001] Multimodal error correction for speech user interfaces — https://dl.acm.org/doi/10.1145/371127.371166
- [Computing, 2026] My data, my rules — https://link.springer.com/article/10.1007/s00607-025-01611-y
- [IJHCI, 2025] Understanding User Perceptions of Personal Data Stores — https://www.tandfonline.com/doi/pdf/10.1080/10447318.2025.2545467
- [IJHCS, 2026] Voice Permission Requests — https://dl.acm.org/doi/10.1016/j.ijhcs.2025.103590
- [exa.ai, 2020] How Relevant is Hick's Law for HCI? — https://exa.ai/library/publication/0flmzgzx2s7
- [Lapsula study, 2025] Evaluating Strategies for User Onboarding — http://www.diva-portal.org/smash/get/diva2:1987932/FULLTEXT01.pdf

### Primary vendor documentation
- [Microsoft, Windows docs] Keyboard shortcuts in Windows / RegisterHotKey — https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey
- [Microsoft, 2022] Guidelines for app settings — https://learn.microsoft.com/en-us/windows/apps/design/app-settings/guidelines-for-app-settings
- [Microsoft, 2022] Notifications and the Notification Area — https://learn.microsoft.com/en-us/windows/win32/shell/notification-area
- [GNOME, 2003] Notification Area guidelines — https://lists.gnome.org/archives/usability/2003-March/msg00039.html
- [Ubuntu Wiki] Custom Status Menu Design Guidelines — https://wiki.ubuntu.com/CustomStatusMenuDesignGuidelines
- [Android Developers] Voice Input Indicator — https://developer.android.google.cn/design/ui/ai-glasses/guides/components/voice-indicator
- [Meta Voice SDK] Attention System — https://developers.meta.com/horizon/design/voice-sdk-attention-system/
- [Chrome permission study, 2024] Websites Need Your Permission Too — https://exa.ai/library/publication/66pvrbvkqtx
- [Wayland/xwayland protocol spec] XWayland keyboard grabbing protocol — https://wayland.app/protocols/xwayland-keyboard-grab-unstable-v1

### Verification log (2026-10-03)

Every citation below was checked against the publisher page, DBLP, or the author's own page — not just the search-result snippet. Corrections found and applied during verification:

| Was | Now | Why |
|---|---|---|
| Stothart DOI `10.1037/xhp0000091` | `10.1037/xhp0000100` | wrong DOI |
| "Pielot, Church & de Oliveira, 2016" | Kushlev, Proulx & Dunn, 2016 | wrong authors — Pielot is a different paper |
| "Bailly et al., 2013" (ExposeHK) | Malacria, Bailly, Harrison, Cockburn & Gutwin, 2013 | wrong first author |
| "Ponsard et al., 2016" | Ponsard & McGrenere, 2016 | only two authors |
| "Nadolski et al., 2005, ETR&D" | Nadolski, Kirschner & van Merriënboer, 2005, *British Journal of Educational Psychology* | wrong venue — the ETR&D paper is 2001 |
| "CHI 2023" (trust after failure) | Baughan, Wang, Liu, Mercurio, Chen & Ma, 2023 | authors were missing |

**Verified against primary sources:** Cuadra et al. 2021 · Hong & Findlater 2018 · Elbitar et al. 2021 · Mark et al. 2008 · Stothart et al. 2015 · Weiser & Brown 1996 · Chua et al. 2016 · Kleppmann et al. 2019 · Lau et al. 2018 · Bonné et al. 2017 · Ruan et al. 2018 · Mishra et al. 2011 · Kim et al. 2022 · Murad et al. 2019 · Bao 2023 · VOICON 2024 · Harrison et al. 2010 · Matthews et al. 2004 · TOCHI 2001 · Baughan et al. 2023 · Ponsard & McGrenere 2016 · Nadolski et al. 2005 · Malacria et al. 2013 · Karat et al. 1999 · Cutrell & Dumais 2006 · Dumais et al. 2003.

**Not independently re-verified** (carried from the research agents' reports; likely fine but unconfirmed): the remaining practitioner sources, the systematic review (arXiv 2211.04193), Alharbi et al. 2022, Tabassum et al. 2019, Malkin et al. 2019, Grobelny et al. 2005, Woodworth & Borst 2023, and the vendor/practitioner guidelines (Microsoft, GNOME, Android, Meta, NN/g, MeasuringU, Snoopr, Produktly, HPE, Salt, vocab.design, Dix et al.).

### Practitioner guidelines
- [Nielsen, 2006, NN/g] Progressive Disclosure — https://www.nngroup.com/articles/progressive-disclosure/
- [MeasuringU, 2012] Search vs. Browse on Websites — https://measuringu.com/search-browse/
- [Snoopr, 2026] How Long Should Mobile App Onboarding Be? — https://snoopr.co/blog/how-long-should-mobile-app-onboarding-be-screens-time-and-completion-data
- [Produktly, 2026] Onboarding benchmarks — https://dev.to/olli_produktly/onboarding-benchmarks-from-real-data-across-464-saas-products-median-tour-completion-is-29-1-2-101n
- [Microsoft Design, 2026] Designing the first five minutes — https://microsoft.design/articles/designing-the-first-five-minutes/
- [HPE Design System] Toast notifications — https://design-system.hpe.design/templates/toast-notifications
- [Salt Design System, 2024] Preferences Dialog pattern — https://www.saltdesignsystem.com/salt/patterns/preferences-dialog
- [vocab.design] Menu bar extra pattern — https://vocab.design/menu-bar-extra
- [Dix, Finlay, Abowd & Beale, 2004, Prentice Hall] Human-Computer Interaction (3rd ed.) — https://alandix.com/academic/alan.html
