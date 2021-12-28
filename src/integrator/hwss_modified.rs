#[allow(unused)]
use crate::{
    bsdf::{Bsdf, FresnelBsdf, LambertianBsdf, MicrofacetBsdf, SampleableBsdf, SpecularBsdf},
    integrator::Integrator,
    math::Ray,
    math::{PdfSet, Point3, Shading, Vec3},
    sampling::Sampler,
    sampling::{self, mis},
    scene::Scene,
    shape::{Geometry, Intersection, Primitive, Shape, Sphere},
    spectrum::{upsample::UpsampleTable, ConstantSpectrum, Spectrum, UpsampledHdrSpectrum},
    spectrum::{SampleableSpectrum, SpectralSample, Wavelength},
    types::PrimIndex,
};

const MAX_DEPTH: u32 = 15;
const MIN_DEPTH: u32 = 2;

pub struct HwssModified;

impl Default for HwssModified {
    #[allow(dead_code)]
    fn default() -> Self {
        Self
    }
}

impl Integrator for HwssModified {
    fn radiance(
        &self,
        scene: &Scene,
        mut ray: Ray,
        wavelength: Wavelength,
        sampler: &mut Sampler,
    ) -> SpectralSample {
        //let mut radiance = SpectralSample::splat(0.0);
        let throughput = SpectralSample::splat(1.0);
        let path_pdfs = PdfSet::splat(1.0);

        let radiance = self.radiance_recursive(scene, ray, wavelength, sampler,
            MAX_DEPTH, throughput, path_pdfs);

        #[cfg(feature = "hwss")]
        return radiance;
        #[cfg(not(feature = "hwss"))]
        return SpectralSample::new(radiance.hero(), 0.0, 0.0, 0.0);
    }
}

impl HwssModified {
    fn radiance_recursive(
        &self,
        scene: &Scene,
        ray: Ray,
        wavelength: Wavelength,
        sampler: &mut Sampler,
        more_bounces: u32,
        throughput: SpectralSample,
        path_pdfs: PdfSet,
    ) -> SpectralSample {
        // check if we should continue
        if more_bounces == 0 {
            // no more bounces
            return SpectralSample::splat(0.0);
        }

        // initialize radiance added in this step
        let mut radiance = SpectralSample::splat(0.0);

        // check for ray intersection in scene
        let (prim, hit) = match scene.intersection(&ray) {
            Some(ph) => ph,
            None => return SpectralSample::splat(0.0),
        };

        // check for material at intersected primitive
        let bsdf = match prim.get_material(&scene.materials) {
            Some(bsdf) => bsdf,
            None => return SpectralSample::splat(0.0),
        };

        // if the primitive is emissive, count emissive term: Le(x, -w)
        if let Some(light) = prim.get_light(&scene.lights) {

            #[cfg(feature = "hwss")]
            let weight = mis::balance_heuristic_1(path_pdfs);
            #[cfg(not(feature = "hwss"))]
            let weight = 1.0;

            radiance += throughput
                * light.evaluate(wavelength)
                * weight;
        }

        // generate next ray
        let shading_wo = hit.world_to_shading(-ray.d());
        let (bsdf_sampled_wi, bsdf_values, bsdf_pdfs) =
            bsdf.sample(shading_wo, wavelength, sampler);
        let cos_theta = bsdf_sampled_wi.cos_theta().abs();
        if bsdf_pdfs.hero() == 0.0 || cos_theta == 0.0 {
            return SpectralSample::splat(0.0);
        }

        // spawn new ray
        let world_wi = hit.shading_to_world(bsdf_sampled_wi);
        let new_ray = Ray::spawn(hit.point, world_wi, hit.normal);

        // calculate recursive term and weight
        #[cfg(feature = "hwss")]
        let weight = mis::balance_heuristic_1(path_pdfs);
        #[cfg(not(feature = "hwss"))]
        let weight = 1.0;
        
        let recursive_value = self.radiance_recursive(scene, new_ray, wavelength, sampler, more_bounces-1, 
            throughput * bsdf_values * cos_theta / bsdf_pdfs.hero(), 
            path_pdfs * bsdf_pdfs);

        return radiance // Le(x,w)
            + weight * bsdf_values * recursive_value * cos_theta / bsdf_pdfs.hero();
    }
}
