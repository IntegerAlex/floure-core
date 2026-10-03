# Backend Work & Cross-Environment Robustness

**Scope:** the four backend-dependent UX items, plus the harder engineering question — how to make Floure work across the full matrix of OS versions, distros, compositors, audio servers, microphones, drivers and sample rates.

**Method:** four parallel literature searches (ASR confidence/error detection; correction interfaces; VAD/endpointing; cross-environment validation) plus a direct audit of `audio.rs`, `vad.rs` and `pipeline.rs`.

**Evidence tags:** **[P]** peer-reviewed · **[V]** primary vendor/standard docs · **[G]** practitioner · **[I]** inference.

---

# Part 1 — The four backend items

## 1.1 Confidence-based self-repair

**The evidence says: possible, but hard, and it should be rare and non-blocking.**

**What confidence signal is actually available.** The literature splits into three families: decoder predictor features, approximate posterior probability, and utterance verification. For end-to-end models the usable signals are **per-token softmax probability, the negative entropy of the predictive distribution, and their aggregation to word/utterance level**. **[P]** (Jiang 2005, Speech Communication; Oneata et al., SLT 2021)

- **Raw max-probability is a weak, overconfident signal.** Entropy-based measures beat it for both CTC and RNN-T: exponentially normalised **Tsallis entropy (α ≈ 1/3) with *minimum* aggregation** was 1.5–4× better at detecting incorrect words, and could filter ~40% of hallucinations on pure-noise audio at ~5% cost on correct words. **[P]** (Laptev & Ginsburg, SLT 2022)
- **Transducer-specific traps.** Word posteriors cannot be well approximated from the tree-like lattice (each token conditions on full history); autoregressive decoders are overconfident; and **deletion errors carry no word-confidence mass at all**. **[P]** (Qiu et al., Interspeech 2021)
- **A fixed threshold will not generalise.** Optimal thresholds varied *between and within* models; precision/recall hovered ~0.5 with AUC 0.68–0.87. **[P]** (Kuhn et al., CHI EA 2025). Calibration also degrades under noise — Whisper assigned confidence >0.7 to **10–20% of wrongly predicted tokens** at low SNR, and selective temperature scaling cut ECE ~58%. **[P]** (Huo et al., ASRU 2025). Speaker adaptation is a real effect. **[P]** (Del-Agua et al., TASLP 2018)
- **Two dictation-specific papers are blunt about it.** Suhm et al. (TOCHI 2001): confidence-based correction "may not expedite error correction." Feng & Sears (TOCHI 2004): confidence scores are "unlikely to be useful for error detection" in dictation. **[P]**
- **Reference-free detection has other levers.** Linguistic signals work without any confidence score: LM perplexity/grammaticality, repetition loops, implausible token/duration ratios. Litman et al. predicted user corrections at 15.7% error vs 25.7–29.0% baselines using prosody + confidence + POS + dialogue features. **[P]**

**Applied to Floure — the honest position:**

1. **Today we cannot compute any of this.** The sherpa-onnx safe wrapper exposes only `text`, `tokens`, `timestamps`, `durations` — no logprobs. The zero-fork path to `ys_log_probs` is documented in `selective-gate-and-eval.md` and remains the prerequisite. Until then, confidence-based repair is **not buildable**, only fakeable — which is exactly what we should not do.
2. **If we do expose logprobs**, compute **Tsallis entropy (α ≈ 1/3, min aggregation)**, not raw max-probability, and treat it as an overconfident signal requiring calibration.
3. **Do not use a fixed global threshold.** Calibrate per user/environment from a small set of confirmed correct/incorrect dictations, and start at a **high-precision, conservative** operating point.
4. **Repair only on a high-precision detected failure, and never block the typed output.** Cuadra et al.: self-repair after a *real* error improves assessment, and the positive effect outweighs the harm of overcorrection — but overcorrection when nothing was wrong degrades it. A subtle, dismissible prompt ("I didn't catch that — try again?") is an appropriate open-class other-initiated repair; keep it rare.
5. **The better-supported path is user-initiated, not automatic** — see §1.3 (re-speaking with context + a local correction cache), which needs no confidence scores at all and has stronger evidence behind it.

## 1.2 Confidence highlighting / click-to-edit — do not build this as a speed feature

