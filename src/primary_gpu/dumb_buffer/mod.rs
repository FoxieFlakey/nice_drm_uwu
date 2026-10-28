use std::{io, num::NonZero, os::fd::AsFd, sync::Arc};

use drm::buffer::{DrmFourcc, DrmModifier};
use nix::sys::mman::{MapFlags, ProtFlags, mmap};

use crate::{
    primary_gpu::{PrimaryGpu, dumb_buffer::mapping::DumbMapping, framebuffer::Framebuffer},
    util::retry_eintr,
};

pub mod mapping;

pub struct DumbBuffer {
    pub(super) gpu: Arc<PrimaryGpu>,
    pub(super) handle: drm::buffer::Handle,
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) pitch: usize,
    pub(super) length: usize,
    pub(super) format: BufferFormat,
    pub(super) modifier: BufferModifier,
}

impl Drop for DumbBuffer {
    fn drop(&mut self) {
        drm_ffi::mode::dumbbuffer::destroy(self.gpu.fd.as_fd(), self.handle.into())
            .expect("Cannot destroy dumb buffer");
    }
}

impl DumbBuffer {
    pub fn format(&self) -> BufferFormat {
        self.format
    }

    pub fn get_pitch(&self) -> usize {
        self.pitch
    }

    pub fn get_width(&self) -> usize {
        self.width
    }

    pub fn get_height(&self) -> usize {
        self.height
    }

    pub fn get_length(&self) -> usize {
        self.length
    }

    pub fn create_framebuffer(&self) -> io::Result<Framebuffer> {
        let ret = retry_eintr(|| {
            drm_ffi::mode::add_fb2(
                self.gpu.fd.as_fd(),
                self.width as u32,
                self.height as u32,
                self.format.as_drm_fourcc() as u32,
                &[self.handle.into(), 0, 0, 0],
                &[self.pitch as u32, 0, 0, 0],
                &[0, 0, 0, 0],
                &[0, 0, 0, 0],
                0,
            )
        })?;

        Ok(Framebuffer {
            gpu: self.gpu.clone(),
            framebuffer: drm::control::from_u32(ret.fb_id).unwrap(),
        })
    }

    pub fn map<'a>(&'a self) -> io::Result<DumbMapping<'a>> {
        let info =
            drm_ffi::mode::dumbbuffer::map(self.gpu.fd.as_fd(), u32::from(self.handle), 0, 0)?;

        // Do the mapping
        // SAFETY: Everything make sense?
        let ptr = unsafe {
            mmap(
                None,
                NonZero::try_from(self.length).unwrap(),
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                MapFlags::MAP_SHARED,
                self.gpu.fd.as_fd(),
                info.offset.try_into().unwrap(),
            )?
        };

        Ok(DumbMapping {
            _owner: std::marker::PhantomData,
            len: self.length,
            ptr,
        })
    }
}

#[derive(Clone, Copy)]
pub enum BufferModifier {
    Linear,
}

impl BufferModifier {
    pub(super) fn as_drm_modifier(self) -> DrmModifier {
        match self {
            BufferModifier::Linear => DrmModifier::Linear,
        }
    }
}

// There too many forms in DrmFourcc lets opt in known ones
// for now
#[derive(Clone, Copy)]
pub enum BufferFormat {
    Argb8888,
    Xrgb8888,
}

impl BufferFormat {
    pub(super) fn as_drm_fourcc(self) -> DrmFourcc {
        match self {
            BufferFormat::Argb8888 => DrmFourcc::Argb8888,
            BufferFormat::Xrgb8888 => DrmFourcc::Xrgb8888,
        }
    }

    pub(super) fn bits_per_pixel(self) -> u32 {
        match self {
            BufferFormat::Argb8888 => 32,
            BufferFormat::Xrgb8888 => 32,
        }
    }
}
