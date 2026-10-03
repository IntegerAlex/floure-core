use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub struct AudioCapture {
    _stream: cpal::Stream,
    pub sample_rate: u32,
    /// Negotiated channel count. The buffer is interleaved, so the downmix
    /// needs it, and the diagnostics record wants it.
    pub channels: u16,
    /// Negotiated sample format, recorded for environment diagnosis.
    pub format: String,
}

/// Normalise a raw device sample to `f32` in approximately `[-1, 1]`.
///
/// The divisor is the type's full range (`2^(bits-1)`), not its `MAX`: dividing
/// by `i16::MAX` would map `i16::MIN` to -1.00003, just outside the range. The
/// same rule holds for every signed type here.
///
/// Implemented as a trait rather than a closure per call site so that every
/// `SampleFormat` cpal can hand us goes through one conversion path.
trait ToF32: Copy {
    fn to_f32(self) -> f32;
}

impl ToF32 for f32 {
    fn to_f32(self) -> f32 {
        self
    }
}
impl ToF32 for f64 {
    fn to_f32(self) -> f32 {
        self as f32
    }
}
impl ToF32 for i8 {
    fn to_f32(self) -> f32 {
        self as f32 / 128.0
    }
}
impl ToF32 for i16 {
    fn to_f32(self) -> f32 {
        self as f32 / 32768.0
    }
}
impl ToF32 for i32 {
    fn to_f32(self) -> f32 {
        self as f32 / 2_147_483_648.0
    }
}
impl ToF32 for i64 {
    fn to_f32(self) -> f32 {
        self as f32 / 9_223_372_036_854_775_808.0
    }
}
impl ToF32 for u8 {
    fn to_f32(self) -> f32 {
        (self as f32 - 128.0) / 128.0
    }
}
impl ToF32 for u16 {
    fn to_f32(self) -> f32 {
        (self as f32 - 32768.0) / 32768.0
    }
}
impl ToF32 for u32 {
    fn to_f32(self) -> f32 {
        (self as f64 - 2_147_483_648.0) as f32 / 2_147_483_648.0
    }
}
impl ToF32 for u64 {
    fn to_f32(self) -> f32 {
        (self as f64 - 9_223_372_036_854_775_808.0) as f32 / 9_223_372_036_854_775_808.0
    }
}

/// Downmix interleaved multi-channel samples to mono `f32`.
///
/// Each output sample is the arithmetic mean of one `channels`-wide frame. A
/// CPAL input buffer is interleaved, so `channels` must come from the negotiated
/// config — assuming mono is exactly the class of bug this guards against.
///
/// `channels` is clamped to at least 1: a zero would divide by zero and poison
/// the whole stream with NaN, which the VAD and ASR would then have to survive.
fn normalize_to_mono<S: Copy + ToF32>(data: &[S], channels: usize) -> Vec<f32> {
    let channels = channels.max(1);
    data.chunks(channels)
        .map(|frame| {
            let sum: f32 = frame.iter().copied().map(|s| s.to_f32()).sum();
            sum / frame.len() as f32
        })
        .collect()
}

pub fn list_input_devices() -> Result<Vec<(String, String)>> {
    let host = cpal::default_host();
    let mut devices = Vec::new();

    for device in host.input_devices()? {
        let name = device.to_string();
        let id = device.id().map(|d| format!("{:?}", d)).unwrap_or_default();
        devices.push((name, id));
    }

    Ok(devices)
}

/// The sample formats we can convert. `I24` (and any format cpal adds later)
/// has no Rust primitive to read it into, so it is rejected with a clear
/// message rather than building a stream that silently yields nothing.
fn format_supported(f: cpal::SampleFormat) -> bool {
    use cpal::SampleFormat as F;
    matches!(
        f,
        F::F32 | F::F64 | F::I8 | F::I16 | F::I32 | F::I64 | F::U8 | F::U16 | F::U32 | F::U64
    )
}

