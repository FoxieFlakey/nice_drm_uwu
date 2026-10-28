use std::{
    io,
    ops::{Bound, Range, RangeBounds},
};

use nix::errno::Errno;

mod private {
    pub trait Sealed {}
}

pub trait IsEIntr: private::Sealed {
    fn is_eintr(&self) -> bool;
}

impl private::Sealed for io::Error {}
impl IsEIntr for io::Error {
    fn is_eintr(&self) -> bool {
        matches!(self.kind(), io::ErrorKind::Interrupted)
    }
}

impl private::Sealed for Errno {}
impl IsEIntr for Errno {
    fn is_eintr(&self) -> bool {
        matches!(self, Errno::EINTR)
    }
}

pub fn retry_eintr<T, E, F>(mut func: F) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
    E: IsEIntr,
{
    loop {
        match func() {
            Ok(x) => return Ok(x),
            Err(e) => {
                if e.is_eintr() {
                    continue;
                } else {
                    return Err(e);
                }
            }
        }
    }
}

// Gemini generated
pub fn to_standard_range<R: RangeBounds<u32>>(bounds: R) -> Range<u32> {
    let start = match bounds.start_bound() {
        Bound::Included(&n) => n,
        Bound::Excluded(&n) => n.saturating_add(1),
        Bound::Unbounded => 0, // Fallback default for u32 start
    };

    let end = match bounds.end_bound() {
        Bound::Included(&n) => n.checked_add(1).expect("Range end overflowed u32::MAX"),
        Bound::Excluded(&n) => n,
        Bound::Unbounded => u32::MAX, // Fallback default for u32 end
    };

    start..end
}
