#![allow(dead_code)]
#![allow(unused)]
use crate::{
    bsdf::SampleableBsdf,
    math::{self, PdfSet, Shading, Vec3},
    sampling::{self, Sampler},
    spectrum::{SampleableSpectrum, SpectralSample, Spectrum, Wavelength},
};

use std::f32::consts::PI;

#[derive(Debug, Clone)]
pub struct SpecularBsdf {
    reflected_color: Spectrum,
}

impl SpecularBsdf {
    pub fn new<S: Into<Spectrum>>(s: S) -> Self {
        Self {
            reflected_color: s.into(),
        }
    }
}

impl SampleableBsdf for SpecularBsdf {
    fn evaluate(
        &self,
        wi: Vec3<Shading>,
        wo: Vec3<Shading>,
        hero_wavelength: Wavelength,
    ) -> SpectralSample {
        // only perfect specular
        SpectralSample::splat(0.0)
    }

    fn pdf(&self, wi: Vec3<Shading>, wo: Vec3<Shading>, hero_wavelength: Wavelength) -> PdfSet {
        // only perfect specular
        PdfSet::splat(0.0)
    }

    fn sample(
        &self,
        wo: Vec3<Shading>,
        hero_wavelength: Wavelength,
        sampler: &mut Sampler,
    ) -> (Vec3<Shading>, SpectralSample, PdfSet) {
        let wi = Vec3::new(-wo.x(), -wo.y(), wo.z());
        //let wi = Vec3::new(0.0, 0.0, 1.0);
        let fresnel = 1.0;//TODO
        let bsdf = fresnel * self.reflected_color.evaluate(hero_wavelength)
            / wi.cos_theta().abs();
        //let bsdf = SpectralSample::splat(1.0);
        /*println!("--- specular sample ---");
        println!("wo: {}, {}, {}", wo.x(), wo.y(), wo.z());
        println!("wi: {}, {}, {}", wi.x(), wi.y(), wi.z());*/
        (wi, bsdf, PdfSet::splat(1.0))
    }

    fn is_specular(&self) -> bool {
        true
    }
}
