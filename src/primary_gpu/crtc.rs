use std::{
    collections::HashMap,
    fmt::Display,
    io,
    os::fd::{AsFd, AsRawFd},
    sync::{Arc, OnceLock, Weak},
    time::Instant,
};

use drm::control::{Device, crtc, property};
use enumflags2::BitFlags;
use nix::ioctl_readwrite;
use parking_lot::RwLock;
use thiserror::Error;

use crate::{
    primary_gpu::{PrimaryGpu, has_props::HasProperties, mode::Mode, plane::Plane},
    util::{gemini, retry_eintr},
};

pub struct Crtc {
    gpu: Weak<PrimaryGpu>,
    cached_props: RwLock<HashMap<String, (property::Handle, property::Info)>>,
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

pub struct SequenceInfo {
    pub active: bool,
    pub sequence: u64,
    pub sequence_time: Instant,
}

pub enum VBlankSequenceTarget {
    Absolute(u64),
    Relative(u64),
}

#[enumflags2::bitflags]
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum VBlankSequenceFlags {
    NextOnMiss = drm_ffi::DRM_CRTC_SEQUENCE_NEXT_ON_MISS,
}

impl Crtc {
    pub(crate) fn from_raw(gpu: Weak<PrimaryGpu>, handle: crtc::Handle) -> Self {
        Self {
            gpu,
            handle,
            supported_planes: OnceLock::new(),
            cached_props: RwLock::new(HashMap::new()),
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

    pub fn get_vblank_sequence(&self) -> io::Result<SequenceInfo> {
        let gpu_arc = self
            .gpu
            .upgrade()
            .expect("Attempting to use CRTC belonging to dead primary gpu");
        let fd = gpu_arc.fd.as_fd();
        let mut sequence_arg = drm_ffi::drm_crtc_get_sequence {
            crtc_id: u32::from(self.handle),
            sequence: 0,
            active: 0,
            sequence_ns: 0,
        };

        ioctl_readwrite!(
            crtc_get_sequence,
            drm_ffi::DRM_IOCTL_BASE,
            0x3b,
            drm_ffi::drm_crtc_get_sequence
        );

        unsafe { crtc_get_sequence(fd.as_raw_fd(), &raw mut sequence_arg) }?;

        Ok(SequenceInfo {
            active: sequence_arg.active != 0,
            sequence: sequence_arg.sequence,
            sequence_time: gemini::drm_time_to_instant(sequence_arg.sequence_ns),
        })
    }

    pub fn queue_vblank_sequence(
        &self,
        target_sequence: VBlankSequenceTarget,
        flags: BitFlags<VBlankSequenceFlags>,
    ) -> io::Result<()> {
        let gpu_arc = self
            .gpu
            .upgrade()
            .expect("Attempting to use CRTC belonging to dead primary gpu");
        let fd = gpu_arc.fd.as_fd();

        let (sequence, wait_type) = match target_sequence {
            VBlankSequenceTarget::Absolute(n) => (n, 0),
            VBlankSequenceTarget::Relative(n) => (n, drm_ffi::DRM_CRTC_SEQUENCE_RELATIVE),
        };

        let mut sequence_arg = drm_ffi::drm_crtc_queue_sequence {
            crtc_id: u32::from(self.handle),
            flags: flags.bits() | wait_type,
            sequence,
            user_data: u32::from(self.handle).into(),
        };

        ioctl_readwrite!(
            crtc_queue_sequence,
            drm_ffi::DRM_IOCTL_BASE,
            0x3c,
            drm_ffi::drm_crtc_queue_sequence
        );

        unsafe { crtc_queue_sequence(fd.as_raw_fd(), &raw mut sequence_arg) }?;
        Ok(())
    }
}

impl HasProperties for Crtc {
    type HandleType = crtc::Handle;

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
