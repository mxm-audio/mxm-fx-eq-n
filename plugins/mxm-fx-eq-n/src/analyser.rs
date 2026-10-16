//! The analyser's audio half: blocks of pre-EQ and post-EQ audio, and the side chain's key while
//! the editor's Side chain view wants it (plan §7.4), from the audio thread to the editor (plan §4). The editor's half, the history, FFT and display, is E3's and lives with the
//! editor; the FFT will be the shared `crates/mxm-fft`.
//!
//! **The handoff meets plan §4's requirements:**
//! - **Coherent, ordered windows.** The audio thread fills one preallocated [`Block`] at a time and
//!   pushes it into a bounded single-producer, single-consumer ring (`rtrb`). Every block carries a
//!   sequence number that counts every block made, pushed or not, so a block dropped on a full ring
//!   is a gap the editor sees, and it restarts its window rather than splicing.
//! - **Breaks are marked.** Activation, deactivation and reset spend a sequence number and start a
//!   new generation, and a block carries the sample rate it was recorded at, so audio either side of
//!   a break is never one window.
//! - **Audio never blocks, allocates or waits.** A full ring drops the block.
//! - **Every channel is kept** separately in a block; v1's editor draws their power average.
//! - **No editor, no work.** With the editor closed or the analyser off, nothing is copied.

/// Frames per block: 5.3 ms at 48 kHz.
pub const BLOCK_FRAMES: usize = 256;

/// Blocks the ring holds: 170 ms at 192 kHz, where an editor drains about twelve a frame.
pub const RING_BLOCKS: usize = 128;

/// One block of audio, every channel kept. A mono layout carries its channel in both.
#[derive(Clone, Copy)]
pub struct Block {
    pub seq: u64,
    /// The stretch of unbroken processing it came from.
    pub generation: u32,
    /// The sample rate it was recorded at; the editor ignores any other.
    pub sample_rate: f32,
    /// 1 for mono, 2 for stereo.
    pub channels: u8,
    pub pre: [[f32; BLOCK_FRAMES]; 2],
    pub post: [[f32; BLOCK_FRAMES]; 2],
    /// The side-chain input, while the editor's Side chain view wants it (plan §7.4); silence
    /// otherwise.
    pub key: [[f32; BLOCK_FRAMES]; 2],
}

impl Block {
    const SILENT: Block = Block {
        seq: 0,
        generation: 0,
        sample_rate: 48_000.0,
        channels: 2,
        pre: [[0.0; BLOCK_FRAMES]; 2],
        post: [[0.0; BLOCK_FRAMES]; 2],
        key: [[0.0; BLOCK_FRAMES]; 2],
    };
}

/// The audio thread's half: one block being filled, and the ring's producer.
pub struct Tap {
    producer: rtrb::Producer<Block>,
    block: Box<Block>,
    fill: usize,
    seq: u64,
    generation: u32,
    sample_rate: f32,
    wanted: bool,
}

/// The ring, split: the [`Tap`] stays with the plugin and the consumer goes to the editor.
pub fn ring() -> (Tap, rtrb::Consumer<Block>) {
    ring_with(RING_BLOCKS)
}

/// A ring of `blocks` blocks: the analyser's, or the share tap's another track's editor reads
/// (plan §7.8).
pub fn ring_with(blocks: usize) -> (Tap, rtrb::Consumer<Block>) {
    let (producer, consumer) = rtrb::RingBuffer::new(blocks);
    (
        Tap {
            producer,
            block: Box::new(Block::SILENT),
            fill: 0,
            seq: 0,
            generation: 0,
            sample_rate: 48_000.0,
            wanted: false,
        },
        consumer,
    )
}

impl Tap {
    /// Once per block. Coming back after a pause is a gap, so the window restarts.
    pub fn set_wanted(&mut self, wanted: bool) {
        if wanted && !self.wanted {
            self.fill = 0;
            self.seq = self.seq.wrapping_add(1);
        }
        self.wanted = wanted;
    }

    pub fn wanted(&self) -> bool {
        self.wanted
    }

    /// A break in the audio (activation at `sample_rate`, deactivation, reset): the partial block
    /// is dropped, a sequence number is spent, and blocks from here on carry a new generation.
    /// Never allocates or waits.
    pub fn discontinuity(&mut self, sample_rate: f32) {
        self.fill = 0;
        self.seq = self.seq.wrapping_add(1);
        self.generation = self.generation.wrapping_add(1);
        self.sample_rate = sample_rate;
    }

    /// Appends post-EQ frames alone, pre and key left silent: the share tap's (plan §7.8). Each
    /// slice is the same length; mono passes its channel twice.
    pub fn capture_post(&mut self, channels: u8, post: [&[f32]; 2]) {
        if !self.wanted {
            return;
        }
        for (&left, &right) in post[0].iter().zip(post[1]) {
            self.block.post[0][self.fill] = left;
            self.block.post[1][self.fill] = right;
            self.fill += 1;
            if self.fill == BLOCK_FRAMES {
                self.push(channels);
            }
        }
    }

    /// The full block out, numbered; a full ring drops it, its sequence number spent anyway, so
    /// the editor sees the gap.
    fn push(&mut self, channels: u8) {
        self.block.seq = self.seq;
        self.block.generation = self.generation;
        self.block.sample_rate = self.sample_rate;
        self.block.channels = channels;
        self.seq = self.seq.wrapping_add(1);
        let _ = self.producer.push(*self.block);
        self.fill = 0;
    }

