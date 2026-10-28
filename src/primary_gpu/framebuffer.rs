use std::{io, sync::Arc};

use drm::control::{Device, framebuffer};

use crate::{primary_gpu::PrimaryGpu, util::retry_eintr};

// Currently framebuffer doesnt do much than being handle
pub struct Framebuffer {
    pub(super) gpu: Arc<PrimaryGpu>,
    pub(super) framebuffer: framebuffer::Handle,
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        retry_eintr(|| self.gpu.fd.destroy_framebuffer(self.framebuffer))
            .expect("Cannot destroy framebuffer");
    }
}

impl Framebuffer {
    pub fn dirty(&self) -> io::Result<()> {
        retry_eintr(|| self.gpu.fd.dirty_framebuffer(self.framebuffer, &[]))
    }
}
