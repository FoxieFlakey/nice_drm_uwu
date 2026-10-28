use std::{
    fmt::Display,
    io,
    sync::{Arc, OnceLock, Weak},
};

use drm::control::{Device, crtc};
use thiserror::Error;

use crate::{
    primary_gpu::{PrimaryGpu, mode::Mode, plane::Plane},
    util::retry_eintr,
};

pub struct Crtc {
    gpu: Weak<PrimaryGpu>,
    pub(super) handle: crtc::Handle,
    pub(super) supported_planes: OnceLock<Vec<Arc<Plane>>>,
}

#[derive(Error, Debug)]
#[error("Cannot read CRTC information")]
pub struct CantGetCrtcInfo(#[source] io::Error);

impl Display for Crtc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CRTC {}", u32::from(self.handle))
    }
}

impl Crtc {
    pub(crate) fn from_raw(gpu: Weak<PrimaryGpu>, handle: crtc::Handle) -> Self {
        Self {
            gpu,
            handle,
            supported_planes: OnceLock::new(),
        }
    }

    pub fn scan_position(&self) -> Result<(u32, u32), CantGetCrtcInfo> {
        retry_eintr(|| {
            self.gpu
                .upgrade()
                .expect("Using CRTC when there no reference to GPU")
                .fd
                .get_crtc(self.handle)
        })
        .map(|x| x.position())
        .map_err(CantGetCrtcInfo)
    }

    pub fn gamma_lut_size(&self) -> Result<usize, CantGetCrtcInfo> {
        retry_eintr(|| self.gpu
            .upgrade()
            .expect("Using CRTC when there no reference to GPU")
            .fd
            .get_crtc(self.handle))
            .map(|x| {
                x.gamma_length()
                    .try_into()
                    .expect("Yo Linux runs in 16-bit? 32-bit is what gamma_length returns and 32-bit is lowest bit Linux runs")
            })
            .map_err(CantGetCrtcInfo)
    }

    pub fn mode(&self) -> Result<Option<Mode>, CantGetCrtcInfo> {
        retry_eintr(|| {
            self.gpu
                .upgrade()
                .expect("Using CRTC when there no reference to GPU")
                .fd
                .get_crtc(self.handle)
        })
        .map(|x| x.mode().map(Mode::from_raw))
        .map_err(CantGetCrtcInfo)
    }

    pub fn get_planes(&self) -> &[Arc<Plane>] {
        self.supported_planes
            .get()
            .expect("Must be initialized already")
    }
}
