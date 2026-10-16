//! `process` allocates and frees nothing, with every slot in use, every structure changing and
//! values, the gain scale, the output gain, auto gain, solo and every band's dynamics all moving.

use mxm_fx_eq_n_dsp::{Band, Dynamics, Engine, Placement, SLOTS, Shape, Slope};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct TrackingAllocator;

thread_local! {
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static OPERATIONS: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        TRACKING.with(|tracking| {
            if tracking.get() {
                OPERATIONS.with(|operations| operations.set(operations.get() + 1));
            }
        });
        // SAFETY: this allocator delegates the exact layout to the process allocator.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        TRACKING.with(|tracking| {
            if tracking.get() {
                OPERATIONS.with(|operations| operations.set(operations.get() + 1));
            }
        });
        // SAFETY: `pointer` and `layout` came from this delegate.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

#[test]
fn processing_allocates_and_frees_nothing() {
    let mut engine = Box::new(Engine::new(48_000.0));
    let mut left = vec![0.0f32; 2_048];
    let mut right = vec![0.0f32; 2_048];
    let mut mono = vec![0.0f32; 333];
    // A side chain for every keyed call, made before the guard starts counting.
    let key: Vec<f32> = (0..2_048)
        .map(|i| ((i * 13) % 100) as f32 / 50.0 - 1.0)
        .collect();

    OPERATIONS.with(|operations| operations.set(0));
    TRACKING.with(|tracking| tracking.set(true));
    for block in 0..200usize {
        for slot in 0..SLOTS {
            let pick = block + slot;
            engine.set_band(
                slot,
                Band {
                    shape: (pick % 10 != 0).then(|| Shape::ALL[pick % Shape::ALL.len()]),
                    bypass: pick % 13 == 0,
                    freq: 20.0 + 700.0 * ((pick * 7) % 30) as f32,
                    gain_db: ((pick % 61) as f32) - 30.0,
                    q: 0.1 + (pick % 20) as f32,
                    slope: Slope::ALL[(pick / 3) % Slope::ALL.len()],
                },
            );
            engine.set_dynamics(
                slot,
                Dynamics {
                    range_db: ((pick * 11) % 61) as f32 - 30.0,
                    threshold_db: -(((pick * 3) % 61) as f32),
                    attack_ms: 0.5 + (pick % 7) as f32,
                    release_ms: 10.0 + (pick % 9) as f32 * 50.0,
                },
            );
        }
        for slot in 0..SLOTS {
            engine.set_side_chain(slot, (block + slot) % 2 == 0);
            engine.set_placement(slot, Placement::ALL[(block + slot) % Placement::ALL.len()]);
        }
        engine.set_solo((block % 3 == 0).then_some(block % SLOTS));
        engine.set_gain_scale((block % 5) as f32 * 0.5);
        engine.set_output_gain(0.5 + (block % 4) as f32 * 0.5);
        // Auto gain on most blocks, its make-up estimated as the bands move (plan §7.8).
        engine.set_auto_gain(block % 7 != 0);
        for (index, (l, r)) in left.iter_mut().zip(right.iter_mut()).enumerate() {
            *l = ((index * 31 + block) % 200) as f32 / 100.0 - 1.0;
            *r = -*l;
        }
        engine.process_stereo(&mut left, &mut right);
        engine.process_stereo_keyed(&mut left, &mut right, Some([&key, &key]));
        engine.process_mono(&mut mono);
        engine.process_mono_keyed(&mut mono, Some(&key));
        if block % 50 == 0 {
            engine.reset();
        }
        std::hint::black_box((&left, &right, engine.is_ringing(), engine.is_dry()));
    }
    TRACKING.with(|tracking| tracking.set(false));
    let operations = OPERATIONS.with(Cell::get);
    assert_eq!(operations, 0, "process performed allocator operations");
}

/// **Linear phase allocates nothing on the audio thread either** (plan §7.7), in realtime mode, with
/// every slot changing every block: kernels are asked for and taken through preallocated banks,
/// the design on its own thread.
#[test]
fn linear_phase_processing_allocates_and_frees_nothing() {
    use mxm_fx_eq_n_dsp::{Phase, Resolution};
    let mut engine = Box::new(Engine::with_phase(
        48_000.0,
        Phase::Linear(Resolution::Medium),
        2,
    ));
    engine.prime();
    engine.reset();
    let mut left = vec![0.0f32; 1_024];
    let mut right = vec![0.0f32; 1_024];

    OPERATIONS.with(|operations| operations.set(0));
    TRACKING.with(|tracking| tracking.set(true));
    for block in 0..120usize {
        for slot in 0..SLOTS {
            let pick = block + slot;
            engine.set_band(
                slot,
                Band {
                    shape: (pick % 10 != 0).then(|| Shape::ALL[pick % Shape::ALL.len()]),
                    bypass: pick % 13 == 0,
                    freq: 20.0 + 700.0 * ((pick * 7) % 30) as f32,
                    gain_db: ((pick % 61) as f32) - 30.0,
                    q: 0.1 + (pick % 20) as f32,
                    slope: Slope::ALL[(pick / 3) % Slope::ALL.len()],
                },
            );
            engine.set_placement(slot, Placement::ALL[(block + slot) % Placement::ALL.len()]);
        }
        engine.set_solo((block % 3 == 0).then_some(block % SLOTS));
        engine.set_gain_scale((block % 5) as f32 * 0.5);
        engine.set_output_gain(0.5 + (block % 4) as f32 * 0.5);
        for (index, (l, r)) in left.iter_mut().zip(right.iter_mut()).enumerate() {
            *l = ((index * 31 + block) % 200) as f32 / 100.0 - 1.0;
            *r = -*l;
        }
        engine.process_stereo(&mut left, &mut right);
        if block % 40 == 0 {
            engine.reset();
        }
        std::hint::black_box((&left, &right, engine.is_ringing(), engine.is_settled()));
    }
    TRACKING.with(|tracking| tracking.set(false));
    let operations = OPERATIONS.with(Cell::get);
    assert_eq!(operations, 0, "process performed allocator operations");
}
