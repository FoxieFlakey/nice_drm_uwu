use std::{
    ffi::OsString,
    fs::OpenOptions,
    io,
    os::fd::{AsFd, OwnedFd},
    path::Path,
    sync::Arc,
};

use drm::Device;
use thiserror::Error;

use crate::{
    driver_info::{DriverInfo, GetDriverInfoError},
    drm_type::{FindNodeTypeError, NodeType},
    fds::BasicGpuFd,
};

pub struct Gpu {
    pub(crate) gpu: BasicGpuFd,
    kind: NodeType,
    driver: DriverInfo,
    busid: String,
}

#[derive(Error, Debug)]
pub enum OpenError {
    #[error("Unable to open GPU dev")]
    OpenGPUDev(#[source] io::Error),
    #[error("Cant determine DRM type of GPU")]
    DeterminingType(#[source] FindNodeTypeError),
    #[error("Cannot get GPU driver info")]
    GettingDriverInfo(#[source] GetDriverInfoError),
    #[error("Cannot get GPU bus ID")]
    GettingBusID(#[source] io::Error),
    #[error("GPU bus ID is not valid UTF8: {0:?}")]
    InvalidBusID(OsString),
}

impl Gpu {
    pub fn open<P: AsRef<Path> + ?Sized>(path: &P) -> Result<Arc<Self>, OpenError> {
        let fd = OwnedFd::from(
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .map_err(OpenError::OpenGPUDev)?,
        );

        let kind = NodeType::figure_out(fd.as_fd()).map_err(OpenError::DeterminingType)?;
        let gpu = BasicGpuFd(Arc::new(fd));
        let busid = gpu
            .get_bus_id()
            .map_err(OpenError::GettingBusID)?
            .into_string()
            .map_err(OpenError::InvalidBusID)?;

        Ok(Arc::new(Self {
            kind,
            driver: DriverInfo::make(&gpu).map_err(OpenError::GettingDriverInfo)?,
            gpu,
            busid,
        }))
    }

    pub fn driver_info(&self) -> &DriverInfo {
        &self.driver
    }

    pub fn get_bus_id(&self) -> &str {
        &self.busid
    }

    pub fn get_type(&self) -> NodeType {
        self.kind
    }
}