This was in the original plan as evidence-backed. **The evidence says the opposite.**

- **Suhm, Myers & Waibel (TOCHI 2001)** — the condition that *highlighted* likely errors was **slower** than the same interface without highlighting (Tukey HSD = 1.2 cwpm, n.s.), at 89% tagging accuracy. Their conclusion: at that accuracy, highlighting hurts overall performance, and a gain needs a more reliable confidence measure. **[P]**
- **Vertanen & Kristensson (CHI 2008)** — the decisive nuance. Underlining low-confidence text produced **no overall detection improvement** (84% vs 81%, n.s.). Split by case: when an error *was* correctly underlined, detection rose 81%→92% (p < .001); when the confidence measure *missed* an error, detection fell 82%→71% (n.s.). Net zero — gains cancel losses. Their warning: "users tend to trust confidence visualization," so a bad confidence measure makes things *worse*. **[P]**
- **Kuhn, Kersken & Zimmermann (CHI EA 2025)** — 36 users, end-to-end ASR: highlighting **neither improved correction efficiency nor was perceived as helpful**, and did not change confidence in transcript accuracy. Over-highlighting was disliked; no configuration helped. **[P]**
- **Trust framing (Lee & See 2004; Parasuraman & Riley 1997)** — displays that expose automation reliability support *calibrated* trust; unreliable ones drive over-reliance and, after false alarms, disuse. **[P]**

**Applied to Floure:** do not ship per-word confidence colouring. If we expose confidence at all, it should be visually quiet (underline, not colour), gated on a high-precision signal, framed as "possible error" rather than a verdict, and preferably a *frequency* summary ("3 low-confidence words") rather than per-word marks. Note this only matters once a confidence signal exists at all — see §1.1.

## 1.3 Context-aware correction — this is the supported path

- **Shi & Zhou (2011)** — tested three kinds of "external information": word alternative hypotheses, *noisy* context (uncorrected ASR around the sentence), and *accurate* context (already-corrected surrounding sentences). **Accurate context improved both correction outcome and user perception**; noisy context helped less; **word alternatives alone did not help**. Note the citation: this is **Shi & Zhou**, not "Zhou et al." as the earlier doc had it. **[P]**
- **Re-speaking with context beats re-speaking alone.** Vertanen & Kristensson (ASRU 2009): one word of surrounding context raised respeaking alignment success from **65% to 84%**. Isolated hyper-articulated words break the recogniser's context model, which is why re-speaking alone is the *worst* method (Suhm et al.). **[P]**
- **Feeding corrections back improves future recognition.** Yu et al. (INTERSPEECH 2004): >10% relative WER reduction adapting from user corrections. EvolveCaptions (ASSETS 2025): fine-tuning Whisper from corrected segments dropped WER 0.53→0.27 over rounds. **[P]**

**Applied to Floure:** two concrete, buildable wins — (a) when the user re-speaks a correction, pass the corrected preceding sentence as context to the correction recogniser; (b) keep a **local correction cache** and reuse accepted corrections to bias the recogniser's vocabulary. This is the evidence-backed path to fewer repeat errors, and it needs no confidence scores.

## 1.4 Overcapture prevention and VAD tuning

**Findings**

- **`min_silence_duration = 0.25 s` is on the aggressive end.** Conversational endpointing recommendations are 300–500 ms (Deepgram); Silero's own default is 100 ms. 250 ms is defensible for dictation but is the most split-prone setting. **[G]**
- **`max_speech_duration = 5.0 s` is the real problem.** sherpa/k2 use a 20 s hard cap; 5 s guarantees low latency but **hard-cuts every 5 seconds**, splitting long dictation mid-phrase at any pause. **[V]**
- **Trailing silence needs ~600 ms, not 500 ms.** WER "saturates" around 600 ms of trailing silence; 200 ms is explicitly "aggressive" and degrades WER (arXiv 2505.17070). Our 0.5 s stop-flush is at the low edge — 0.6–0.8 s is safer, especially for slow speakers. **[P]**
- **The "final segment stays open and is dropped" failure is the delayed-emission problem.** Streaming/transducer models emit the final token *after* the endpointer fires, so a fixed silence pad cannot guarantee the trailing token was emitted. Fixes in the literature: an end-of-word token plus synchronising the endpoint with the last emitted token; two-pass endpoint arbitration (ICASSP 2024) cut early cut-offs 16–32% relative at no median-latency cost. **[P]**
- **A fixed VAD threshold of 0.5 is not evidence-backed across microphones.** Microphone variability is a first-class robustness problem (Junqua 1997; Mic2Mic, IMWUT 2020: WER varied 23.5%–41.1% across three mics vs 6.5% advertised). Adaptive thresholding from noise-floor estimates is consistently better than fixed thresholds. Silero's `speech_pad_ms` (default 30 ms) prevents clipping final phonemes. **[P]**
- **Linear resampling is a poor anti-alias filter.** Linear interpolation gives ~26 dB image suppression with a rolled-off passband; windowed-sinc/polyphase FIR gives 50+ dB. 48 kHz→16 kHz is exactly 1/3, which suits polyphase. Anti-aliasing matters: 48→16 kHz should use a cut-off centred near 8 kHz. **[P][G]**

