use std::{fmt::Display, io, sync::Arc};

use drm::control::{Device, framebuffer};

use crate::{primary_gpu::PrimaryGpu, util::retry_eintr};

// Currently framebuffer doesnt do much than being handle
pub struct Framebuffer {
    pub(super) gpu: Arc<PrimaryGpu>,
    pub(super) handle: framebuffer::Handle,
    pub(super) width: u32,
    pub(super) height: u32,
}

impl Framebuffer {
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

impl Display for Framebuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Framebuffer {}", u32::from(self.handle))
    }
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        retry_eintr(|| self.gpu.fd.destroy_framebuffer(self.handle))
            .expect("Cannot destroy framebuffer");
    }
}

impl Framebuffer {
    pub fn dirty(&self) -> io::Result<()> {
        retry_eintr(|| self.gpu.fd.dirty_framebuffer(self.handle, &[]))
    }
}
