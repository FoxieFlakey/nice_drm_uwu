use std::{mem, sync::Arc};

use anyhow::{Context, anyhow};
use drm::control::{AtomicCommitFlags, Device, atomic::AtomicModeReq, property::Value};

use crate::{
    primary_gpu::{
        PrimaryGpu, connector::Connector, crtc::Crtc, framebuffer::Framebuffer,
        has_props::HasProperties, mode::Mode, plane::Plane,
    },
    util::multi_errors::MultipleErrors,
};

pub struct AtomicReq {
    gpu: Arc<PrimaryGpu>,
    req: AtomicModeReq,
    property_blobs: Vec<u64>,
    errors: MultipleErrors,
    is_modeset: bool,
    do_pageflip: bool,
}

impl AtomicReq {
    pub(super) fn new(gpu: Arc<PrimaryGpu>) -> Self {
        Self {
            gpu,
            req: AtomicModeReq::new(),
            property_blobs: Vec::new(),
            errors: MultipleErrors::new(),
            is_modeset: false,
            do_pageflip: false,
        }
    }

    pub fn crtc_set_active<'a>(&'a mut self, crtc: &'a Crtc, val: bool) -> &'a mut Self {
        match crtc.get_property_handle("ACTIVE") {
            Ok(None) => {
                self.errors.push(anyhow!(
                    "Cannot set ACTIVE property of {crtc} to {val:?} because cant find ACTIVE attribute"
                ));
            }
            Ok(Some((handle, _))) => {
                self.req
                    .add_property(crtc.handle, handle, Value::Boolean(val));
            }
            Err(e) => {
                self.errors.push(
                    anyhow::Error::from(e)
                        .context(format!("Cannot set ACTIVE property of {crtc} to {val:?}")),
                );
            }
        }
        self
    }

    pub fn crtc_set_mode<'a>(&'a mut self, crtc: &'a Crtc, mode: Mode) -> &'a mut Self {
        self.is_modeset = true;
        match crtc.get_property_handle("MODE_ID") {
            Ok(None) => {
                self.errors.push(anyhow!(
                    "Cannot set MODE_ID property of {crtc} to mode {mode} because cant find MODE_ID attribute"
                ));
            }
            Ok(Some((handle, _))) => match self.gpu.fd.create_property_blob(&mode.inner) {
                Ok(blob) => {
                    let Value::Blob(blob) = blob else {
                        unreachable!()
                    };

                    self.req
                        .add_property(crtc.handle, handle, Value::Blob(blob));
                }
                Err(e) => {
                    self.errors.push(anyhow::Error::from(e).context(format!(
                            "Cannot set MODE_ID property of {crtc} to {mode} because unable to create property blob"
                        )));
                }
            },
            Err(e) => {
                self.errors.push(
                    anyhow::Error::from(e)
                        .context(format!("Cannot set MODE_ID property of {crtc} to {mode}")),
                );
            }
        }
        self
    }