**Applied to Floure (concrete):**
1. Raise the stop-flush to **0.6–0.8 s** (from 0.5 s), derived from the negotiated rate.
2. Reconsider **`max_speech_duration`** — 5 s silently splits long dictation. Raise toward 10–20 s, or emit the segment and continue rather than treating 5 s as a hard boundary.
3. Add **`speech_pad_ms ≈ 30`** so final phonemes aren't clipped. **Not possible at sherpa-onnx 1.13.7** — `SileroVadModelConfig` exposes only `model`, `threshold`, `min_silence_duration`, `min_speech_duration`, `window_size` and `max_speech_duration`. This would need the native config path.
4. Add a **per-device noise-floor calibration** (measure ~300 ms of ambient silence at capture start, shift the VAD threshold), rather than a universal 0.5. **Deferred** — it needs real-mic validation, not a guess.
5. Replace the linear resampler with a **polyphase/windowed-sinc** one (48k→16k = 1/3; 44.1k→16k = 160/147). Linear gives ~26 dB image suppression vs 50+ dB for polyphase. **Deferred** — sherpa-onnx only ships `LinearResampler`; a replacement is its own task.

---

# Part 2 — Cross-environment robustness (the "everyone has different hardware" problem)

## 2.1 The evidence that this dominates

- **~17% of bug reports are resolved non-reproducible at least once**; of classified root causes, **Environmental Differences are 24%** (after Interbug Dependencies at 45%). **[P]** (Erfani Joorabchi et al., MSR 2014)
- **Configuration errors are 27% of customer cases and cause the largest share (31%) of high-severity support requests**; 21.7–57.3% of misconfigurations live *outside* the application. Of "used-to-work" regressions: **hardware changes 18%**, software upgrades 14%. **[P]** (Yin et al., SOSP 2011)
- **65% of "resource unavailability" cases are an identifier pointing at a non-existent resource**; 33.6% are semantic misinterpretations, many **silent** (no message). **[P]** (arXiv 2412.11121) — this is exactly our device-by-index/name lookup.
- Cross-platform bug *nature* differs: desktop skews to build/validation, mobile to concurrency. **[P]** (EASE 2015)

**Applied:** the 48 kHz stop-flush bug is textbook *constraint violation* — a length computed from an assumed rate. The rule that follows: **thread the negotiated `sample_rate` and `channels` through every length/timing computation; never a literal.**

## 2.2 Direct audit of `audio.rs` — concrete gaps

Read against the real cpal surface:

1. **Only 3 of cpal's 10 sample formats are handled** (`audio.rs:62-100`). `F32`/`I16`/`U16` work; **`I32` (common on pro audio and some ALSA/PipeWire setups) and `F64` (some drivers) error out** — the app refuses to record. `I8`/`U8`/`I64`/`U32`/`U64` likewise.
2. **The stable device ID is computed and thrown away** (`audio.rs:32` builds `device.id()`, but matching is by **name string** at `audio.rs:47`, and selection is by **array index** at `pipeline.rs:641`). If device order shifts (USB replug, Bluetooth, a new virtual sink), the saved index silently selects the wrong device.
3. **No hotplug handling anywhere.** Unplug mid-dictation → the stream error goes to `eprintln!` (`audio.rs:60`) and the pipeline keeps running against a dead stream.
4. **No capability fallback.** `default_input_config()` is the only attempt (`audio.rs:54`).
5. **Minor:** i16 divides by `i16::MAX` (32767), so `i16::MIN` maps to −1.00003, slightly outside `[-1, 1]`. Divide by 32768.
6. **Bluetooth profile switching unhandled.** A2DP (44.1/48 kHz, stereo) → HFP (8/16 kHz, mono) tears down and re-creates the audio node mid-recording; nothing detects it.

