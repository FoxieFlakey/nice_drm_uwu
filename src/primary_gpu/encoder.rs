use std::{
    collections::HashMap,
    fmt::Display,
    io,
    sync::{Arc, Weak},
};

use drm::control::{Device, ResourceHandles, crtc, encoder, property};
use parking_lot::RwLock;
use thiserror::Error;

use crate::{
    primary_gpu::{PrimaryGpu, crtc::Crtc, has_props::HasProperties},
    util::retry_eintr,
};

pub struct Encoder {
    gpu: Weak<PrimaryGpu>,
    cached_props: RwLock<HashMap<String, (property::Handle, property::Info)>>,
    pub(super) handle: encoder::Handle,
    kind: encoder::Kind,
    valid_crtcs: Vec<Arc<Crtc>>,
}

#[derive(Error, Debug)]
pub enum CreatingEncoderError {
    #[error("Cannot get encoder info")]
    GetEncoderInformation(#[source] io::Error),
    #[error("Unknown CRTC handle: {0:?}")]
    UnknownCrtcHandle(crtc::Handle),
}

#[derive(Error, Debug)]
pub enum GetCurrentCrtcError {
    #[error("Cannot get encoder info")]
    GetEncoderInformation(#[source] io::Error),
    #[error("Unknown CRTC handle: {0:?}")]
    UnknownCrtcHandle(crtc::Handle),
}

impl Display for Encoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self.kind {
            encoder::Kind::None => "None",
            encoder::Kind::DAC => "DAC",
            encoder::Kind::TMDS => "TMDS",
            encoder::Kind::LVDS => "LVDS",
            encoder::Kind::TVDAC => "TVDAC",
            encoder::Kind::Virtual => "Virtual",
            encoder::Kind::DSI => "DSI",
            encoder::Kind::DPMST => "DPMST",
            encoder::Kind::DPI => "DPI",
        };
        write!(f, "Encoder {} (at handle {})", kind, u32::from(self.handle))
    }
}

impl Encoder {
    pub(super) fn from_raw(
        gpu: Weak<PrimaryGpu>,
        handle: encoder::Handle,
        resources: &ResourceHandles,
    ) -> Result<Self, CreatingEncoderError> {
        let gpu_arc = gpu
            .upgrade()
            .expect("Attempting to make Encoder belonging to no GPU reference");
        let info = retry_eintr(|| gpu_arc.fd.get_encoder(handle))
            .map_err(CreatingEncoderError::GetEncoderInformation)?;

        let mut valid_crtcs = Vec::new();
        for handle in resources.filter_crtcs(info.possible_crtcs()) {
            valid_crtcs.push(
                gpu_arc
                    .find_crtc(handle)
                    .map(Arc::clone)
                    .ok_or(CreatingEncoderError::UnknownCrtcHandle(handle))?,
            );
        }

        Ok(Self {
            gpu,
            handle,
            valid_crtcs,
            cached_props: RwLock::new(HashMap::new()),
            kind: info.kind(),
        })
    }

    pub fn kind(&self) -> encoder::Kind {
        self.kind
    }

    pub fn valid_crtcs<'a>(&'a self) -> &'a [Arc<Crtc>] {
        &self.valid_crtcs
    }

    pub fn get_crtc(&self) -> Result<Option<Arc<Crtc>>, GetCurrentCrtcError> {
        let gpu_arc = self
            .gpu
            .upgrade()
            .expect("Attempting to make Encoder belonging to no GPU reference");
        let info = retry_eintr(|| gpu_arc.fd.get_encoder(self.handle))
            .map_err(GetCurrentCrtcError::GetEncoderInformation)?;

        info.crtc()
            .map(|x| {
                gpu_arc
                    .find_crtc(x)
                    .cloned()
                    .ok_or(GetCurrentCrtcError::UnknownCrtcHandle(x))
            })
            .transpose()
    }
}

impl HasProperties for Encoder {
    type HandleType = encoder::Handle;

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