    pub fn connector_attach_crtc<'a>(
        &'a mut self,
        connector: &'a Connector,
        crtc: Option<&'a Crtc>,
    ) -> &'a mut Self {
        match connector.get_property_handle("CRTC_ID") {
            Ok(None) => {
                self.errors.push(anyhow!(
                    "Cannot set CRTC_ID property of {connector} to {} because cant find CRTC_ID attribute", crtc.map(|x| format!("{x}")).unwrap_or_else(|| format!("unset"))
                ));
            }
            Ok(Some((handle, _))) => {
                self.req.add_property(
                    connector.handle,
                    handle,
                    Value::CRTC(crtc.map(|x| x.handle)),
                );
            }
            Err(e) => {
                self.errors.push(anyhow::Error::from(e).context(format!(
                        "Cannot set CRTC_ID property of {connector} to {}",
                        crtc.map(|x| format!("{x}"))
                            .unwrap_or_else(|| format!("unset"))
                    )));
            }
        }
        self
    }

    pub fn plane_attach_framebuffer<'a>(
        &'a mut self,
        plane: &'a Plane,
        framebuffer: Option<&'a Framebuffer>,
    ) -> &'a mut Self {
        match plane.get_property_handle("FB_ID") {
            Ok(None) => {
                self.errors.push(anyhow!(
                    "Cannot set FB_ID property of {plane} to {} because cant find FB_ID attribute",
                    framebuffer
                        .map(|x| format!("{x}"))
                        .unwrap_or_else(|| format!("unset"))
                ));
            }
            Ok(Some((handle, _))) => {
                self.req.add_property(
                    plane.handle,
                    handle,
                    Value::Framebuffer(framebuffer.map(|x| x.handle)),
                );
            }
            Err(e) => {
                self.errors.push(anyhow::Error::from(e).context(format!(
                        "Cannot set FB_ID property of {plane} to {}",
                        framebuffer.map(|x| format!("{x}"))
                            .unwrap_or_else(|| format!("unset"))
                    )));
            }
        }
        self
    }

    pub fn plane_attach_crtc<'a>(
        &'a mut self,
        plane: &'a Plane,
        crtc: Option<&'a Crtc>,
    ) -> &'a mut Self {
        match plane.get_property_handle("CRTC_ID") {
            Ok(None) => {
                self.errors.push(anyhow!(
                    "Cannot set CRTC_ID property of {plane} to {} because cant find CRTC_ID attribute",
                    crtc.map(|x| format!("{x}"))
                        .unwrap_or_else(|| format!("unset"))
                ));
            }
            Ok(Some((handle, _))) => {
                self.req
                    .add_property(plane.handle, handle, Value::CRTC(crtc.map(|x| x.handle)));
            }
            Err(e) => {
                self.errors.push(anyhow::Error::from(e).context(format!(
                        "Cannot set CRTC_ID property of {plane} to {}",
                        crtc.map(|x| format!("{x}"))
                            .unwrap_or_else(|| format!("unset"))
                    )));
            }
        }
        self
    }

    pub fn plane_set_crtc_x<'a>(&'a mut self, plane: &'a Plane, val: i32) -> &'a mut Self {
        match plane.get_property_handle("CRTC_X") {
            Ok(None) => {
                self.errors.push(anyhow!(
                    "Cannot set CRTC_X property of {plane} to {val} because cant find CRTC_X attribute",
                ));
            }
            Ok(Some((handle, _))) => {
                self.req
                    .add_property(plane.handle, handle, Value::SignedRange(val.into()));
            }
            Err(e) => {
                self.errors.push(
                    anyhow::Error::from(e)
                        .context(format!("Cannot set CRTC_X property of {plane} to {val}")),
                );
            }
        }
        self
    }

    pub fn plane_set_crtc_y<'a>(&'a mut self, plane: &'a Plane, val: i32) -> &'a mut Self {
        match plane.get_property_handle("CRTC_Y") {
            Ok(None) => {
                self.errors.push(anyhow!(
                    "Cannot set CRTC_Y property of {plane} to {val} because cant find CRTC_Y attribute",
                ));
            }
            Ok(Some((handle, _))) => {
                self.req
                    .add_property(plane.handle, handle, Value::SignedRange(val.into()));
            }
            Err(e) => {
                self.errors.push(
                    anyhow::Error::from(e)
                        .context(format!("Cannot set CRTC_Y property of {plane} to {val}")),
                );
            }
        }
        self
    }

    pub fn plane_set_crtc_w<'a>(&'a mut self, plane: &'a Plane, val: u32) -> &'a mut Self {
        match plane.get_property_handle("CRTC_W") {
            Ok(None) => {
                self.errors.push(anyhow!(
                    "Cannot set CRTC_W property of {plane} to {val} because cant find CRTC_W attribute",
                ));
            }
            Ok(Some((handle, _))) => {
                self.req
                    .add_property(plane.handle, handle, Value::UnsignedRange(val.into()));
            }
            Err(e) => {
                self.errors.push(
                    anyhow::Error::from(e)
                        .context(format!("Cannot set CRTC_W property of {plane} to {val}")),
                );
            }
        }
        self
    }

    pub fn plane_set_crtc_h<'a>(&'a mut self, plane: &'a Plane, val: u32) -> &'a mut Self {
        match plane.get_property_handle("CRTC_H") {
            Ok(None) => {
                self.errors.push(anyhow!(
                    "Cannot set CRTC_H property of {plane} to {val} because cant find CRTC_H attribute",
                ));
            }
            Ok(Some((handle, _))) => {
                self.req
                    .add_property(plane.handle, handle, Value::UnsignedRange(val.into()));
            }
            Err(e) => {
                self.errors.push(
                    anyhow::Error::from(e)
                        .context(format!("Cannot set CRTC_H property of {plane} to {val}")),
                );
            }
        }
        self
    }

    pub fn plane_set_src_x<'a>(&'a mut self, plane: &'a Plane, val: u32) -> &'a mut Self {
        match plane.get_property_handle("SRC_X") {
            Ok(None) => {
                self.errors.push(anyhow!(
                    "Cannot set SRC_X property of {plane} to {val} because cant find SRC_X attribute",
                ));
            }
            Ok(Some((handle, _))) => {
                self.req.add_property(
                    plane.handle,
                    handle,
                    Value::UnsignedRange(u64::from(val) << 16),
                );
            }
            Err(e) => {
                self.errors.push(
                    anyhow::Error::from(e)
                        .context(format!("Cannot set SRC_X property of {plane} to {val}")),
                );
            }
        }
        self
    }

    pub fn plane_set_src_y<'a>(&'a mut self, plane: &'a Plane, val: u32) -> &'a mut Self {
        match plane.get_property_handle("SRC_Y") {
            Ok(None) => {
                self.errors.push(anyhow!(
                    "Cannot set SRC_Y property of {plane} to {val} because cant find SRC_Y attribute",
                ));
            }
            Ok(Some((handle, _))) => {
                self.req.add_property(
                    plane.handle,
                    handle,
                    Value::UnsignedRange(u64::from(val) << 16),
                );
            }
            Err(e) => {
                self.errors.push(
                    anyhow::Error::from(e)
                        .context(format!("Cannot set SRC_Y property of {plane} to {val}")),
                );
            }
        }
        self
    }

    pub fn plane_set_src_w<'a>(&'a mut self, plane: &'a Plane, val: u32) -> &'a mut Self {
        match plane.get_property_handle("SRC_W") {
            Ok(None) => {
                self.errors.push(anyhow!(
                    "Cannot set SRC_W property of {plane} to {val} because cant find SRC_W attribute",
                ));
            }
            Ok(Some((handle, _))) => {
                self.req.add_property(
                    plane.handle,
                    handle,
                    Value::UnsignedRange(u64::from(val) << 16),
                );
            }
            Err(e) => {
                self.errors.push(
                    anyhow::Error::from(e)
                        .context(format!("Cannot set SRC_W property of {plane} to {val}")),
                );
            }
        }
        self
    }

    pub fn plane_set_src_h<'a>(&'a mut self, plane: &'a Plane, val: u32) -> &'a mut Self {
        match plane.get_property_handle("SRC_H") {
            Ok(None) => {
                self.errors.push(anyhow!(
                    "Cannot set SRC_H property of {plane} to {val} because cant find SRC_H attribute",
                ));
            }
            Ok(Some((handle, _))) => {
                self.req.add_property(
                    plane.handle,
                    handle,
                    Value::UnsignedRange(u64::from(val) << 16),
                );
            }
            Err(e) => {
                self.errors.push(
                    anyhow::Error::from(e)
                        .context(format!("Cannot set SRC_H property of {plane} to {val}")),
                );
            }
        }
        self
    }

    pub fn pageflip(&mut self, can_do: bool) -> &mut Self {
        self.do_pageflip = can_do;
        self
    }

    // After this the atomicreq is now fresh nothing
    // NOTE: Previous borrowed made will not be cleared
    // There no Rust way say "hey forgive all previous borrows"
    // while keeping ability to chain calls to AtomicReq
    pub fn commit(&mut self) -> anyhow::Result<()> {
        if !self.errors.is_empty() {
            return Err(mem::replace(&mut self.errors, MultipleErrors::new()))
                .context("One or more operations is failed before commit");
        }

        let mut flags = AtomicCommitFlags::empty();
        if self.is_modeset {
            flags |= AtomicCommitFlags::ALLOW_MODESET;
        }

        if self.do_pageflip {
            flags |= AtomicCommitFlags::PAGE_FLIP_EVENT;
        }

        self.gpu
            .fd
            .atomic_commit(flags, mem::replace(&mut self.req, AtomicModeReq::new()))
            .context("Cannot perform commit ioctl")?;
        Ok(())
    }
}

impl Drop for AtomicReq {
    fn drop(&mut self) {
        for blob in self.property_blobs.drain(..) {
            self.gpu
                .fd
                .destroy_property_blob(blob)
                .expect("Cannot destroy property blob");
        }
    }
}