## 2.3 Testing strategy: test the seam, not the device

The literature's core pattern is a **hardware abstraction layer + test doubles at the boundary**, with a small set of real-hardware tests. **[G]**

- Define a `CaptureSource` trait returning `(samples, rate, channels, format)`.
- Implement `CpalCaptureSource` (real) and `FakeCaptureSource` (scripted: 8/16/44.1/48 kHz, mono/stereo, silence, noise, DC, clipping).
- Run the **whole** resample→VAD→segment chain offline in CI against the fake — no audio device needed.
- Keep a small **hardware smoke job** (nightly, self-hosted) that opens real devices at several rates and asserts the flush window equals the configured duration *at the actual rate*.
- Test doubles drift — keep them at boundaries we don't own (device/OS/clock), and pair them with a real-dependency suite.

## 2.4 Invariants testable without a microphone

Property-based / metamorphic testing is the standard oracle-free approach for ASR and audio. **[P]** (ASRTest, ISSTA 2022; AequeVox; CrossASR++) Concrete properties for Floure:

| Property | Statement |
|---|---|
| **Length preservation** | `resample(x, r→16000)` yields `ceil(n·16000/r) ± 1` frames |
| **Rate invariance** | Same speech at 16/44.1/48 kHz → same transcript (or bounded WER) — **directly catches the 48 kHz bug class** |
| **Flush invariant** | After stop, emitted silence duration == configured window **at the negotiated rate**; property over rates {8k,16k,44.1k,48k} × channels {1,2} |
| **Silence-in → silence-out** | No spurious segments from silence |
| **Finiteness** | No NaN/Inf for any finite input |
| **Bounded output** | Samples ≤ full scale (catches the i16 asymmetry) |
| **Channel invariance** | Mono vs duplicated stereo → same transcript |
| **Downmix correctness** | Interleaved frames average to the expected mono value (already partly tested) |

Fuzz the PCM path and any model-input parsing with `cargo-fuzz`; OSS-Fuzz has found >13,000 vulnerabilities across 1,000 projects. **[V]**

## 2.5 Detect capabilities, don't assume them

- **Feature detection beats version sniffing.** **[V]** (MDN/W3C)
- **Wayland typing/hotkeys:** the `org.freedesktop.portal.RemoteDesktop` and `GlobalShortcuts` portals are **compositor-dependent** — GNOME/KDE implement them; wlroots/Hyprland/Sway historically do not. Probe availability and fall back (our loopback server) rather than assuming. **[V]**
- **Audio negotiation:** PipeWire runs one graph rate (default 48 kHz) and resamples; `node.rate`/`allowed-rates` negotiate. WASAPI **shared mode must use the engine mix format** (same rate + channels) — the engine resamples; exclusive mode takes the device directly. ALSA has **no capability query without opening the PCM** and reading `snd_pcm_hw_params`. **[V]**
- **Capture at the device's native rate and resample in-process** — do not force 16 kHz at capture.

## 2.6 Graceful degradation & field diagnostics

- Detect failed capabilities, continue with the critical subset, re-enable when capability returns. **[P]** (Shelton & Koopman, WRES 2001)
- **Diagnostics that respect privacy:** a local, opt-out, *anonymous* capability/health record — OS build, compositor, audio server, device id, negotiated rate/channels/format, error class — bucketed, no serials or usernames, honour `DO_NOT_TRACK`. This is the cheapest way to convert "works on my machine" into "environmental difference, category X." **[V]** (Firefox/ChromiumOS/VS Code telemetry designs)

---

# Part 3 — Prioritised actions

