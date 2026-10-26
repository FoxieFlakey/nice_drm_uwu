use std::{fmt::Display, io, sync::Arc};

use drm::control::{Device, connector, encoder};
use thiserror::Error;

use crate::{
    primary_gpu::{PrimaryGpu, encoder::Encoder, mode::Mode},
    util::retry_eintr,
};

pub use drm::control::connector::{Interface, State, SubPixel};

pub struct Connector {
    gpu: Arc<PrimaryGpu>,
    pub(super) handle: connector::Handle,
    interface: Interface,
    interface_id: u32,
    connection: State,
    physical_size: Option<(u32, u32)>,
    modes: Vec<Mode>,
    valid_encoders: Vec<Arc<Encoder>>,
    current_encoder: Option<Arc<Encoder>>,
    subpixel: SubPixel,
}

#[derive(Debug, Error)]
pub enum ProbeConnectorError {
    #[error("Cannot get connector information")]
    RetrievingConnectorInfo(#[source] io::Error),
    #[error("Cannot find encoder {0:?}")]
    FindingEncoder(encoder::Handle),
}

impl Display for Connector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}-{} (Connector {})",
            self.interface.as_str(),
            self.interface_id,
            u32::from(self.handle)
        )
    }
}

impl Connector {
    pub(crate) fn from_raw(
        gpu: Arc<PrimaryGpu>,
        handle: connector::Handle,
        force_probe: bool,
    ) -> Result<Self, ProbeConnectorError> {
        let (valid_encoders, current_encoder, modes, info) =
            Self::probe_impl(&gpu, handle, force_probe)?;

        Ok(Self {
            gpu,
            handle,
            interface: info.interface(),
            interface_id: info.interface_id(),
            connection: info.state(),
            physical_size: info.size(),
            modes,
            valid_encoders,
            current_encoder,
            subpixel: info.subpixel(),
        })
    }

    pub fn probe(&mut self, force_probe: bool) -> Result<(), ProbeConnectorError> {
        // Just rewrite self x3
        *self = Connector::from_raw(self.gpu.clone(), self.handle, force_probe)?;
        Ok(())
    }

    fn probe_impl(
        gpu: &Arc<PrimaryGpu>,
        handle: connector::Handle,
        force_probe: bool,
    ) -> Result<
        (
            Vec<Arc<Encoder>>,
            Option<Arc<Encoder>>,
            Vec<Mode>,
            connector::Info,
        ),
        ProbeConnectorError,
    > {
        let info = retry_eintr(|| gpu.fd.get_connector(handle, force_probe))
            .map_err(ProbeConnectorError::RetrievingConnectorInfo)?;
        let mut valid_encoders = Vec::new();
        for &encoder in info.encoders() {
            valid_encoders.push(
                gpu.find_encoder(encoder)
                    .ok_or(ProbeConnectorError::FindingEncoder(encoder))?
                    .clone(),
            );
        }

        let current_encoder = info
            .current_encoder()
            .map(|encoder| {
                gpu.find_encoder(encoder)
                    .ok_or(ProbeConnectorError::FindingEncoder(encoder))
            })
            .transpose()?
            .cloned();

        let mut modes = Vec::new();
        for &mode in info.modes() {
            modes.push(Mode::from_raw(mode));
        }

        Ok((valid_encoders, current_encoder, modes, info))
    }

    pub fn valid_encoders<'a>(&'a self) -> &'a [Arc<Encoder>] {
        &self.valid_encoders
    }

    pub fn current_encoder<'a>(&'a self) -> Option<&'a Arc<Encoder>> {
        self.current_encoder.as_ref()
    }

    pub fn subpixel(&self) -> SubPixel {
        self.subpixel
    }

    pub fn interface(&self) -> Interface {
        self.interface
    }

    pub fn interface_id(&self) -> u32 {
        self.interface_id
    }

    pub fn modes<'a>(&'a self) -> &'a [Mode] {
        &self.modes
    }

    pub fn size(&self) -> Option<(u32, u32)> {
        self.physical_size
    }

    pub fn state(&self) -> State {
        self.connection
    }
}
