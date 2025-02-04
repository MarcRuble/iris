use crate::spectrum::{SampleableSpectrum, SpectralSample, Wavelength};

#[derive(Debug, Clone, Copy)]
pub struct ConstantSpectrum {
    value: f32,
}

impl ConstantSpectrum {
    #[allow(unused)]
    pub fn new(value: f32) -> Self {
        Self { value }
    }
}

impl SampleableSpectrum for ConstantSpectrum {
    fn evaluate_single(&self, _: f32) -> f32 {
        self.value
    }

    fn evaluate(&self, _: Wavelength) -> SpectralSample {
        SpectralSample::splat(self.value)
    }
}
