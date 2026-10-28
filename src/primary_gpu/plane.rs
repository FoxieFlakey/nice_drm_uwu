use std::{
    io,
    os::fd::AsFd,
    sync::{Arc, Weak},
};

use drm::control::{Device, ResourceHandles, crtc, plane};
use thiserror::Error;

use crate::primary_gpu::{PrimaryGpu, crtc::Crtc, framebuffer::Framebuffer};

pub struct Plane {
    gpu: Weak<PrimaryGpu>,
    pub(super) handle: plane::Handle,
    pub(super) valid_crtcs: Vec<Weak<Crtc>>,
}

#[derive(Error, Debug)]
pub enum CreatingPlaneError {
    #[error("Cannot get plane info")]
    GetPlaneInformation(#[source] io::Error),
    #[error("Unknown CRTC handle: {0:?}")]
    UnknownCrtcHandle(crtc::Handle),
}

#[derive(Error, Debug)]
pub enum GettingPlaneInfoError {
    #[error("Cannot get plane info")]
    GetPlaneInformation(#[source] io::Error),
    #[error("Unknown CRTC handle: {0:?}")]
    UnknownCrtcHandle(crtc::Handle),
}

impl Plane {
    pub(super) fn from_raw(
        gpu: Weak<PrimaryGpu>,
        handle: plane::Handle,
        resources: &ResourceHandles,
    ) -> Result<Self, CreatingPlaneError> {
        let gpu_arc = gpu
            .upgrade()
            .expect("Attempting to make Encoder belonging to no GPU reference");
        let info = gpu_arc
            .fd
            .get_plane(handle)
            .map_err(CreatingPlaneError::GetPlaneInformation)?;

        let mut valid_crtcs = Vec::new();
        for crtc in resources.filter_crtcs(info.possible_crtcs()) {
            valid_crtcs.push(Arc::downgrade(
                gpu_arc
                    .find_crtc(crtc)
                    .ok_or(CreatingPlaneError::UnknownCrtcHandle(crtc))?,
            ));
        }

        Ok(Self {
            gpu,
            handle,
            valid_crtcs,
        })
    }

    fn set_framebuffer_impl(
        &self,
        crtc: Option<&Crtc>,
        framebuffer: Option<&Framebuffer>,
        crtc_rect: (i32, i32, u32, u32),
        src_rect: (u32, u32, u32, u32),
    ) -> io::Result<()> {
        let gpu = self
            .gpu
            .upgrade()
            .expect("Attempting to use plane when primary gpu is dead");

        drm_ffi::mode::set_plane(
            gpu.fd.as_fd(),
            u32::from(self.handle),
            crtc.map(|x| u32::from(x.handle)).unwrap_or(0),
            framebuffer.map(|x| u32::from(x.handle)).unwrap_or(0),
            0,
            crtc_rect.0,
            crtc_rect.1,
            crtc_rect.2,
            crtc_rect.3,
            src_rect.0 << 16,
            src_rect.1 << 16,
            src_rect.2 << 16,
            src_rect.3 << 16,
        )?;
        Ok(())
    }

    pub fn clear_framebuffer_basic(&self) -> io::Result<()> {
        self.set_framebuffer_impl(None, None, (0, 0, 0, 0), (0, 0, 0, 0))
    }

    pub fn set_framebuffer_basic(&self, crtc: &Crtc, framebuffer: &Framebuffer) -> io::Result<()> {
        self.set_framebuffer_impl(
            Some(crtc),
            Some(framebuffer),
            (0, 0, framebuffer.width, framebuffer.height),
            (0, 0, framebuffer.width, framebuffer.height),
        )
    }

    pub fn current_crtc(&self) -> Result<Option<Arc<Crtc>>, GettingPlaneInfoError> {
        let gpu = self
            .gpu
            .upgrade()
            .expect("Attempting to use plane when primary gpu is dead");
        let info = gpu
            .fd
            .get_plane(self.handle)
            .map_err(GettingPlaneInfoError::GetPlaneInformation)?;

        info.crtc()
            .map(|x| {
                gpu.find_crtc(x)
                    .ok_or(GettingPlaneInfoError::UnknownCrtcHandle(x))
                    .cloned()
            })
            .transpose()
    }
}
