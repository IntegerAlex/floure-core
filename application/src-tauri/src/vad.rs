use anyhow::Result;
use sherpa_onnx::{VadModelConfig, VoiceActivityDetector as SherpaVad};

/// Trailing silence the VAD needs before it closes a segment.
///
/// Silero's own default is 0.1s and conversational endpointing guidance is
/// 0.3-0.5s. 0.25s sits between them: dictation pauses fall mostly at sentence
/// boundaries, so a shorter threshold buys responsiveness without many
/// mid-phrase splits.
pub const MIN_SILENCE_SECS: f32 = 0.25;

/// Shortest burst treated as speech, so a cough or key click is not a segment.
pub const MIN_SPEECH_SECS: f32 = 0.25;

/// Hard cap on one segment, after which it is emitted regardless of pauses.
///
/// 5s fired on ordinary continuous sentences and truncated the recogniser's
/// context mid-phrase. 10s halves the number of cuts while still keeping text
/// flowing for a long monologue; sherpa-onnx's reference cap is 20s, which
/// would leave a continuous speaker waiting too long to see anything.
pub const MAX_SPEECH_SECS: f32 = 10.0;

/// Silero v5's native chunk at 16 kHz (512 samples = 32 ms). Larger windows
/// measurably degrade VAD accuracy, so this is not a tuning knob.
pub const WINDOW_SIZE: i32 = 512;

pub struct VoiceActivityDetector {
    vad: SherpaVad,
    buffer: Vec<f32>,
    offset: usize,
    window_size: usize,
}

impl VoiceActivityDetector {
    pub fn new(model_path: &std::path::Path, threshold: f32) -> Result<Self> {
        let mut config = VadModelConfig::default();
        config.silero_vad.model = Some(model_path.to_str().unwrap().into());
        config.silero_vad.threshold = threshold;
        config.silero_vad.min_silence_duration = MIN_SILENCE_SECS;
        config.silero_vad.min_speech_duration = MIN_SPEECH_SECS;
        config.silero_vad.max_speech_duration = MAX_SPEECH_SECS;
        config.silero_vad.window_size = WINDOW_SIZE;
        config.sample_rate = 16000;
        config.debug = false;

        let vad = SherpaVad::create(&config, 60.0)
            .ok_or_else(|| anyhow::anyhow!("Failed to create Silero VAD"))?;

        Ok(Self {
            vad,
            buffer: Vec::new(),
            offset: 0,
            window_size: WINDOW_SIZE as usize,
        })
    }

    pub fn feed(&mut self, samples: &[f32]) {
        self.buffer.extend_from_slice(samples);

        while self.offset + self.window_size <= self.buffer.len() {
            self.vad
                .accept_waveform(&self.buffer[self.offset..self.offset + self.window_size]);
            self.offset += self.window_size;
        }
    }

    pub fn try_get_segment(&mut self) -> Option<Vec<f32>> {
        if !self.vad.is_empty() {
            if let Some(segment) = self.vad.front() {
                self.vad.pop();
                return Some(segment.samples().to_vec());
            }
        }
        None
    }

    pub fn reset_after_segment(&mut self) {
        // Drain only consumed windows. Clearing the whole buffer here
        // starves the VAD whenever resampled chunks are smaller than the
        // 512-sample window (e.g. ~160 samples per 10ms chunk from a 48kHz
        // mic): nothing would ever reach accept_waveform again.
        self.buffer.drain(..self.offset);
        self.offset = 0;
    }
}
