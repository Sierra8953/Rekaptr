use crate::config::MicSettings;

/// Real-time mic DSP processor operating on interleaved F32LE samples.
/// All processing is done in-place on the sample buffer.
pub struct MicDsp {
    sample_rate: f32,
    channels: usize,

    // Gain
    gain_linear: f32,

    // Noise gate state
    gate_enabled: bool,
    gate_threshold_linear: f32,
    gate_attack_coeff: f32,
    gate_release_coeff: f32,
    gate_envelope: f32,

    // Compressor state
    comp_enabled: bool,
    comp_threshold_linear: f32,
    comp_ratio: f32,
    comp_attack_coeff: f32,
    comp_release_coeff: f32,
    comp_envelope: f32,

    // Limiter state
    limiter_enabled: bool,
    limiter_threshold_linear: f32,

    // Mono downmix
    force_mono: bool,
}

impl MicDsp {
    pub fn new(sample_rate: f32, channels: usize) -> Self {
        Self {
            sample_rate,
            channels,
            gain_linear: 1.0,
            gate_enabled: false,
            gate_threshold_linear: 0.0,
            gate_attack_coeff: 0.0,
            gate_release_coeff: 0.0,
            gate_envelope: 0.0,
            comp_enabled: false,
            comp_threshold_linear: 1.0,
            comp_ratio: 1.0,
            comp_attack_coeff: 0.0,
            comp_release_coeff: 0.0,
            comp_envelope: 0.0,
            limiter_enabled: false,
            limiter_threshold_linear: 1.0,
            force_mono: false,
        }
    }

    /// Reload DSP parameters from config. Call when settings change.
    pub fn load_settings(&mut self, s: &MicSettings) {
        self.gain_linear = db_to_linear(s.gain_db);
        self.force_mono = s.force_mono;

        // Noise gate
        self.gate_enabled = s.noise_gate_enabled;
        self.gate_threshold_linear = db_to_linear(s.noise_gate_threshold);
        self.gate_attack_coeff = time_constant(s.noise_gate_attack as f32, self.sample_rate);
        self.gate_release_coeff = time_constant(s.noise_gate_release as f32, self.sample_rate);

        // Compressor
        self.comp_enabled = s.compressor_enabled;
        self.comp_threshold_linear = db_to_linear(s.compressor_threshold);
        self.comp_ratio = s.compressor_ratio;
        self.comp_attack_coeff = time_constant(s.compressor_attack as f32, self.sample_rate);
        self.comp_release_coeff = time_constant(s.compressor_release as f32, self.sample_rate);

        // Limiter (hard knee, instant attack, ~5ms release)
        self.limiter_enabled = s.limiter_enabled;
        self.limiter_threshold_linear = db_to_linear(s.limiter_threshold);
    }

    /// Process interleaved F32LE samples in-place.
    pub fn process(&mut self, samples: &mut [f32]) {
        let channels = self.channels;
        if channels == 0 {
            return;
        }

        // Apply gain
        if self.gain_linear != 1.0 {
            for s in samples.iter_mut() {
                *s *= self.gain_linear;
            }
        }

        // Mono downmix: average all channels into each frame
        if self.force_mono && channels >= 2 {
            for frame in samples.chunks_exact_mut(channels) {
                let sum: f32 = frame.iter().sum();
                let mono = sum / channels as f32;
                for ch in frame.iter_mut() {
                    *ch = mono;
                }
            }
        }

        // Noise gate (per-frame envelope follower)
        if self.gate_enabled {
            for frame in samples.chunks_exact_mut(channels) {
                let peak = frame.iter().fold(0.0_f32, |m, s| m.max(s.abs()));
                let coeff = if peak > self.gate_envelope {
                    self.gate_attack_coeff
                } else {
                    self.gate_release_coeff
                };
                self.gate_envelope = coeff * self.gate_envelope + (1.0 - coeff) * peak;

                if self.gate_envelope < self.gate_threshold_linear {
                    for ch in frame.iter_mut() {
                        *ch = 0.0;
                    }
                }
            }
        }

        // Compressor (feed-forward, peak-based)
        if self.comp_enabled {
            for frame in samples.chunks_exact_mut(channels) {
                let peak = frame.iter().fold(0.0_f32, |m, s| m.max(s.abs()));
                let coeff = if peak > self.comp_envelope {
                    self.comp_attack_coeff
                } else {
                    self.comp_release_coeff
                };
                self.comp_envelope = coeff * self.comp_envelope + (1.0 - coeff) * peak;

                if self.comp_envelope > self.comp_threshold_linear {
                    let over_db = linear_to_db(self.comp_envelope) - linear_to_db(self.comp_threshold_linear);
                    let gain_reduction_db = over_db * (1.0 - 1.0 / self.comp_ratio);
                    let gain = db_to_linear(-gain_reduction_db);
                    for ch in frame.iter_mut() {
                        *ch *= gain;
                    }
                }
            }
        }

        // Limiter (brick-wall, sample-level)
        if self.limiter_enabled {
            let thresh = self.limiter_threshold_linear;
            for s in samples.iter_mut() {
                if s.abs() > thresh {
                    *s = thresh * s.signum();
                }
            }
        }
    }
}

#[inline]
fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

#[inline]
fn linear_to_db(linear: f32) -> f32 {
    if linear <= 0.0 {
        -96.0
    } else {
        20.0 * linear.log10()
    }
}

/// Convert attack/release time in ms to a one-pole smoothing coefficient.
#[inline]
fn time_constant(time_ms: f32, sample_rate: f32) -> f32 {
    if time_ms <= 0.0 {
        return 0.0;
    }
    (-1.0 / (time_ms * 0.001 * sample_rate)).exp()
}
