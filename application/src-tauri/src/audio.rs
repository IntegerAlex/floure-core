use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub struct AudioCapture {
    _stream: cpal::Stream,
    pub sample_rate: u32,
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

pub fn start_capture<F>(device_name: Option<&str>, mut callback: F) -> Result<AudioCapture>
where
    F: FnMut(&[f32]) + Send + 'static,
{
    let host = cpal::default_host();
    let device = match device_name {
        Some(name) => host
            .input_devices()?
            .find(|d| d.to_string() == name)
            .ok_or_else(|| anyhow::anyhow!("Device not found: {}", name))?,
        None => host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No default input device"))?,
    };

    let supported = device.default_input_config()?;
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

    let err_fn = |err| eprintln!("Audio stream error: {:?}", err);

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
                    callback(&mono);
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
    fn interleaved_frames_are_averaged_per_frame_not_globally() {
        // 4 frames of 2 channels: averaging must be per frame, in order.
        let mono = normalize_to_mono(&[1.0f32, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0], 2);
        assert_eq!(mono, vec![0.5, 0.5, 1.0, 0.0]);
    }
}