    /// Appends frames. Each slice is the same length; mono passes its channel twice. `key` is the
    /// side chain's, or none, which is silence.
    pub fn capture(
        &mut self,
        channels: u8,
        pre: [&[f32]; 2],
        post: [&[f32]; 2],
        key: Option<[&[f32]; 2]>,
    ) {
        if !self.wanted {
            return;
        }
        for index in 0..pre[0].len() {
            for channel in 0..2 {
                self.block.pre[channel][self.fill] = pre[channel][index];
                self.block.post[channel][self.fill] = post[channel][index];
                self.block.key[channel][self.fill] = key
                    .and_then(|key| key[channel].get(index).copied())
                    .unwrap_or(0.0);
            }
            self.fill += 1;
            if self.fill == BLOCK_FRAMES {
                self.push(channels);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain(consumer: &mut rtrb::Consumer<Block>) -> Vec<Block> {
        let mut blocks = Vec::new();
        while let Ok(block) = consumer.pop() {
            blocks.push(block);
        }
        blocks
    }

    #[test]
    fn unwanted_audio_is_never_copied() {
        let (mut tap, mut consumer) = ring();
        let frames = [0.5f32; BLOCK_FRAMES];
        for _ in 0..10 {
            tap.capture(2, [&frames, &frames], [&frames, &frames], None);
        }
        assert!(drain(&mut consumer).is_empty());
    }

    #[test]
    fn blocks_are_numbered_in_order_and_keep_both_streams() {
        let (mut tap, mut consumer) = ring();
        tap.set_wanted(true);
        let pre: Vec<f32> = (0..BLOCK_FRAMES * 3).map(|n| n as f32).collect();
        let post: Vec<f32> = pre.iter().map(|x| -x).collect();
        // Uneven pieces, as a host's buffers come.
        let mut at = 0;
        for piece in [100, 7, 300, 361] {
            let end = at + piece;
            tap.capture(
                1,
                [&pre[at..end], &pre[at..end]],
                [&post[at..end], &post[at..end]],
                None,
            );
            at = end;
        }
        let blocks = drain(&mut consumer);
        assert_eq!(blocks.len(), 3);
        for (index, block) in blocks.iter().enumerate() {
            assert_eq!(block.seq, blocks[0].seq + index as u64);
            assert_eq!(block.channels, 1);
            assert_eq!(block.pre[0][0], (index * BLOCK_FRAMES) as f32);
            assert_eq!(
                block.post[1][BLOCK_FRAMES - 1],
                -(((index + 1) * BLOCK_FRAMES - 1) as f32)
            );
        }
    }

    #[test]
    fn a_break_drops_the_partial_block_and_starts_a_generation() {
        let (mut tap, mut consumer) = ring();
        tap.discontinuity(48_000.0);
        tap.set_wanted(true);
        let frames = [0.25f32; BLOCK_FRAMES];
        tap.capture(2, [&frames, &frames], [&frames, &frames], None);
        tap.capture(
            2,
            [&frames[..100], &frames[..100]],
            [&frames[..100], &frames[..100]],
            None,
        );
        tap.discontinuity(44_100.0);
        tap.capture(2, [&frames, &frames], [&frames, &frames], None);
        let blocks = drain(&mut consumer);
        assert_eq!(
            blocks.len(),
            2,
            "the partial block was dropped, not completed"
        );
        assert!(
            blocks[1].seq > blocks[0].seq + 1,
            "the break spent a number"
        );
        assert_ne!(blocks[0].generation, blocks[1].generation);
        assert_eq!(
            (blocks[0].sample_rate, blocks[1].sample_rate),
            (48_000.0, 44_100.0)
        );
    }

    #[test]
    fn a_full_ring_drops_and_the_gap_is_visible() {
        let (mut tap, mut consumer) = ring();
        tap.set_wanted(true);
        let frames = [0.25f32; BLOCK_FRAMES];
        for _ in 0..RING_BLOCKS + 3 {
            tap.capture(2, [&frames, &frames], [&frames, &frames], None);
        }
        let seqs: Vec<u64> = drain(&mut consumer).iter().map(|block| block.seq).collect();
        assert_eq!(seqs.len(), RING_BLOCKS);
        tap.capture(2, [&frames, &frames], [&frames, &frames], None);
        let next = consumer.pop().unwrap().seq;
        assert!(
            next > *seqs.last().unwrap() + 1,
            "the dropped blocks spent their numbers"
        );
    }

    #[test]
    fn coming_back_after_a_pause_is_a_gap() {
        let (mut tap, mut consumer) = ring();
        tap.set_wanted(true);
        let frames = [0.25f32; BLOCK_FRAMES];
        tap.capture(2, [&frames, &frames], [&frames, &frames], None);
        tap.set_wanted(false);
        tap.capture(2, [&frames, &frames], [&frames, &frames], None);
        tap.set_wanted(true);
        tap.capture(2, [&frames, &frames], [&frames, &frames], None);
        let blocks = drain(&mut consumer);
        assert_eq!(blocks.len(), 2);
        assert!(blocks[1].seq > blocks[0].seq + 1);
    }
    /// **The key rides with pre and post** (plan §7.4); none is silence.
    #[test]
    fn a_block_carries_the_key_or_silence() {
        let (mut tap, mut consumer) = ring();
        tap.set_wanted(true);
        let frames = vec![0.25f32; BLOCK_FRAMES];
        let key = vec![-0.5f32; BLOCK_FRAMES];
        tap.capture(
            2,
            [&frames, &frames],
            [&frames, &frames],
            Some([&key, &key]),
        );
        let block = consumer.pop().expect("a block");
        assert_eq!(block.key[0], [-0.5; BLOCK_FRAMES]);
        tap.capture(2, [&frames, &frames], [&frames, &frames], None);
        let block = consumer.pop().expect("a block");
        assert_eq!(block.key[1], [0.0; BLOCK_FRAMES]);
    }
}
