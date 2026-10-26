use std::{
    os::fd::{AsFd, OwnedFd},
    sync::Arc,
};

#[derive(Clone)]
pub struct BasicGpuFd(pub Arc<OwnedFd>);

impl AsRef<BasicGpuFd> for BasicGpuFd {
    fn as_ref(&self) -> &BasicGpuFd {
        self
    }
}

impl AsFd for BasicGpuFd {
    fn as_fd(&self) -> std::os::unix::prelude::BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl drm::Device for BasicGpuFd {}

#[derive(Clone)]
pub struct PrimaryGpuFd(pub BasicGpuFd);

impl AsRef<BasicGpuFd> for PrimaryGpuFd {
    fn as_ref(&self) -> &BasicGpuFd {
        &self.0
    }
}

impl AsFd for PrimaryGpuFd {
    fn as_fd(&self) -> std::os::unix::prelude::BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl drm::Device for PrimaryGpuFd {}
impl drm::control::Device for PrimaryGpuFd {}
