use std::{
    collections::HashMap,
    io,
    ops::Range,
    os::fd::AsFd,
    path::Path,
    sync::{Arc, OnceLock},
    time::Instant,
};

use drm::{ClientCapability, Device, buffer, control::Device as ControlDevice};
use parking_lot::{Mutex, MutexGuard};
use thiserror::Error;

use crate::{
    drm_type::NodeType,
    fds::PrimaryGpuFd,
    gpu::{self, Gpu},
    primary_gpu::{
        atomic_req::AtomicReq,
        connector::Connector,
        crtc::Crtc,
        dumb_buffer::{BufferFormat, BufferModifier, DumbBuffer},
        encoder::Encoder,
        master_state::MasterGuard,
        plane::Plane,
    },
    util::{self, gemini, retry_eintr},
};

mod has_props;

pub mod atomic_req;
pub mod connector;
pub mod crtc;
pub mod dumb_buffer;
pub mod encoder;
pub mod framebuffer;
pub mod master_state;
pub mod mode;
pub mod plane;

pub struct PrimaryGpu {
    base: Arc<Gpu>,
    fd: PrimaryGpuFd,
    recv_events_lock: Mutex<()>,
    known_planes: OnceLock<Vec<Arc<Plane>>>,
    known_crtcs: OnceLock<Vec<Arc<Crtc>>>,
    known_encoders: OnceLock<Vec<Arc<Encoder>>>,
    supported_fb_width: Range<u32>,
    supported_fb_height: Range<u32>,
}

impl AsRef<Arc<Gpu>> for PrimaryGpu {
    fn as_ref(&self) -> &Arc<Gpu> {
        &self.base
    }
}

