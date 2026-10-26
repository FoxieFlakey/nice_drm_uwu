use std::{fmt::Display, io, os::fd::AsFd};

use nix::sys::stat::{fstat, major, minor};
use procfs::Current;
use thiserror::Error;

#[derive(Clone, Copy)]
pub enum NodeType {
    Primary,
    Render,
}

impl Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeType::Primary => write!(f, "primary"),
            NodeType::Render => write!(f, "render"),
        }
    }
}

#[derive(Error, Debug)]
pub enum FindNodeTypeError {
    #[error("Unable to stat GPU")]
    StatGPUDev(#[source] io::Error),
    #[error("Cannot open /proc/devices")]
    OpenProcDevices(#[source] procfs::ProcError),
    #[error("Cannot find DRM major in/proc/devices")]
    SearchingDRMMajor,
    #[error("Device is not DRM (major: {0})")]
    NotADRMDevice(u64),
}

impl NodeType {
    pub(super) fn figure_out<Fd>(fd: Fd) -> Result<NodeType, FindNodeTypeError>
    where
        Fd: AsFd,
    {
        let ret = fstat(fd.as_fd()).map_err(|e| FindNodeTypeError::StatGPUDev(e.into()))?;
        let major = major(ret.st_rdev);
        let minor = minor(ret.st_rdev);

        let drm_major = procfs::Devices::current()
            .map_err(FindNodeTypeError::OpenProcDevices)?
            .char_devices
            .iter()
            .find(|x| x.name == "drm")
            .map(|x| u64::from(x.major))
            .ok_or(FindNodeTypeError::SearchingDRMMajor)?;

        if major != drm_major {
            return Err(FindNodeTypeError::NotADRMDevice(major));
        }

        // Source for minor: //https://github.com/torvalds/linux/blob/390febb5805d3bb830402679f277f1d5f309e6bc/drivers/gpu/drm/drm_drv.c#L131
        match minor {
            0..64 => Ok(NodeType::Primary),
            64..128 => unimplemented!("this dont handle legacy control type purposely -w-"),
            128..192 => Ok(NodeType::Render),
            192.. => unimplemented!("Extended minor will not be implemented"),
        }
    }
}
