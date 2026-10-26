use drm::Device;
use yoke::Yokeable;

use crate::{primary_gpu::PrimaryGpu, util::retry_eintr};

#[derive(Yokeable)]
pub struct MasterGuard<'a> {
    pub(super) gpu: &'a PrimaryGpu,
}

impl Drop for MasterGuard<'_> {
    fn drop(&mut self) {
        retry_eintr(|| self.gpu.fd.release_master_lock())
            .expect("Cannot release master lock on primary GPU");
    }
}
