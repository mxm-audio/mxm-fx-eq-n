//! `process` allocates and frees nothing, with every slot in use, every structure changing and
//! values, the gain scale, the output gain and solo all moving.

use mxm_fx_eq_n_dsp::{Band, Engine, SLOTS, Shape, Slope};
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
        }
        engine.set_solo((block % 3 == 0).then_some(block % SLOTS));
        engine.set_gain_scale((block % 5) as f32 * 0.5);
        engine.set_output_gain(0.5 + (block % 4) as f32 * 0.5);
        for (index, (l, r)) in left.iter_mut().zip(right.iter_mut()).enumerate() {
            *l = ((index * 31 + block) % 200) as f32 / 100.0 - 1.0;
            *r = -*l;
        }
        engine.process_stereo(&mut left, &mut right);
        engine.process_mono(&mut mono);
        if block % 50 == 0 {
            engine.reset();
        }
        std::hint::black_box((&left, &right, engine.is_ringing(), engine.is_dry()));
    }
    TRACKING.with(|tracking| tracking.set(false));
    let operations = OPERATIONS.with(Cell::get);
    assert_eq!(operations, 0, "process performed allocator operations");
}
