//! Lock-free, lossy editor telemetry: whether an editor is open, the analyser's switch, the sample
//! rate, the Out meter and each band's gains. Observation only; the DSP reads none of it.
//!
//! **Each band's two gains** (plan §7.1): the drawn gain where the DSP has glided it, and the gain
//! heard, the drawn plus the dynamic moving part held inside ±30 dB, both before the gain scale.
//! The editor's moving curve draws the heard one, and its meter their difference, both under the
//! gain scale where the DSP has glided it, published beside them.
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
    /// The layout's channel count, 1 or 2: in mono a placed band is disabled (plan §7.3).
    channels: AtomicU32,
    /// Whether the layout has the side-chain port (plan §7.4): all the plugin can observe of a
    /// route, so the Side chain view is offered only then.
    side_chain_port: AtomicBool,
    /// The editor's Side chain view, so the audio thread copies the key only when it is drawn.
    side_chain_view: AtomicBool,
    /// The Out meter, post output gain: left, right.
    pub output: [PeakMeter; 2],
    /// Each slot's drawn and heard gains in dB, as `f32` bits.
    gains: [[AtomicU32; 2]; mxm_fx_eq_n_dsp::SLOTS],
    /// The gain scale the gains are heard under now, as `f32` bits.
    heard_scale: AtomicU32,
}

impl Default for Telemetry {
    fn default() -> Self {
        Self {
            connected: AtomicBool::new(false),
            analyser_on: AtomicBool::new(true),
            sample_rate: AtomicU32::new(48_000f32.to_bits()),
            channels: AtomicU32::new(2),
            side_chain_port: AtomicBool::new(false),
            side_chain_view: AtomicBool::new(false),
            output: [PeakMeter::new(), PeakMeter::new()],
            gains: std::array::from_fn(|_| [AtomicU32::new(0), AtomicU32::new(0)]),
            heard_scale: AtomicU32::new(1f32.to_bits()),
        }
    }
}

impl Telemetry {
    /// The editor opening or closing. Either clears the meter: a block that read "connected" just
    /// before a close can still publish after it, and a reopened editor must not show that.
    pub fn connect(&self, connected: bool) {
        self.connected.store(connected, Ordering::Release);
        for meter in &self.output {
            let _ = meter.take();
            meter.clear_clip();
        }
        for gains in &self.gains {
            for gain in gains {
                gain.store(0, Ordering::Relaxed);
            }
        }
        self.heard_scale.store(1f32.to_bits(), Ordering::Relaxed);
    }

    /// The gain scale where the DSP has glided it, from the audio thread after a block.
    pub fn publish_heard_scale(&self, scale: f32) {
        self.heard_scale.store(scale.to_bits(), Ordering::Relaxed);
    }

    /// The gain scale the published gains are heard under: 1 before any.
    pub fn heard_scale(&self) -> f32 {
        f32::from_bits(self.heard_scale.load(Ordering::Relaxed))
    }

    /// A slot's drawn and heard gains, from the audio thread after a block.
    pub fn publish_gains(&self, slot: usize, drawn_db: f32, heard_db: f32) {
        if let Some([drawn, heard]) = self.gains.get(slot) {
            drawn.store(drawn_db.to_bits(), Ordering::Relaxed);
            heard.store(heard_db.to_bits(), Ordering::Relaxed);
        }
    }

    /// A slot's last published drawn and heard gains, in dB: `(0, 0)` before any.
    pub fn gains(&self, slot: usize) -> (f32, f32) {
        self.gains.get(slot).map_or((0.0, 0.0), |[drawn, heard]| {
            (
                f32::from_bits(drawn.load(Ordering::Relaxed)),
                f32::from_bits(heard.load(Ordering::Relaxed)),
            )
        })
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

    pub fn publish_side_chain_port(&self, present: bool) {
        self.side_chain_port.store(present, Ordering::Relaxed);
    }

    pub fn side_chain_port(&self) -> bool {
        self.side_chain_port.load(Ordering::Relaxed)
    }

    pub fn set_side_chain_view(&self, on: bool) {
        self.side_chain_view.store(on, Ordering::Relaxed);
    }

    /// Whether the audio thread should copy the key into the analyser's blocks.
    pub fn side_chain_wanted(&self) -> bool {
        self.analyser_wanted()
            && self.side_chain_port()
            && self.side_chain_view.load(Ordering::Relaxed)
    }

    pub fn publish_channels(&self, channels: usize) {
        self.channels.store(channels as u32, Ordering::Relaxed);
    }

    /// Whether the layout is mono, as the shell last activated it.
    pub fn mono(&self) -> bool {
        self.channels.load(Ordering::Relaxed) == 1
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
    fn a_block_publishing_across_a_close_is_not_shown_on_reopening() {
        let telemetry = Telemetry::default();
        telemetry.connect(true);
        telemetry.connect(false);
        // The block read "connected" before the close and publishes after it.
        telemetry.output[0].publish(1.5);
        telemetry.connect(true);
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