#[derive(Error, Debug)]
pub enum GetConnectorsError {
    #[error("Cannot get GPU resources")]
    GetResources(#[source] io::Error),
    #[error("Cannot get connector info {0:?}")]
    CannotMakeConnector(
        drm::control::connector::Handle,
        #[source] connector::ProbeConnectorError,
    ),
}

#[derive(Error, Debug)]
pub enum CreatingPrimaryGPUError {
    #[error("GPU dont support universal planes")]
    CantEnableUniversalPlanes(#[source] io::Error),
    #[error("Cannot retrieve planes")]
    CantGetPlanes(#[source] io::Error),
    #[error("This GPU is not primary GPU")]
    GPUIsNotPrimary,
    #[error("Cannot get GPU resources")]
    GetResourcesOnGPU(#[source] io::Error),
    #[error("Cant query plane information for {0:?}")]
    QueryPlaneInformation(
        drm::control::plane::Handle,
        #[source] plane::CreatingPlaneError,
    ),
    #[error("Cant query encoder information for {0:?}")]
    QueryEncoderInformation(
        drm::control::encoder::Handle,
        #[source] encoder::CreatingEncoderError,
    ),
}

#[derive(Error, Debug)]
#[error("Cannot lock as master")]
pub struct MasterLockError(#[source] io::Error);

#[derive(Error, Debug)]
pub enum OpenError {
    #[error("Cannot open GPU")]
    OpenGPU(#[source] gpu::OpenError),
    #[error("Cannot create primary GPU")]
    CantMakePrimaryGPU(#[source] CreatingPrimaryGPUError),
}

impl PrimaryGpu {
    pub fn open<P: AsRef<Path> + ?Sized>(path: &P) -> Result<Arc<Self>, OpenError> {
        Self::try_from_gpu(Gpu::open(path).map_err(OpenError::OpenGPU)?)
            .map_err(OpenError::CantMakePrimaryGPU)
    }

    pub fn get_basic_gpu(&self) -> &Arc<Gpu> {
        &self.base
    }

    fn find_crtc<'a>(&'a self, handle: drm::control::crtc::Handle) -> Option<&'a Arc<Crtc>> {
        let known_crtcs = self
            .known_crtcs
            .get()
            .expect("known_crtcs are not initialized yet");

        known_crtcs
            .binary_search_by_key(&u32::from(handle), |x| u32::from(x.handle))
            .ok()
            .map(|x| &known_crtcs[x])
    }

    fn find_encoder<'a>(
        &'a self,
        handle: drm::control::encoder::Handle,
    ) -> Option<&'a Arc<Encoder>> {
        let known_encoders = self
            .known_encoders
            .get()
            .expect("known_crtcs are not initialized yet");

        known_encoders
            .binary_search_by_key(&u32::from(handle), |x| u32::from(x.handle))
            .ok()
            .map(|x| &known_encoders[x])
    }

    pub fn try_from_gpu(gpu: Arc<Gpu>) -> Result<Arc<Self>, CreatingPrimaryGPUError> {
        match gpu.get_type() {
            NodeType::Primary => {
                gpu.set_client_capability(ClientCapability::UniversalPlanes, true)
                    .map_err(CreatingPrimaryGPUError::CantEnableUniversalPlanes)?;

                let fd = PrimaryGpuFd(gpu.gpu.clone());
                let resources = retry_eintr(|| fd.resource_handles())
                    .map_err(CreatingPrimaryGPUError::GetResourcesOnGPU)?;

                let this = Arc::new(Self {
                    fd,
                    base: gpu,
                    known_planes: OnceLock::new(),
                    known_crtcs: OnceLock::new(),
                    known_encoders: OnceLock::new(),
                    supported_fb_height: util::to_standard_range(resources.supported_fb_height()),
                    supported_fb_width: util::to_standard_range(resources.supported_fb_height()),
                    recv_events_lock: Mutex::new(()),
                });

                let mut crtcs = Vec::new();
                for &crtc in &resources.crtcs {
                    crtcs.push(Arc::new(Crtc::from_raw(Arc::downgrade(&this), crtc)));
                }

                crtcs.sort_by_key(|x| u32::from(x.handle));
                this.known_crtcs
                    .set(crtcs)
                    .ok()
                    .expect("Nothing should initialized the known_crtcs");

                let mut planes = Vec::new();
                let mut crtc_planes = HashMap::new();
                for plane in this
                    .fd
                    .plane_handles()
                    .map_err(CreatingPrimaryGPUError::CantGetPlanes)?
                {
                    let plane = Arc::new(
                        Plane::from_raw(Arc::downgrade(&this), plane, &resources).map_err(|e| {
                            CreatingPrimaryGPUError::QueryPlaneInformation(plane, e)
                        })?,
                    );

                    for crtc in &plane.valid_crtcs {
                        let crtc = crtc.upgrade().unwrap();
                        crtc_planes
                            .entry(crtc.handle)
                            .or_insert(Vec::new())
                            .push(plane.clone());
                    }

                    planes.push(plane);
                }

                planes.sort_by_key(|x| u32::from(x.handle));
                this.known_planes
                    .set(planes)
                    .ok()
                    .expect("Nothing should initialized the known_planes");

                // Now lets store support planes to each CRTC
                for (crtc, mut supported_planes) in crtc_planes {
                    supported_planes.sort_by_key(|x| u32::from(x.handle));
                    let crtc = this
                        .find_crtc(crtc)
                        .expect("Contain unknown CRTC when shouldnt");

                    crtc.supported_planes
                        .set(supported_planes)
                        .ok()
                        .expect("Already initialized?");
                }

                // And fill with empty vec for crtc that is has no planes?
                for crtc in this.known_crtcs.get().unwrap() {
                    // Dont care if its already set
                    let _ = crtc.supported_planes.set(Vec::new());
                }

                let mut encoders = Vec::new();
                for &encoder in &resources.encoders {
                    encoders.push(Arc::new(
                        Encoder::from_raw(Arc::downgrade(&this), encoder, &resources).map_err(
                            |e| CreatingPrimaryGPUError::QueryEncoderInformation(encoder, e),
                        )?,
                    ));
                }

                encoders.sort_by_key(|x| u32::from(x.handle));
                this.known_encoders
                    .set(encoders)
                    .ok()
                    .expect("Nothing should initialized the known_encoders");

                Ok(this)
            }
            NodeType::Render => Err(CreatingPrimaryGPUError::GPUIsNotPrimary),
        }
    }

    pub fn get_crtcs<'a>(&'a self) -> &'a [Arc<Crtc>] {
        self.known_crtcs
            .get()
            .expect("Known crtcs should not be uninitiliazed")
    }

