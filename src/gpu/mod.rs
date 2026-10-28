use std::{
    ffi::OsString,
    fs::OpenOptions,
    io,
    os::fd::{AsFd, BorrowedFd, OwnedFd},
    path::Path,
    sync::Arc,
};

pub use drm::ClientCapability;

use drm::{Device, DriverCapability};
use thiserror::Error;

use crate::{
    driver_info::{DriverInfo, GetDriverInfoError},
    drm_type::{FindNodeTypeError, NodeType},
    fds::BasicGpuFd,
    util::retry_eintr,
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
    #[error("Prime handle import/export is required but its not present")]
    PrimeSupportNotPresent,
    #[error("Cant query prime handle support")]
    QueryPrimeSupport(#[source] io::Error),
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

        // Pretty specific set of supports, dont care about older DRM drivers
        let this = Self {
            kind,
            driver: DriverInfo::make(&gpu).map_err(OpenError::GettingDriverInfo)?,
            gpu,
            busid,
        };

        if !this
            .is_prime_handle_supported()
            .map_err(OpenError::QueryPrimeSupport)?
        {
            return Err(OpenError::PrimeSupportNotPresent);
        }

        Ok(Arc::new(this))
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

    pub fn set_client_capability(
        &self,
        capability: ClientCapability,
        enable: bool,
    ) -> io::Result<()> {
        retry_eintr(|| self.gpu.set_client_capability(capability, enable))
    }

    fn get_driver_capability(&self, capability: DriverCapability) -> io::Result<u64> {
        retry_eintr(|| self.gpu.get_driver_capability(capability))
    }

    pub fn is_atomic_page_flip_supported(&self) -> io::Result<bool> {
        Ok(self.get_driver_capability(DriverCapability::AtomicASyncPageFlip)? != 0)
    }

    pub fn is_dumb_buffer_supported(&self) -> io::Result<bool> {
        Ok(self.get_driver_capability(DriverCapability::DumbBuffer)? != 0)
    }

    pub fn is_timeline_syncobj_supported(&self) -> io::Result<bool> {
        Ok(self.get_driver_capability(DriverCapability::TimelineSyncObj)? != 0)
    }

    pub fn is_syncobj_supported(&self) -> io::Result<bool> {
        Ok(self.get_driver_capability(DriverCapability::SyncObj)? != 0)
    }

    pub fn is_monotonic_timestamp_supported(&self) -> io::Result<bool> {
        Ok(self.get_driver_capability(DriverCapability::MonotonicTimestamp)? != 0)
    }

    // NOTE: This would required entire import + export enabled at once
    pub fn is_prime_handle_supported(&self) -> io::Result<bool> {
        let ret = self.get_driver_capability(DriverCapability::Prime)?;
        Ok(ret & u64::from(drm_ffi::DRM_PRIME_CAP_EXPORT | drm_ffi::DRM_PRIME_CAP_IMPORT) != 0)
    }

    pub fn is_add_fb2_modifier_supported(&self) -> io::Result<bool> {
        Ok(self.get_driver_capability(DriverCapability::AddFB2Modifiers)? != 0)
    }

    pub fn get_fd<'a>(&'a self) -> BorrowedFd<'a> {
        self.gpu.as_fd()
    }
}
