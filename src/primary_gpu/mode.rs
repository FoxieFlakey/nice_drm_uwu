use std::fmt::Display;

pub use drm::control::ModeFlags;

use drm::control::ModeTypeFlags;

#[derive(Clone)]
pub struct Mode {
    inner: drm::control::Mode,
    mode_type: ModeType,
    flags: ModeFlags,
}

#[derive(Clone)]
pub enum ModeSource {
    Driver,
    User,
}

#[derive(Clone)]
pub struct ModeType {
    pub is_preferred: bool,
    pub defined_by: ModeSource,
}

impl Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}x{} @ {:.2} Hz",
            self.resolution().0,
            self.resolution().1,
            self.vrefresh_f64()
        )
    }
}

impl Mode {
    pub(super) fn from_raw(raw: drm::control::Mode) -> Self {
        let supported = ModeTypeFlags::PREFERRED | ModeTypeFlags::USERDEF | ModeTypeFlags::DRIVER;
        let mode_type = raw.mode_type().intersection(supported);

        let defined_by = if mode_type.contains(ModeTypeFlags::USERDEF) {
            ModeSource::User
        } else if mode_type.contains(ModeTypeFlags::DRIVER) {
            ModeSource::Driver
        } else {
            panic!("Source of this mode cant be determine whether from user or driver")
        };

        Self {
            flags: raw.flags(),
            mode_type: ModeType {
                is_preferred: mode_type.contains(ModeTypeFlags::PREFERRED),
                defined_by,
            },
            inner: raw,
        }
    }

    pub fn clock(&self) -> u32 {
        self.inner.clock()
    }

    // Return width, height
    pub fn resolution(&self) -> (u16, u16) {
        self.inner.size()
    }

    // Returns start, end, total
    pub fn hsync(&self) -> (u16, u16, u16) {
        self.inner.hsync()
    }

    // Returns start, end, total
    pub fn vsync(&self) -> (u16, u16, u16) {
        self.inner.vsync()
    }

    pub fn hskew(&self) -> u16 {
        self.inner.hskew()
    }

    // vrefresh_f32 but decimals are truncated
    pub fn vrefresh(&self) -> u32 {
        self.inner.vrefresh() as u32
    }

    pub fn vscan(&self) -> u16 {
        self.inner.vscan()
    }

    // Made by Gemini
    pub fn vrefresh_f64(&self) -> f64 {
        let htotal = self.hsync().2;
        let vtotal = self.vsync().2;

        if htotal == 0 || vtotal == 0 {
            return 0.0;
        }

        let htotal = f64::from(htotal);
        let vtotal = f64::from(vtotal);

        // Convert pixel clock from kHz to Hz
        let clock_hz = f64::from(self.clock()) * 1000.0;
        let total_pixels = htotal * vtotal;
        let mut num = 1.0;
        let mut den = 1.0;

        // Interlace mode splits frames into fields, doubling the perceived
        if self.flags.contains(ModeFlags::INTERLACE) {
            num *= 2.0;
        }

        // Double scan doubles the number of lines per frame
        if self.flags.contains(ModeFlags::DBLSCAN) {
            den *= 2.0;
        }

        // Vscan multiplies the scanlines per vertical line
        den *= f64::from(self.vscan()).max(1.0);

        // Calculate precision decimal refresh rate
        return (clock_hz * num) / (total_pixels * den);
    }

    pub fn mode_type(&self) -> &ModeType {
        &self.mode_type
    }
}
