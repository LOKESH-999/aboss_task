use std::{
    alloc::{Layout, alloc, dealloc},
    cell::Cell,
    marker::PhantomData,
    ptr::NonNull,
    sync::{
        Arc,
        atomic::{
            AtomicUsize,
            Ordering::{Acquire, Release},
            fence,
        },
    },
};

use crate::utils::{bound_index, calculate_stream_mean};

/// Raw statistical data snapshot.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RawData {
    /// Minimum value observed
    pub min: f64,
    /// Maximum value observed
    pub max: f64,
    /// Streaming mean (average) of all values
    pub curr_avg: f64,
    /// Simple moving average over the configured window
    pub sma: f64,
    /// Number of data points observed
    pub data_point: u64,
}

/// Unsafe fixed-size queue for internal numeric storage.
///  
/// Provides manual memory management for fast circular buffer operations.
pub struct UnsafeQueue<T: Copy> {
    ptr: NonNull<T>,
    capacity: usize,
    _phantom: PhantomData<T>,
}

impl<T: Copy> UnsafeQueue<T> {
    /// Allocates a new UnsafeQueue with a fixed capacity.
    pub fn new(capacity: usize) -> Self {
        let layout = Layout::array::<T>(capacity).expect("Layout initialization for array failed");
        let ptr = NonNull::new(unsafe { alloc(layout) as *mut T })
            .expect("Memory allocation failed for UnsafeQueue");
        UnsafeQueue {
            ptr,
            capacity,
            _phantom: PhantomData,
        }
    }

    /// Sets value at the given index. Unsafe: no bounds checks in release.
    #[inline(always)]
    pub unsafe fn set(&self, val: T, idx: usize) {
        debug_assert!(idx < self.capacity);
        unsafe { self.ptr.as_ptr().add(idx).write(val) }
    }

    /// Gets value at the given index. Unsafe: no bounds checks in release.
    #[inline(always)]
    pub unsafe fn get(&self, idx: usize) -> T {
        debug_assert!(idx < self.capacity);
        unsafe { self.ptr.as_ptr().add(idx).read() }
    }

    /// Swaps value at index with a new value, returning the old value.
    #[inline(always)]
    pub unsafe fn swap(&self, idx: usize, val: T) -> T {
        debug_assert!(idx < self.capacity);
        unsafe {
            let old = self.get(idx);
            self.set(val, idx);
            old
        }
    }
}

impl<T: Copy> Drop for UnsafeQueue<T> {
    fn drop(&mut self) {
        let layout = Layout::array::<T>(self.capacity).unwrap();
        unsafe { dealloc(self.ptr.as_ptr() as *mut u8, layout) };
    }
}

/// A lock-free processor that maintains streaming statistics and SMA (Simple Moving Average).
///
/// Can be split into a `DataProcessorReader` and `DataProcessorWriter` for
/// concurrent single-writer, multiple-reader usage.
pub struct DataProcessor {
    raw_data: [Cell<RawData>; 2],
    /// Indicates which index (0 or 1) is safe for readers.
    active2read: AtomicUsize,
    /// Circular buffer for SMA calculations
    queue: UnsafeQueue<f64>,
    /// Current running SMA value
    curr_sma_avg: Cell<f64>,
    /// Current index in the circular SMA buffer
    curr_queue_idx: Cell<usize>,
}

impl DataProcessor {
    /// Splits the processor into a reader and writer pair.
    ///
    /// # Arguments
    /// - `sma_n_size`: window size for the simple moving average
    /// - `initial_data`: initial seed value for statistics
    pub fn split(
        sma_n_size: usize,
        initial_data: f64,
    ) -> (DataProcessorReader, DataProcessorWriter) {
        assert!(sma_n_size > 0, "SMA window size must be > 0");

        let raw_data = RawData {
            curr_avg: initial_data,
            max: initial_data,
            min: initial_data,
            sma: initial_data,
            data_point: 1,
        };
        let active2read = 0.into();

        let queue = UnsafeQueue::new(sma_n_size);
        for idx in 0..sma_n_size {
            // Initialize SMA buffer with the seed value
            unsafe {
                queue.set(initial_data, idx);
            }
        }

        let inner = Arc::new(Self {
            raw_data: [raw_data.into(), raw_data.into()],
            active2read,
            queue,
            curr_sma_avg: initial_data.into(),
            curr_queue_idx: 0.into(),
        });

        let reader = DataProcessorReader {
            inner: inner.clone(),
        };
        let writer = DataProcessorWriter {
            inner,
            _no_clone: NoClone,
        };
        (reader, writer)
    }

