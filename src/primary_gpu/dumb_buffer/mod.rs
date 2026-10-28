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
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) pitch: u32,
    pub(super) length: u32,
    pub(super) format: BufferFormat,
    #[expect(unused)]
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

    pub fn get_pitch(&self) -> u32 {
        self.pitch
    }

    pub fn get_width(&self) -> u32 {
        self.width
    }

    pub fn get_height(&self) -> u32 {
        self.height
    }

    pub fn get_length(&self) -> u32 {
        self.length
    }

    pub fn create_framebuffer(&self) -> io::Result<Framebuffer> {
        let ret = retry_eintr(|| {
            drm_ffi::mode::add_fb2(
                self.gpu.fd.as_fd(),
                self.width,
                self.height,
                self.format.as_drm_fourcc() as u32,
                &[self.handle.into(), 0, 0, 0],
                &[self.pitch, 0, 0, 0],
                &[0, 0, 0, 0],
                &[0, 0, 0, 0],
                0,
            )
        })?;

        Ok(Framebuffer {
            gpu: self.gpu.clone(),
            handle: drm::control::from_u32(ret.fb_id).unwrap(),
            width: self.width,
            height: self.height,
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
                NonZero::try_from(self.length as usize).unwrap(),
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                MapFlags::MAP_SHARED,
                self.gpu.fd.as_fd(),
                info.offset.try_into().unwrap(),
            )?
        };

        Ok(DumbMapping {
            _owner: std::marker::PhantomData,
            len: self.length as usize,
            ptr,
        })
    }
}

#[derive(Clone, Copy)]
pub enum BufferModifier {
    Linear,
}

impl BufferModifier {
    #[expect(unused)]
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