/// A device's stable, driver-level identifier, as opposed to its display name
/// (which is user-editable and can change between sessions).
fn device_id(device: &cpal::Device) -> Option<String> {
    device.id().ok().map(|d| format!("{:?}", d))
}

/// Find the requested device by stable id, then by display name.
///
/// Both identifiers go stale: ids are backend-scoped, names are user-editable,
/// and device order changes on replug. A miss therefore falls back to the
/// current default device (logged) rather than failing the whole capture —
/// recording from the wrong-but-working mic beats not recording at all.
fn resolve_device(host: &cpal::Host, hint: Option<&str>) -> Result<cpal::Device> {
    let default = || {
        host.default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No default input device"))
    };
    let Some(hint) = hint.filter(|h| !h.is_empty()) else {
        return default();
    };
    let devices: Vec<cpal::Device> = host.input_devices()?.collect();
    if let Some(d) = devices
        .iter()
        .find(|d| device_id(d).as_deref() == Some(hint))
    {
        return Ok(d.clone());
    }
    if let Some(d) = devices.iter().find(|d| d.to_string() == hint) {
        return Ok(d.clone());
    }
    eprintln!("[audio] requested device {hint:?} not found — falling back to the default");
    default()
}

/// The device's default input config, or the best supported fallback.
///
/// `default_input_config` is not always available — some ALSA and virtual
/// devices report none until opened. Enumerating instead of failing is the
/// difference between "this mic works" and "this app does not support my
/// setup". Preference order: f32 (no conversion), a rate that resamples
/// cleanly to 16 kHz, then the fewest channels.
fn choose_input_config(device: &cpal::Device) -> Result<cpal::SupportedStreamConfig> {
    if let Ok(cfg) = device.default_input_config() {
        return Ok(cfg);
    }
    let mut best: Option<((u8, u8, u16), cpal::SupportedStreamConfig)> = None;
    for range in device.supported_input_configs()? {
        let (key, rate) = config_rank(
            range.sample_format(),
            range.min_sample_rate(),
            range.max_sample_rate(),
            range.channels(),
        );
        let candidate = range.with_sample_rate(rate);
        if best.as_ref().is_none_or(|(k, _)| key < *k) {
            best = Some((key, candidate));
        }
    }
    best.map(|(_, c)| c)
        .ok_or_else(|| anyhow::anyhow!("Device reports no supported input configurations"))
}

/// Pick a rate from a device range: 16 kHz exactly, else 48 kHz (1/3 to 16 kHz),
/// else 44.1 kHz (160/147), else the range's maximum. Returns the preference
/// rank alongside the rate so callers can compare ranges.
fn preferred_rate(min: u32, max: u32) -> (u8, u32) {
    for (rank, rate) in [(0u8, 16_000u32), (1, 48_000), (2, 44_100)] {
        if min <= rate && rate <= max {
            return (rank, rate);
        }
    }
    (3, max)
}

/// Rank a supported config range for capture; lower is better.
///
/// f32 first because it needs no conversion, then a rate that resamples cleanly
/// to the 16 kHz the VAD and ASR want, then the fewest channels.
fn config_rank(
    format: cpal::SampleFormat,
    min_rate: u32,
    max_rate: u32,
    channels: u16,
) -> ((u8, u8, u16), u32) {
    let fmt_rank = u8::from(format != cpal::SampleFormat::F32);
    let (rate_rank, rate) = preferred_rate(min_rate, max_rate);
    ((fmt_rank, rate_rank, channels), rate)
}

