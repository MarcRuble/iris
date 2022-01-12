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

pub struct PathIntegrator;

impl Default for PathIntegrator {
    #[allow(dead_code)]
    fn default() -> Self {
        Self
    }
}

impl Integrator for PathIntegrator {
    fn radiance(
        &self,
        scene: &Scene,
        mut ray: Ray,
        wavelength: Wavelength,
        sampler: &mut Sampler,
    ) -> SpectralSample {
        
        // keep track of total radiance contributed so far
        let mut radiance = SpectralSample::splat(0.0);

        // keep track of path throughput
        // contains bsdf values and cosine term
        // divided by bsdf pdf
        let mut throughput = SpectralSample::splat(1.0);

        // keep track of product of bsdf pdfs
        let mut path_pdfs = PdfSet::splat(1.0);

        // incrementally follow path
        for bounce in 0..MAX_DEPTH {

            // cast the ray to find intersection
            let (prim, hit) = match scene.intersection(&ray) {
                Some(ph) => ph,
                None => break,
            };

            // handle a special case:
            // at first intersection of camera ray there was no
            // previous loop that could determine the emissive radiance
            // therefore we count the emissiveness of this surface
            if bounce == 0 {
                if let Some(light) = prim.get_light(&scene.lights) {
                    radiance += self.get_mis_weight(path_pdfs) * throughput * light.evaluate(wavelength);
                }
            }

            // get BSDF of the hit primitive
            let bsdf = match prim.get_material(&scene.materials) {
                Some(bsdf) => bsdf,
                None => break,
            };

            // sample direct lighting
            // for the moment, do not use MIS, sample only light
            {
                // first sample 1 light source
                let (light_spectrum, light_prim, light_pick_factor) = scene.pick_one_light(sampler);
                let light_emission = light_spectrum.evaluate(wavelength);

                // sample a point on the light surface and spawn a ray towards it
                let shading_wo = hit.world_to_shading(-ray.d());
                let (light_pos, light_pdf) = light_prim.sample(&hit, sampler);
                let ray_to_light = Ray::spawn_to(hit.point, light_pos, hit.normal);
                let facing_forward = (light_pos - hit.point).dot(hit.normal) > 0.0;

                // check that the light has contribution and is reachable
                if light_pdf > 0.0
                    && facing_forward != hit.back_face
                    && light_pos.distance_squared(hit.point) > 0.00001
                    && scene.ray_hits_point(&ray_to_light, light_pos)
                {
                    // evaluate BSDF for the solid angle towards the light
                    let shading_wi = hit.world_to_shading(ray_to_light.d());
                    let bsdf_values = bsdf.evaluate(shading_wi, shading_wo, wavelength);
                    let bsdf_pdfs = bsdf.pdf(shading_wi, shading_wo, wavelength);
                    let cos_theta = shading_wi.cos_theta().abs();

                    // compute radiance coming from this single light source
                    let single_light_radiance = bsdf_values * cos_theta * light_emission / light_pdf;

                    // adjust for number of lights and apply throughput of the paths so far
                    radiance += self.get_mis_weight(path_pdfs) * throughput * light_pick_factor * single_light_radiance;
                }
            }

            // for the last step, no next path is needed
            if bounce == MAX_DEPTH-1 {
                break;
            }

            // sample BSDF for next path direction
            let shading_wo = hit.world_to_shading(-ray.d());
            let (bsdf_sampled_wi, bsdf_values, bsdf_pdfs) = bsdf.sample(shading_wo, wavelength, sampler);
            let cos_theta = bsdf_sampled_wi.cos_theta().abs();
            if bsdf_pdfs.hero() == 0.0 || cos_theta == 0.0 {
                break;
            }

            // update throughput and product of pdfs to account for the path extension
            throughput *= bsdf_values * cos_theta / bsdf_pdfs.hero();
            path_pdfs *= bsdf_pdfs;

            // spawn a new ray for next iteration
            let world_wi = hit.shading_to_world(bsdf_sampled_wi);
            ray = Ray::spawn(hit.point, world_wi, hit.normal);
        }

        return self.determine_radiance(radiance);
    }
}

impl PathIntegrator {
    // Returns the Multiple Importance Sampling (MIS) weight
    // for given product of path BSDF PDFs.
    fn get_mis_weight(&self, path_pdfs: PdfSet) -> PdfSet {
        #[cfg(feature = "hwss")]
        {
            // for hero wavelength sampling, we use equation (8) from Wilkie et al. (2014), L=lambda
            // ws(X,L) = ps(X,L) / sum_(k of C)(pk(X,L)) with ps(X,L) = pXs(X|L) * pLs(L).
            // We can omit the factor pLs(L) if (and only if!) the wavelengths are sampled from a uniform distribution.
            // So all pLs(L) terms in the fraction are equal and can be cancelled out because all wavelengths are sampled
            // with the same probability.
            // The other term pXs(X|L) refers to the probability of sampling the whole path X up to this point with wavelength
            // L which is simply the product of the BSDF PDFs for wavelength L (found in path_pdfs).
            // So we divide each path_pdfs entry by the sum of path_pdfs.
            let weight = path_pdfs / path_pdfs.sum();
            //assert_eq!(weight.x(), 0.25);
            //assert_eq!(weight.y(), 0.25);
            //assert_eq!(weight.z(), 0.25);
            //assert_eq!(weight.w(), 0.25);
            //assert_eq!(weight.sum(), 1.0);
            return weight;
        }
        #[cfg(not(feature = "hwss"))]
        {
            // for single wavelength sampling, only
            // the hero wavelength is considered
            return PdfSet::new(1.0, 0.0, 0.0, 0.0);
        }
    }

    // Returns the radiance to return from the integrator.
    fn determine_radiance(&self, radiance: SpectralSample) -> SpectralSample {
        #[cfg(feature = "hwss")]
        {
            //assert_eq!(radiance.x(), radiance.y());
            //assert_eq!(radiance.x(), radiance.z());
            //assert_eq!(radiance.x(), radiance.w());
            return radiance;
        }
        #[cfg(not(feature = "hwss"))]
        return SpectralSample::new(radiance.hero(), 0.0, 0.0, 0.0);
    }
}