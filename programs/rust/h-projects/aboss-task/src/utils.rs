use std::mem::transmute;

/// Incrementally calculates the mean of a data stream.
///
/// This function updates the average when a new element is added,
/// without needing to keep the entire dataset in memory.
/// It uses the standard online mean update formula:
///
/// `new_avg = curr_avg + (new_elem - curr_avg) / n`
///
/// # Arguments
/// * `curr_avg` - Current running mean.
/// * `new_elem` - New element to include.
/// * `n` - Total number of elements after adding `new_elem`.
///
/// # Returns
/// The updated mean.
///
/// # Example
/// ```
/// use aboss_task::utils::calculate_stream_mean;
/// let mut avg = 0.0;
/// avg = calculate_stream_mean(avg, 5.0, 1);
/// avg = calculate_stream_mean(avg, 7.0, 2);
/// assert!((avg - 6.0) < 1e-10);
/// ```
pub const fn calculate_stream_mean(curr_avg: f64, new_elem: f64, n: u64) -> f64 {
    curr_avg + ((new_elem - curr_avg) / (n as f64))
}

/// Bounds an index by a given upper limit using branchless masking.
///
/// If `idx < bound_by`, the function returns `idx`.  
/// If `idx >= bound_by`, the function returns `0`.  
///
/// This avoids conditional branching (`if` statements), which can be
/// beneficial in performance-critical code (e.g., hot loops, SIMD-friendly code).
///
/// ## Special Case: Modulus-Like Behavior
/// When a denominator `< bound_by`, this can be used as a branchless
/// alternative to `%` in certain scenarios. Unlike `%`, values beyond
/// the bound reset to `0` instead of wrapping around.
///
/// # Arguments
/// * `idx` - The index to be bounded.
/// * `bound_by` - The exclusive upper bound.
///
/// # Returns
/// `idx` if it is within bounds, otherwise `0`.
///
/// # Example
/// ```
/// use aboss_task::utils::bound_index;
/// assert_eq!(bound_index(3, 5), 3);  // within bounds
/// assert_eq!(bound_index(7, 5), 0);  // out of bounds
///
/// // modulus-like usage
/// let denominator = 4;
/// assert_eq!(bound_index(3, denominator + 1), 3);
/// assert_eq!(bound_index(7, denominator + 1), 0); // resets instead of wrapping
/// ```
pub const fn bound_index(idx: usize, bound_by: usize) -> usize {
    let is_less_mask = isize2usize(-((idx < bound_by) as isize));
    idx & is_less_mask
}

/// Reinterprets an `isize` as a `usize` by bit-casting.
///
/// This performs a raw bit reinterpretation rather than a numeric cast.
/// - Positive values map to the same numeric `usize`.
/// - Negative values map to large `usize` values (due to two's complement).
///
/// This is used internally by [`bound_index`] to generate
/// all-ones (`usize::MAX`) or all-zeros masks in a branchless way.
///
/// # Safety
/// This uses [`std::mem::transmute`] under the hood, but it is safe here
/// because `isize` and `usize` are guaranteed to have the same size
/// on all Rust-supported targets.
///
/// # Arguments
/// * `val` - The signed integer to reinterpret.
///
/// # Returns
/// The unsigned reinterpretation of `val`.
///
/// # Example
/// ```
/// use aboss_task::utils::isize2usize;
/// assert_eq!(isize2usize(5), 5);
/// assert_eq!(isize2usize(-1), usize::MAX); // all bits set
/// ```
pub const fn isize2usize(val: isize) -> usize {
    unsafe { transmute(val) }
}

/// Used to extract Symbol
pub fn extract_symbol(url: &str) -> Option<String> {
    url.split("symbol=").nth(1).map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_stream_mean() {
        let mut avg = 0.0;
        avg = calculate_stream_mean(avg, 5.0, 1);
        assert!((avg - 5.0).abs() < 1e-10);

        avg = calculate_stream_mean(avg, 7.0, 2);
        assert!((avg - 6.0).abs() < 1e-10);

        avg = calculate_stream_mean(avg, 9.0, 3);
        assert!((avg - 7.0).abs() < 1e-10);

        avg = calculate_stream_mean(avg, -3.0, 4);
        assert!((avg - 4.5).abs() < 1e-10);
    }

    #[test]
    fn test_bound_index_within_bounds() {
        assert_eq!(bound_index(0, 5), 0);
        assert_eq!(bound_index(4, 5), 4);
        assert_eq!(bound_index(3, 10), 3);
    }

    #[test]
    fn test_bound_index_out_of_bounds() {
        assert_eq!(bound_index(5, 5), 0);
        assert_eq!(bound_index(10, 5), 0);
        assert_eq!(bound_index(100, 99), 0);
    }

    #[test]
    fn test_bound_index_modulus_like_usage() {
        let denom = 4;
        assert_eq!(bound_index(3, denom + 1), 3);
        assert_eq!(bound_index(4, denom + 1), 4);
        assert_eq!(bound_index(5, denom + 1), 0); // like modulus reset
    }

    #[test]
    fn test_isize2usize_positive() {
        assert_eq!(isize2usize(0), 0);
        assert_eq!(isize2usize(42), 42);
        assert_eq!(isize2usize(isize::MAX), isize::MAX as usize);
    }

    #[test]
    fn test_isize2usize_negative() {
        assert_eq!(isize2usize(-1), usize::MAX);
        assert_eq!(isize2usize(-2), usize::MAX - 1);
        assert_eq!(isize2usize(isize::MIN), (isize::MIN as usize));
    }
}
