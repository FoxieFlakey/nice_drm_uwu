use std::{ffi::OsString, io};

use drm::Device;
use thiserror::Error;

use crate::{fds::BasicGpuFd, util::retry_eintr};

pub struct DriverInfo {
    pub version: (i32, i32, i32),
    pub name: String,
    pub description: String,
    pub date: String,
}

#[derive(Error, Debug)]
pub enum GetDriverInfoError {
    #[error("Non UTF-8 driver name: {0:?}")]
    NonUTF8Name(OsString),
    #[error("Non UTF-8 driver description: {0:?}")]
    NonUTF8Description(OsString),
    #[error("Non UTF-8 driver date: {0:?}")]
    NonUTF8Date(OsString),
    #[error("Cannot get driver info")]
    RetrievingDriverInfo(#[source] io::Error),
}

impl DriverInfo {
    pub(crate) fn make<Gpu>(gpu: &Gpu) -> Result<Self, GetDriverInfoError>
    where
        Gpu: AsRef<BasicGpuFd>,
    {
        let gpu = gpu.as_ref();
        let info =
            retry_eintr(|| gpu.get_driver()).map_err(GetDriverInfoError::RetrievingDriverInfo)?;

        let name = info
            .name
            .into_string()
            .map_err(GetDriverInfoError::NonUTF8Date)?;
        let desc = info
            .desc
            .into_string()
            .map_err(GetDriverInfoError::NonUTF8Description)?;
        let date = info
            .date
            .into_string()
            .map_err(GetDriverInfoError::NonUTF8Date)?;

        Ok(Self {
            version: info.version,
            date: date,
            description: desc,
            name: name,
        })
    }
}
