use std::{
    collections::HashMap,
    fmt::Display,
    io,
    sync::{Arc, Weak},
};

use drm::control::{Device, ResourceHandles, crtc, plane, property};
use parking_lot::RwLock;
use thiserror::Error;

use crate::primary_gpu::{PrimaryGpu, crtc::Crtc, has_props::HasProperties};

pub struct Plane {
    gpu: Weak<PrimaryGpu>,
    cached_props: RwLock<HashMap<String, (property::Handle, property::Info)>>,
    pub(super) handle: plane::Handle,
    pub(super) valid_crtcs: Vec<Weak<Crtc>>,
}

impl Display for Plane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Plane {}", u32::from(self.handle))
    }
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
            cached_props: RwLock::new(HashMap::new()),
        })
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

impl HasProperties for Plane {
    type HandleType = plane::Handle;

    fn get_primary_gpu(&self) -> Arc<PrimaryGpu> {
        self.gpu
            .upgrade()
            .expect("Attempgint to use dead primary GPU")
    }

    fn get_cache(
        &self,
    ) -> &parking_lot::RwLock<HashMap<String, (property::Handle, property::Info)>> {
        &self.cached_props
    }

    fn get_handle(&self) -> Self::HandleType {
        self.handle
    }
}