| # | Action | Evidence | Effort |
|---|---|---|---|
| 1 | Handle all cpal sample formats (at minimum I32, F64) instead of erroring | **[V]** cpal surface | low |
| 2 | Thread negotiated rate/channels through every length/timing computation | **[P]** SOSP'11, MSR'14 | low |
| 3 | Add `FakeCaptureSource` + rate-invariance & flush-invariant property tests | **[P]** ISSTA'22 | medium |
| 4 | Device selection by stable `id()`, not name/index | **[P]** arXiv 2412.11121 | low |
| 5 | Raise stop-flush to 0.6–0.8 s at the negotiated rate | **[P]** arXiv 2505.17070 | trivial |
| 6 | Reconsider `max_speech_duration` (5 s silently splits dictation) | **[V]** sherpa 20 s cap | low |
| 7 | Add `speech_pad_ms ≈ 30`; per-device noise-floor calibration | **[P]** Silero; Li et al. | medium |
| 8 | Polyphase resampler instead of linear | **[P]** JASA 2020 | medium |
| 9 | Hotplug recovery on the cpal error callback | **[V]** cpal docs | medium |
| 10 | Re-speaking correction with surrounding context + local correction cache | **[P]** Shi & Zhou; ASRU 2009; Yu 2004 | medium |
| 11 | **Do not** ship per-word confidence colouring | **[P]** Suhm 2001; Vertanen 2008; Kuhn 2025 | — |
| 12 | Anonymous local capability record for field diagnosis | **[V]** telemetry designs | low |

---

# Sources

## Peer-reviewed
- Jiang, 2005, Speech Communication 45(4):455–470 — https://doi.org/10.1016/j.specom.2004.12.004
- Oneata et al., 2021, IEEE SLT — https://arxiv.org/abs/2101.05525
- Laptev & Ginsburg, 2022, IEEE SLT — https://arxiv.org/abs/2212.08703
- Qiu et al., 2021, Interspeech — https://doi.org/10.21437/Interspeech.2021-1207
- Gitman et al., 2023, Interspeech — https://doi.org/10.21437/Interspeech.2023-1281
- Yu, Li & Deng, 2011, IEEE TASLP 19(8):2461–2473 — https://doi.org/10.1109/TASL.2011.2141988
- Huo, Zhang & Tang, 2025, ASRU — https://arxiv.org/abs/2509.07195
- Del-Agua et al., 2018, IEEE/ACM TASLP 26(7):1198–1206 — https://doi.org/10.1109/TASLP.2018.2819900
- Feng & Sears, 2004, TOCHI 11(4):329–356 — https://doi.org/10.1145/1035575.1035576
- Litman, Swerts & Hirschberg, 2006, Computational Linguistics 32(3):417–438 — https://doi.org/10.1162/coli.2006.32.3.417
- Meripo & Konam, 2022, Interspeech — https://arxiv.org/abs/2207.10849
- Moore, An & Marrese, 2024, PACM HCI 8(CSCW1) — https://doi.org/10.1145/3641026
- Suhm, Myers & Waibel, 2001, TOCHI 8(1):60–98 — https://doi.org/10.1145/371127.371166
- Vertanen & Kristensson, 2008, CHI — https://doi.org/10.1145/1357054.1357288
- Kuhn, Kersken & Zimmermann, 2025, CHI EA — https://doi.org/10.1145/3706599.3720038
- Nowrin & Vertanen, 2025, PETRA — https://doi.org/10.1145/3733155.3734896
- Shi & Zhou, 2011, Behaviour & Information Technology 30(6):761–774 — https://doi.org/10.1080/01449290903353039
- Vertanen & Kristensson, 2009, ASRU — https://pokristensson.com/pubs/VertanenKristenssonASRU2009.pdf
- Yu et al., 2004, INTERSPEECH — https://www.isca-archive.org/interspeech_2004/yu04f_interspeech.html
- Wu et al., 2025, ASSETS (EvolveCaptions) — https://soundability.eecs.umich.edu/img/portfolio/Wu_EvolveCaptions_ASSETS2025.pdf
- Lee & See, 2004, Human Factors 46(1):50–80 — https://doi.org/10.1518/hfes.46.1.50_30392
- Parasuraman & Riley, 1997, Human Factors 39(2):230–253 — https://doi.org/10.1518/001872097778543886
- Erfani Joorabchi et al., 2014, MSR — https://doi.org/10.1145/2597073.2597098
- Yin et al., 2011, SOSP — https://www.sigops.org/s/conferences/sosp/2011/current/2011-Cascais/12-yin-online.pdf
- Zhou et al., 2015, EASE — https://www.cs.ucr.edu/~neamtiu/pubs/ease15zhou2.pdf
- Ghosh, Tsiartas & Narayanan, 2010, IEEE TASLP 19(3) — https://doi.org/10.1109/TASL.2010.2052803
- Junqua, 1997, Eurospeech — https://www.isca-archive.org/eurospeech_1997/junqua97_eurospeech.pdf
- Mic2Mic, 2020, IMWUT (arXiv:2003.12425) — https://arxiv.org/abs/2003.12425
- Vincent et al., 2017, Computer Speech & Language 46 — https://www.merl.com/publications/docs/TR2016-172.pdf
- Li et al., 2016, IWAENC — https://doi.org/10.1109/IWAENC.2016.7602911
- Liu & Picheny, 1998, ICSLP — https://www.isca-archive.org/icslp_1998/liu98b_icslp.pdf
- Bauerecker et al., 2003, Eurospeech — https://www.isca-archive.org/eurospeech_2003/bauerecker03_eurospeech.pdf
- JASA 147(3):EL221, 2020 — https://pubs.aip.org/asa/jasa/article/147/3/EL221/997262
- Liu et al., 2015, Interspeech (expected pause duration) — https://www.isca-archive.org/interspeech_2015/liu15d_interspeech.pdf
- ASRTest, 2022, ISSTA — https://conf.researchr.org/details/issta-2022/issta-2022-technical-papers/33/
- Shelton & Koopman, 2001, WRES — https://users.ece.cmu.edu/~koopman/roses/wres01/shelton_wres01.pdf
- Two-Pass Endpoint Detection, 2024, ICASSP — https://arxiv.org/abs/2401.08916