/// Start capture from `device_hint` — a stable id or a display name; `None`
/// means the system default.
///
/// `on_error` is invoked if the stream dies mid-capture: an unplugged USB mic,
/// a Bluetooth headset switching profile, a device the OS revokes. The caller
/// uses it to stop the run cleanly rather than typing from a frozen buffer.
pub fn start_capture<F, E>(
    device_hint: Option<&str>,
    mut on_samples: F,
    mut on_error: E,
) -> Result<AudioCapture>
where
    F: FnMut(&[f32]) + Send + 'static,
    E: FnMut(String) + Send + 'static,
{
    let host = cpal::default_host();
    let device = resolve_device(&host, device_hint)?;

    let supported = choose_input_config(&device)?;
    let config = supported.config();
    let sample_format = supported.sample_format();
    let sample_rate = supported.sample_rate();
    let channels = config.channels as usize;

    if !format_supported(sample_format) {
        return Err(anyhow::anyhow!(
            "Unsupported sample format {:?} on this device — please report it",
            sample_format
        ));
    }

    // Type inferred from `build_input_stream_raw`'s bound; cpal's error type
    // is Debug-only, not Display.
    let err_fn = move |err| {
        let msg = format!("{err:?}");
        eprintln!("[audio] stream error: {msg}");
        on_error(msg);
    };

    // `build_input_stream_raw` dispatches on the runtime format, so every
    // format above shares one callback instead of a closure per type. Without
    // this, a device reporting I32 or F64 (some pro interfaces and drivers)
    // simply refused to record.
    let stream = device.build_input_stream_raw(
        config,
        sample_format,
        move |data: &cpal::Data, _: &cpal::InputCallbackInfo| {
            let mono = match sample_format {
                cpal::SampleFormat::F32 => data
                    .as_slice::<f32>()
                    .map(|s| normalize_to_mono(s, channels)),
                cpal::SampleFormat::F64 => data
                    .as_slice::<f64>()
                    .map(|s| normalize_to_mono(s, channels)),
                cpal::SampleFormat::I8 => data
                    .as_slice::<i8>()
                    .map(|s| normalize_to_mono(s, channels)),
                cpal::SampleFormat::I16 => data
                    .as_slice::<i16>()
                    .map(|s| normalize_to_mono(s, channels)),
                cpal::SampleFormat::I32 => data
                    .as_slice::<i32>()
                    .map(|s| normalize_to_mono(s, channels)),
                cpal::SampleFormat::I64 => data
                    .as_slice::<i64>()
                    .map(|s| normalize_to_mono(s, channels)),
                cpal::SampleFormat::U8 => data
                    .as_slice::<u8>()
                    .map(|s| normalize_to_mono(s, channels)),
                cpal::SampleFormat::U16 => data
                    .as_slice::<u16>()
                    .map(|s| normalize_to_mono(s, channels)),
                cpal::SampleFormat::U32 => data
                    .as_slice::<u32>()
                    .map(|s| normalize_to_mono(s, channels)),
                cpal::SampleFormat::U64 => data
                    .as_slice::<u64>()
                    .map(|s| normalize_to_mono(s, channels)),
                // Rejected above; kept so the match stays exhaustive.
                _ => None,
            };
            if let Some(mono) = mono {
                if !mono.is_empty() {
                    on_samples(&mono);
                }
            }
        },
        err_fn,
        None,
    )?;

    stream.play()?;

    Ok(AudioCapture {
        _stream: stream,
        sample_rate,
        channels: channels as u16,
        format: format!("{sample_format:?}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmixes_stereo_f32_by_averaging() {
        let mono = normalize_to_mono(&[0.5f32, -0.5, 1.0, 1.0], 2);
        assert_eq!(mono, vec![0.0, 1.0]);
    }

    #[test]
    fn converts_integer_formats_to_unit_range() {
        // Signed types: full scale is the type's range, so MIN maps to exactly -1
        // and MAX maps to just under +1 (32767/32768). The old divisor of
        // i16::MAX made MAX land on exactly 1.0 and MIN overshoot to -1.00003.
        assert_eq!(normalize_to_mono(&[i16::MIN], 1), vec![-1.0]);
        assert_eq!(normalize_to_mono(&[i32::MIN], 1), vec![-1.0]);
        assert!((normalize_to_mono(&[i16::MAX], 1)[0] - 1.0).abs() < 1e-4);
        assert!((normalize_to_mono(&[i8::MAX], 1)[0] - 127.0 / 128.0).abs() < 1e-6);
        // Unsigned types are offset by half their range.
        assert_eq!(normalize_to_mono(&[32768u16], 1), vec![0.0]);
        assert_eq!(normalize_to_mono(&[0u16], 1), vec![-1.0]);
        assert_eq!(normalize_to_mono(&[128u8], 1), vec![0.0]);
        // Floats pass through.
        assert_eq!(normalize_to_mono(&[0.25f64, 0.75], 2), vec![0.5]);
    }

    #[test]
    fn every_supported_format_stays_within_unit_range() {
        // The i16::MAX divisor bug made i16::MIN land at -1.00003. This is the
        // invariant that catches it, and the same class of bug for other types.
        macro_rules! check {
            ($t:ty, $min:expr, $max:expr) => {
                for v in [$min, 0 as $t, $max] {
                    let out = normalize_to_mono(&[v], 1)[0];
                    assert!(
                        (-1.0..=1.0).contains(&out),
                        "{} sample {v} produced {out}, outside [-1, 1]",
                        stringify!($t)
                    );
                }
            };
        }
        check!(i8, i8::MIN, i8::MAX);
        check!(i16, i16::MIN, i16::MAX);
        check!(i32, i32::MIN, i32::MAX);
        check!(u8, u8::MIN, u8::MAX);
        check!(u16, u16::MIN, u16::MAX);
    }

    #[test]
    fn a_zero_channel_count_does_not_divide_by_zero() {
        // Defensive: a bad config would otherwise produce NaN for every sample.
        let mono = normalize_to_mono(&[0.5f32, 0.5], 0);
        assert_eq!(mono, vec![0.5, 0.5]);
        assert!(mono.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn format_supported_covers_the_primitive_formats() {
        use cpal::SampleFormat as F;
        for f in [
            F::F32,
            F::F64,
            F::I8,
            F::I16,
            F::I32,
            F::I64,
            F::U8,
            F::U16,
            F::U32,
            F::U64,
        ] {
            assert!(format_supported(f), "{f:?} should be supported");
        }
        // I24 has no Rust primitive to read it into, so it is rejected up front.
        assert!(!format_supported(F::I24));
    }

    #[test]
    fn config_rank_prefers_f32_then_a_clean_rate_then_fewer_channels() {
        use cpal::SampleFormat as F;
        assert!(
            config_rank(F::F32, 16_000, 16_000, 1) < config_rank(F::I16, 16_000, 16_000, 1),
            "f32 should outrank i16"
        );
        assert!(
            config_rank(F::F32, 16_000, 16_000, 1) < config_rank(F::F32, 48_000, 48_000, 1),
            "16 kHz should outrank 48 kHz"
        );
        assert!(
            config_rank(F::F32, 16_000, 16_000, 1) < config_rank(F::F32, 16_000, 16_000, 2),
            "fewer channels should outrank more"
        );
    }

    #[test]
    fn preferred_rate_falls_back_to_max_when_no_standard_rate_fits() {
        assert_eq!(preferred_rate(8_000, 8_000), (3, 8_000));
        assert_eq!(preferred_rate(8_000, 96_000), (0, 16_000));
        assert_eq!(preferred_rate(44_100, 44_100), (2, 44_100));
        assert_eq!(preferred_rate(48_000, 48_000), (1, 48_000));
    }

    #[test]
    fn interleaved_frames_are_averaged_per_frame_not_globally() {
        // 4 frames of 2 channels: averaging must be per frame, in order.
        let mono = normalize_to_mono(&[1.0f32, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0], 2);
        assert_eq!(mono, vec![0.5, 0.5, 1.0, 0.0]);
    }
}