    /// Updates statistics with a new data point.
    ///
    /// Updates:
    /// - min / max
    /// - streaming mean (`curr_avg`)
    /// - simple moving average (`sma`)
    /// - data point count
    fn write(&self, new_data: f64) {
        // Load current reader index (0 or 1)
        let idx = self.active2read.load(Acquire);
        fence(Acquire); // ensure memory ordering for data
        let old_raw = self.raw_data[idx].get();

        // Update min/max and data point count
        let min = old_raw.min.min(new_data);
        let max = old_raw.max.max(new_data);
        let data_point = old_raw.data_point + 1;

        // Streaming mean (online update)
        let curr_avg = calculate_stream_mean(old_raw.curr_avg, new_data, data_point);

        // Simple Moving Average (SMA) update
        let sma = {
            let b_idx = bound_index(self.curr_queue_idx.get(), self.queue.capacity);
            self.curr_queue_idx.set(b_idx + 1);

            // Swap new value into circular buffer and get popped value
            let popped = unsafe { self.queue.swap(b_idx, new_data) };

            // Update running SMA in O(1) time
            let new_sma = self.curr_sma_avg.get() - (popped / self.queue.capacity as f64)
                + (new_data / self.queue.capacity as f64);
            self.curr_sma_avg.set(new_sma);
            new_sma
        };

        let new_raw = RawData {
            curr_avg,
            max,
            min,
            sma,
            data_point,
        };
        let bounded_idx = bound_index(idx + 1, 2);

        // Write into the "inactive" slot so readers see consistent snapshot
        self.raw_data[bounded_idx].set(new_raw);
        fence(Release); // ensure ordering before publishing
        self.active2read.store(bounded_idx, Release); // switch active reader index
    }

    /// Reads the latest snapshot of statistics
    pub fn read(&self) -> RawData {
        let idx = self.active2read.load(Acquire);
        self.raw_data[idx].get()
    }
}

/// Marker to prevent cloning of writer
struct NoClone;

/// Writer handle for `DataProcessor`
pub struct DataProcessorWriter {
    inner: Arc<DataProcessor>,
    _no_clone: NoClone,
}

impl DataProcessorWriter {
    /// Add a new data point
    pub fn write(&self, new_data: f64) {
        self.inner.write(new_data);
    }
}

/// Reader handle for `DataProcessor`
#[derive(Clone)]
pub struct DataProcessorReader {
    inner: Arc<DataProcessor>,
}

impl DataProcessorReader {
    /// Read the current statistics snapshot
    pub fn read(&self) -> RawData {
        self.inner.read()
    }
}

// SAFETY: Single-writer, multi-reader semantics
unsafe impl Send for DataProcessor {}
unsafe impl Send for DataProcessorWriter {}
unsafe impl Send for DataProcessorReader {}
unsafe impl Sync for DataProcessor {}