## Preprints (not peer-reviewed)
- Improving endpoint detection in streaming ASR, 2025 — https://arxiv.org/abs/2505.17070
- Adaptive Endpointing with Contextual Bandits, 2023 — https://arxiv.org/abs/2303.13407
- Rethinking Software Misconfigurations, 2024 — https://arxiv.org/abs/2412.11121
- Anti-aliasing and ASR/VAD, 2025 — https://arxiv.org/pdf/2508.02483
- On training targets for noise-robust VAD — https://arxiv.org/abs/2102.07445

## Primary vendor/standard documentation
- cpal `DeviceTrait` — https://docs.rs/cpal/latest/cpal/traits/trait.DeviceTrait.html
- PipeWire properties — https://docs.pipewire.org/page_man_pipewire-props_7.html
- WASAPI device formats — https://learn.microsoft.com/en-us/windows/win32/coreaudio/device-formats
- ALSA hw params — https://alsa-project.org/alsa-doc/alsa-lib/group___p_c_m___h_w___params.html
- XDG RemoteDesktop portal — https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html
- XDG GlobalShortcuts portal — https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html
- WirePlumber Bluetooth — https://stefanu21.pages.freedesktop.org/wireplumber/daemon/configuration/bluetooth.html
- OSS-Fuzz — https://google.github.io/oss-fuzz/
- MDN feature detection — https://developer.mozilla.org/en-US/docs/Learn_web_development/Extensions/Testing/Feature_detection

## Practitioner
- Martin Fowler, Mocks Aren't Stubs — https://www.martinfowler.com/articles/mocksArentStubs.html
- Silero VAD quality metrics — https://github.com/snakers4/silero-vad/wiki/Quality-Metrics
- k2/sherpa endpointing — https://k2-fsa.github.io/sherpa/python/streaming_asr/endpointing.html
- CCRMA windowed-sinc resampling — https://ccrma.stanford.edu/~jos/resample/resample.html

## Verification status
Verified against primary sources: Suhm 2001, Vertanen 2008, Shi & Zhou 2011, Lee & See 2004, Kuhn 2025 (DOI), Liu 2015, Karat 1999, Hong & Findlater 2018, Cuadra 2021.
Not independently re-verified (carried from agent reports, likely fine): MSR 2014 (rate-limited during check), SOSP 2011, the arXiv preprints, and the vendor docs (fetched by the agents, not re-fetched here).
