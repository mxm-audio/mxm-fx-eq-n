//! Lock-free, lossy editor telemetry: whether an editor is open, the analyser's switch, the sample
//! rate, and the Out meter. Observation only; the DSP reads none of it.
//!
//! The meter is mxm-kit's `PeakMeter`, one per channel, the collection's one policy: a peak is
//! max-combined and reset on read, a peak at or above full scale latches the clip until the player
//! clears it, and a non-finite peak latches the clip and is never stored. Callers go through the
//! field (`telemetry.output[channel].publish(peak)`). With no editor connected the shell publishes
//! nothing (`mxm-fx-curve`'s `connect` gating).

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use mxm_telemetry::PeakMeter;

pub struct Telemetry {
    connected: AtomicBool,
    analyser_on: AtomicBool,
    sample_rate: AtomicU32,
    /// The Out meter, post output gain: left, right.
    pub output: [PeakMeter; 2],
}

impl Default for Telemetry {
    fn default() -> Self {
        Self {
            connected: AtomicBool::new(false),
            analyser_on: AtomicBool::new(true),
            sample_rate: AtomicU32::new(48_000f32.to_bits()),
            output: [PeakMeter::new(), PeakMeter::new()],
        }
    }
}

impl Telemetry {
    /// The editor opening or closing. Closing clears the meter, so a reopened editor starts clean.
    pub fn connect(&self, connected: bool) {
        self.connected.store(connected, Ordering::Release);
        if !connected {
            for meter in &self.output {
                let _ = meter.take();
                meter.clear_clip();
            }
        }
    }

    pub fn connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }

    /// The Analyser card's on/off, a view setting rather than a parameter (plan §4).
    pub fn set_analyser_on(&self, on: bool) {
        self.analyser_on.store(on, Ordering::Relaxed);
    }

    /// Whether the audio thread should hand blocks to the analyser at all.
    pub fn analyser_wanted(&self) -> bool {
        self.connected() && self.analyser_on.load(Ordering::Relaxed)
    }

    pub fn publish_sample_rate(&self, rate: f32) {
        self.sample_rate.store(rate.to_bits(), Ordering::Relaxed);
    }

    pub fn sample_rate(&self) -> f32 {
        f32::from_bits(self.sample_rate.load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_the_editor_clears_the_meter() {
        let telemetry = Telemetry::default();
        telemetry.connect(true);
        telemetry.output[0].publish(1.5);
        telemetry.output[1].publish(0.25);
        assert!(telemetry.output[0].clipped());
        telemetry.connect(false);
        assert_eq!(telemetry.output.each_ref().map(PeakMeter::take), [0.0; 2]);
        assert!(!telemetry.output[0].clipped());
    }

    #[test]
    fn the_analyser_is_wanted_only_with_an_editor_and_its_switch_on() {
        let telemetry = Telemetry::default();
        assert!(!telemetry.analyser_wanted());
        telemetry.connect(true);
        assert!(telemetry.analyser_wanted());
        telemetry.set_analyser_on(false);
        assert!(!telemetry.analyser_wanted());
    }
}
