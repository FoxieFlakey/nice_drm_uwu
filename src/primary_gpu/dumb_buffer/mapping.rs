use std::{ffi::c_void, marker::PhantomData, mem, ptr::NonNull, slice};

use nix::sys::mman::munmap;

use crate::{primary_gpu::dumb_buffer::DumbBuffer, util::retry_eintr};

pub struct DumbMapping<'a> {
    pub(super) _owner: PhantomData<&'a DumbBuffer>,
    pub(super) len: usize,
    pub(super) ptr: NonNull<c_void>,
}

impl Drop for DumbMapping<'_> {
    fn drop(&mut self) {
        // Nobody acesses the 'ptr' anymore
        retry_eintr(|| unsafe { munmap(self.ptr, self.len) })
            .expect("Cannot unmap dumb buffer mapping");
    }
}

impl DumbMapping<'_> {
    // Dumb buffer can be destroyed but the mappping remains alive
    // this is present for some "edge case" where its unwieldly to
    // use lifetimes. Kernel won't unmaps even if buffer destroyed
    pub fn forget_lifetime(self) -> DumbMapping<'static> {
        let ret = DumbMapping {
            _owner: PhantomData,
            len: self.len,
            ptr: self.ptr,
        };
        mem::forget(self);
        ret
    }

    // # Safety
    // Caller has to ensure there no other active mutable
    // access or concurrent usage including external processes
    // sharing same buffer
    pub unsafe fn get_mut<'a>(&'a mut self) -> &'a mut [u8] {
        // SAFETY: Caller make sure there no other shared/mutable
        // access including scope of other processes
        unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr().cast(), self.len) }
    }

    // # Safety
    // Caller has to ensure there no other active mutable
    // access or concurrent usage including external processes
    // sharing same buffer
    pub unsafe fn get<'a>(&'a self) -> &'a [u8] {
        // SAFETY: Caller make sure there no other mutable
        // access including scope of other processes
        unsafe { slice::from_raw_parts(self.ptr.as_ptr().cast(), self.len) }
    }
}