    pub fn get_encoders<'a>(&'a self) -> &'a [Arc<Encoder>] {
        self.known_encoders
            .get()
            .expect("Known encoders should not be uninitiliazed")
    }

    pub fn get_connectors(
        self: &Arc<Self>,
        force_probe: bool,
    ) -> Result<Vec<Connector>, GetConnectorsError> {
        let resources = self
            .fd
            .resource_handles()
            .map_err(GetConnectorsError::GetResources)?;
        let mut connectors = Vec::new();
        for handle in resources.connectors {
            connectors.push(
                Connector::from_raw(self.clone(), handle, force_probe)
                    .map_err(|e| GetConnectorsError::CannotMakeConnector(handle, e))?,
            );
        }

        Ok(connectors)
    }

    pub fn lock_master<'a>(&'a self) -> Result<MasterGuard<'a>, MasterLockError> {
        retry_eintr(|| self.fd.acquire_master_lock()).map_err(MasterLockError)?;
        Ok(MasterGuard { gpu: self })
    }

    pub fn supported_fb_width(&self) -> Range<u32> {
        self.supported_fb_width.clone()
    }

    pub fn supported_fb_height(&self) -> Range<u32> {
        self.supported_fb_height.clone()
    }

    pub fn alloc_dumb_buffer(
        self: &Arc<PrimaryGpu>,
        resolution: (u32, u32),
        modifier: BufferModifier,
        format: BufferFormat,
    ) -> io::Result<DumbBuffer> {
        let info = drm_ffi::mode::dumbbuffer::create(
            self.fd.as_fd(),
            resolution.0,
            resolution.1,
            format.bits_per_pixel(),
            0,
        )?;

        Ok(DumbBuffer {
            handle: drm::control::from_u32::<buffer::Handle>(info.handle)
                .expect("create dumb buffer ioctl failed"),
            gpu: self.clone(),
            width: resolution.0.try_into().unwrap(),
            height: resolution.1.try_into().unwrap(),
            pitch: info.pitch.try_into().unwrap(),
            length: info.size.try_into().unwrap(),
            format,
            modifier,
        })
    }

    pub fn read_events<'a>(&'a self) -> io::Result<EventsIterator<'a>> {
        let guard = self.recv_events_lock.lock();
        Ok(EventsIterator {
            iter: self.fd.receive_events()?,
            owner: self,
            _guard: guard,
        })
    }

    pub fn create_atomic_req(self: &Arc<PrimaryGpu>) -> AtomicReq {
        AtomicReq::new(self.clone())
    }
}

pub struct PageFlipEvent {
    pub frame: u32,
    pub time: Instant,
    pub crtc: Arc<Crtc>,
}

pub struct VBlankSequence {
    pub crtc: Arc<Crtc>,
    pub sequence: u64,
    pub time: Instant,
}

pub enum Event {
    VBlankSequence(VBlankSequence),
    PageFlipDone(PageFlipEvent),
    Unknown(Vec<u8>),
}

pub struct EventsIterator<'a> {
    iter: drm::control::Events,
    owner: &'a PrimaryGpu,
    _guard: MutexGuard<'a, ()>,
}

impl Iterator for EventsIterator<'_> {
    type Item = Event;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next()? {
            drm::control::Event::Vblank(_) => {
                unimplemented!("Wont implement legacy vblank event")
            }
            drm::control::Event::PageFlip(page_flip_event) => {
                Some(Event::PageFlipDone(PageFlipEvent {
                    frame: page_flip_event.frame,
                    time: gemini::drm_time_to_instant(page_flip_event.duration.as_nanos() as i64),
                    crtc: self
                        .owner
                        .find_crtc(page_flip_event.crtc)
                        .expect("Unknown CRTC handle")
                        .clone(),
                }))
            }
            drm::control::Event::Unknown(data) => {
                assert!(
                    data.len() >= size_of::<drm_ffi::drm_event>(),
                    "Unknown payload is too small to fit drm_event"
                );

                let event_ptr = data.as_ptr().cast::<drm_ffi::drm_event>();
                let event = unsafe { std::ptr::read_unaligned(event_ptr) };
                match event.type_ {
                    drm_ffi::DRM_EVENT_CRTC_SEQUENCE => {
                        assert!(
                            data.len() >= size_of::<drm_ffi::drm_event_crtc_sequence>(),
                            "CRTC sequence event payload is too small to fit drm_event_crtc_sequence"
                        );

                        let event = unsafe {
                            std::ptr::read_unaligned(
                                event_ptr.cast::<drm_ffi::drm_event_crtc_sequence>(),
                            )
                        };

                        Some(Event::VBlankSequence(VBlankSequence {
                            crtc: self
                                .owner
                                .find_crtc(drm::control::from_u32(event.user_data as u32).unwrap())
                                .expect("Unknown CRTC handle")
                                .clone(),
                            sequence: event.sequence,
                            time: gemini::drm_time_to_instant(event.time_ns),
                        }))
                    }

                    _ => Some(Event::Unknown(data)),
                }
            }
        }
    }
}