#[cfg(test)]
mod dataproc_tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering as AtomicOrdering},
    };
    use std::thread;
    use std::time::{Duration, Instant};

    // Helper: approx equality for f64
    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() <= eps.max(a.abs().max(b.abs()) * 1e-6)
    }

    #[test]
    fn test_initial_state() {
        let (r, _w) = DataProcessor::split(4, 1.0);
        let s = r.read();
        assert_eq!(s.min, 1.0);
        assert_eq!(s.max, 1.0);
        assert!(approx_eq(s.curr_avg, 1.0, 1e-12));
        assert!(approx_eq(s.sma, 1.0, 1e-12));
        assert_eq!(s.data_point, 1);
    }

    #[test]
    fn test_single_writer_updates_and_invariants() {
        let (r, w) = DataProcessor::split(3, 2.0);
        // initial
        let s0 = r.read();
        assert_eq!(s0.data_point, 1);

        // write sequence
        w.write(4.0); // now seen values: 2.0(initial seeded *3), then 4.0
        let s1 = r.read();
        assert!(s1.max >= s1.min);
        assert!(s1.data_point >= s0.data_point);
        // streaming mean should have increased
        assert!(s1.curr_avg >= s0.curr_avg);

        w.write(0.0);
        let s2 = r.read();
        assert!(s2.min <= s1.min);
        assert!(s2.max >= s1.max);
        assert!(s2.data_point >= s1.data_point);

        // More writes to fill SMA window and rotate ring
        w.write(10.0);
        w.write(5.0);

        let s3 = r.read();
        // sanity: sma must be between min and max of the last window approximately
        assert!(s3.sma >= s3.min - 1e-12 && s3.sma <= s3.max + 1e-12);
    }

    #[test]
    fn test_sma_correctness_small_window() {
        // Use a small window so we can compute expected SMA easily
        let window = 4usize;
        let (r, w) = DataProcessor::split(window, 1.0);

        // initial queue: [1,1,1,1], sma = 1
        let mut expected_buf = vec![1.0; window];
        let mut expected_sum: f64 = expected_buf.iter().sum();
        assert!(approx_eq(r.read().sma, expected_sum / window as f64, 1e-12));

        let inputs = [2.0, 3.0, 4.0, 5.0, 6.0];
        for &x in &inputs {
            w.write(x);
            // rotate expected buffer
            let popped = expected_buf.remove(0);
            expected_buf.push(x);
            expected_sum = expected_sum - popped + x;

            let snap = r.read();
            let expected_sma = expected_sum / window as f64;
            assert!(
                approx_eq(snap.sma, expected_sma, 1e-9),
                "sma mismatch: got {} expected {} (buf {:?})",
                snap.sma,
                expected_sma,
                expected_buf
            );
        }
    }

    #[test]
    fn test_streaming_mean_growth_and_monotonic_data_point() {
        let (r, w) = DataProcessor::split(5, 10.0);
        let mut last = r.read();
        for i in 1..50 {
            let v = (i as f64) * 0.5;
            w.write(v);
            let cur = r.read();
            // data point increments by at least 1
            assert!(cur.data_point >= last.data_point);
            // curr_avg should be between min and max of observed values
            assert!(cur.curr_avg >= cur.min - 1e-12 && cur.curr_avg <= cur.max + 1e-12);
            last = cur;
        }
    }

    #[test]
    fn test_concurrent_readers_single_writer_stress() {
        // spawn many readers that continuously read while writer updates
        let (reader, writer) = DataProcessor::split(16, 0.0);
        let reader = Arc::new(reader);
        let writer = Arc::new(writer);

        let stop = Arc::new(AtomicBool::new(false));
        let mut handles = Vec::new();

        // Spawn 8 reader threads
        for _ in 0..8 {
            let r = Arc::clone(&reader);
            let stop_c = Arc::clone(&stop);
            handles.push(thread::spawn(move || {
                // each reader continuously reads and checks invariants
                while !stop_c.load(AtomicOrdering::Relaxed) {
                    let s = r.read();
                    // basic invariants:
                    assert!(s.max >= s.min);
                    assert!(s.data_point >= 1);
                    // sma shouldn't be NaN
                    assert!(s.sma.is_finite());
                    // avoid busy spin too aggressively
                    std::thread::yield_now();
                }
            }));
        }

        // Writer thread: perform lots of writes for 200ms
        let w = Arc::clone(&writer);
        let writer_handle = thread::spawn(move || {
            let start = Instant::now();
            let mut v = 0.0f64;
            while start.elapsed() < Duration::from_millis(200) {
                v += 1.0;
                w.write(v); // single-writer only
            }
        });

        // wait for writer to finish
        writer_handle.join().expect("writer panicked");
        // stop readers
        stop.store(true, AtomicOrdering::Relaxed);
        for h in handles {
            h.join().expect("reader panicked");
        }

        // final sanity read from main thread
        let final_snap = reader.read();
        assert!(final_snap.data_point > 1);
        assert!(final_snap.sma.is_finite());
    }

    // Optional heavy stress test (long-running) commented out by default.
    // Remove the cfg attribute to run it.
    // #[cfg(feature = "heavy-stress")]
    #[test]
    fn heavy_stress_test_many_reads_writes() {
        let (r, w) = DataProcessor::split(32, 100.0);
        let r = Arc::new(r);
        let w = Arc::new(w);
        let stop = Arc::new(AtomicBool::new(false));

        let mut readers = Vec::new();
        for _ in 0..16 {
            let rr = Arc::clone(&r);
            let stopc = Arc::clone(&stop);
            readers.push(thread::spawn(move || {
                while !stopc.load(AtomicOrdering::Relaxed) {
                    let s = rr.read();
                    // quick consistency checks
                    assert!(s.max >= s.min);
                    assert!(s.data_point >= 1);
                }
            }));
        }

        let writer = Arc::clone(&w);
        let writer_thread = thread::spawn(move || {
            for i in 0..200_000 {
                writer.write((i as f64) * 0.1);
            }
        });

        writer_thread.join().unwrap();
        stop.store(true, AtomicOrdering::Relaxed);
        for th in readers {
            th.join().unwrap();
        }

        let s = r.read();
        assert!(s.data_point > 1);
    }
}
