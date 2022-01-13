#![allow(unused)]
#![allow(dead_code)]
use crate::{
    bsdf::{Bsdf, FresnelBsdf, LambertianBsdf, MicrofacetBsdf, SampleableBsdf, SpecularBsdf},
    math::{self, PdfSet, Point3, Ray, Shading, Vec3},
    sampling::{self, mis, Sampler},
    shape::{Geometry, Intersection, Primitive, Shape, Sphere, Triangle},
    spectrum::{
        upsample::UpsampleTable,
        ConstantSpectrum,
        SampleableSpectrum,
        SpectralSample,
        Spectrum,
        UpsampledHdrSpectrum,
        Wavelength,
    },
    types::PrimIndex,
};

use std::f32::INFINITY;

#[derive(Default)]
pub struct Scene {
    pub lights: Vec<PrimIndex<Spectrum>>,
    pub materials: Vec<PrimIndex<Bsdf>>,
    pub primitives: Vec<Primitive>,
    _env_map: Vec<UpsampledHdrSpectrum>,
}

impl Scene {
    pub fn dispersion() -> Self {
        let mut scene = Self::default();
        let upsample_table = UpsampleTable::load();

        // define color spectra
        let orange = upsample_table.get_spectrum([1.0, 0.4, 0.0]);
        let blue = upsample_table.get_spectrum([0.0, 0.1, 1.0]);
        let gray = upsample_table.get_spectrum([0.8, 0.8, 0.8]);
        let black = upsample_table.get_spectrum([0.1, 0.1, 0.1]);
        let constant = ConstantSpectrum::new(1.0);

        // add floor
        let floor_size = 100.0;
        let floor_corner1 = Point3::new(-floor_size, -1.1, -floor_size);
        let floor_corner2 = Point3::new(-floor_size, -1.1, floor_size);
        let floor_corner3 = Point3::new(floor_size, -1.1, -floor_size);
        let floor_corner4 = Point3::new(floor_size, -1.1, floor_size);

        scene.add_material(
            Triangle::new(floor_corner1, floor_corner3, floor_corner4),
            LambertianBsdf::new(gray),
        );
        scene.add_material(
            Triangle::new(floor_corner1, floor_corner4, floor_corner2),
            LambertianBsdf::new(gray),
        );

        // add light ball
        let light_pos = Point3::new(-1.5, -0.75, 2.0);
        scene.add_emissive_material(
            Sphere::new(light_pos, 0.1),
            LambertianBsdf::new(gray),
            ConstantSpectrum::new(100000.0),
        );

        // add walls with tiny opening
        let verts_box_left = Scene::get_vertices_closed_box(
            light_pos + Vec3::new(0.5, 0.0, -10.0),
            Vec3::new(0.1, 1.0, 9.99),
        );
        let mut i = 0;
        while i < verts_box_left.len() {
            scene.add_material(
                Triangle::new(
                    verts_box_left[i],
                    verts_box_left[i+1],
                    verts_box_left[i+2]
                ),
                LambertianBsdf::new(gray),
            );
            i += 3;
        }
        let verts_box_right = Scene::get_vertices_closed_box(
            light_pos + Vec3::new(0.5, 0.0, 10.0),
            Vec3::new(0.1, 1.0, 9.99),
        );
        let mut i = 0;
        while i < verts_box_right.len() {
            scene.add_material(
                Triangle::new(
                    verts_box_right[i],
                    verts_box_right[i+1],
                    verts_box_right[i+2]
                ),
                LambertianBsdf::new(gray),
            );
            i += 3;
        }

        // add prism
        let prism_corner1 = Point3::new(0.0, -1.0, 2.05);
        let prism_corner2 = Point3::new(0.0, -0.5, 2.05);
        let prism_corner3 = Point3::new(0.6, -1.0, 1.6);
        let prism_corner4 = Point3::new(0.6, -0.5, 1.6);
        let prism_corner5 = Point3::new(0.3, -1.0, 2.3);
        let prism_corner6 = Point3::new(0.3, -0.5, 2.3);

        let verts_prism: [Point3; 24] = [
            // ceiling
            prism_corner2, prism_corner4, prism_corner6,
            // floor
            prism_corner1, prism_corner5, prism_corner3,
            // walls
            prism_corner1, prism_corner3, prism_corner4,
            prism_corner1, prism_corner4, prism_corner2,
            prism_corner3, prism_corner5, prism_corner6,
            prism_corner3, prism_corner6, prism_corner4,
            prism_corner5, prism_corner1, prism_corner2,
            prism_corner5, prism_corner2, prism_corner6,
        ];
        let mut i = 0;
        while i < verts_prism.len() {
            scene.add_material(
                Triangle::new(
                    verts_prism[i],
                    verts_prism[i+1],
                    verts_prism[i+2]
                ),
                //LambertianBsdf::new(gray),
                FresnelBsdf::new(gray, gray, 1.55, 0.1)
            );
            i += 3;
        }

        // add glass box
        /*let verts_box_left = Scene::get_vertices_closed_box(
            Point3::new(0.0, -0.75, 2.0),
            Vec3::new(0.25, 0.25, 0.25),
        );
        let mut i = 0;
        while i < verts_box_left.len() {
            scene.add_material(
                Triangle::new(
                    verts_box_left[i],
                    verts_box_left[i+1],
                    verts_box_left[i+2]
                ),
                //LambertianBsdf::new(gray),
                FresnelBsdf::new(gray, gray, 1.55, 0.1)
            );
            i += 3;
        }*/

        // add glass sphere
        /*scene.add_material(
            Sphere::new(Point3::new(0.8, -0.75, 2.0), 0.25),
            FresnelBsdf::new(gray, gray, 1.55, 0.1),
        );*/

        scene
    }

    pub fn boxed_light() -> Self {
        let mut scene = Self::default();
        let upsample_table = UpsampleTable::load();

        // define color spectra
        let orange = upsample_table.get_spectrum([1.0, 0.4, 0.0]);
        let blue = upsample_table.get_spectrum([0.0, 0.1, 1.0]);
        let gray = upsample_table.get_spectrum([0.8, 0.8, 0.8]);
        let black = upsample_table.get_spectrum([0.1, 0.1, 0.1]);
        let constant = ConstantSpectrum::new(1.0);
        let mut green_light = upsample_table.get_spectrum([0.0, 204.0 / 255.0, 102.0 / 255.0]);
        green_light.set_scale(150.0);

        // build a cornell box
        let vertices = Scene::get_vertices_cornell_box();

        // add triangles
        let mut i = 0;
        while i < vertices.len() {
            scene.add_material(
                Triangle::new(
                    vertices[i],
                    vertices[i+1],
                    vertices[i+2]
                ),
                LambertianBsdf::new(gray),
            );
            i += 3;
        }

        // build a smaller box around the light
        let box_size = 0.15;
        let box_factor = 1.5;
        let box_center = Point3::new(0.0, 0.5, 1.0);
        let bfl = box_center + Vec3::new(-box_size, -box_size, -box_size);
        let bfr = box_center + Vec3::new(box_size, -box_size, -box_size);
        let bbl = box_center + Vec3::new(-box_size, -box_size, box_size);
        let bbr = box_center + Vec3::new(box_size, -box_size, box_size);
        let tfl = box_center + Vec3::new(-box_size * box_factor, box_size, -box_size * box_factor);
        let tfr = box_center + Vec3::new(box_size * box_factor, box_size, -box_size * box_factor);
        let tbl = box_center + Vec3::new(-box_size * box_factor, box_size, box_size * box_factor);
        let tbr = box_center + Vec3::new(box_size * box_factor, box_size, box_size * box_factor);

        let box_vertices: [Point3; 30] = [
            // floor
            bfl,
            bbl,
            bbr,
            bfl,
            bbr,
            bfr,
            // front
            bfl,
            bfr,
            tfr,
            bfl,
            tfr,
            tfl,
            // back
            bbr,
            bbl,
            tbr,
            bbl,
            tbl,
            tbr,
            // left
            bbl,
            bfl,
            tfl,
            bbl,
            tfl,
            tbl,
            // right
            bfr,
            bbr,
            tfr,
            bbr,
            tbr,
            tfr,
        ];

        // add triangles
        let mut j = 0;
        while j < box_vertices.len() {
            scene.add_material(
                Triangle::new(
                    box_vertices[j],
                    box_vertices[j+1],
                    box_vertices[j+2]
                ),
                LambertianBsdf::new(gray),
            );
            j += 3;
        }

        // add the light
        scene.add_emissive_material(
            Sphere::new(box_center, 0.1),
            LambertianBsdf::new(gray),
            ConstantSpectrum::new(150.0),
            //green_light
        );

        // add additional spheres
        scene.add_material(
            Sphere::new(Point3::new(-0.3, -0.85, 1.5), 0.3),
            LambertianBsdf::new(orange),
        );
        scene.add_material(
            Sphere::new(Point3::new(0.5, -0.3, 1.2), 0.2),
            LambertianBsdf::new(blue),
        );

        scene
    }

    pub fn glass_on_field() -> Self {
        let mut scene = Self::default();
        let upsample_table = UpsampleTable::load();

        // define color spectra
        let orange = upsample_table.get_spectrum([1.0, 0.4, 0.0]);
        let blue = upsample_table.get_spectrum([0.0, 0.1, 1.0]);
        let gray = upsample_table.get_spectrum([0.8, 0.8, 0.8]);
        let black = upsample_table.get_spectrum([0.1, 0.1, 0.1]);
        let constant = ConstantSpectrum::new(1.0);

        // add floor
        scene.add_material(
            Sphere::new(Point3::new(0.0, -101.0, 1.0), 100.0),
            LambertianBsdf::new(gray),
        );

        // add emissive spheres
        scene.add_emissive_material(
            Sphere::new(Point3::new(-0.5, 1.0, 2.0), 0.25),
            LambertianBsdf::new(gray),
            ConstantSpectrum::new(50.0),
        );
        scene.add_emissive_material(
            Sphere::new(Point3::new(5.0, 0.0, 7.0), 1.0),
            LambertianBsdf::new(gray),
            ConstantSpectrum::new(50.0),
        );

        // add glass spheres
        scene.add_material(
            Sphere::new(Point3::new(0.5, 0.0, 3.0), 0.5),
            FresnelBsdf::new(gray, gray, 1.55, 0.1),
        );
        scene.add_material(
            Sphere::new(Point3::new(-0.5, -0.5, 3.5), 0.25),
            FresnelBsdf::new(gray, blue, 1.55, 0.1),
        );
        scene.add_material(
            Sphere::new(Point3::new(3.5, 0.0, 6.0), 0.5),
            FresnelBsdf::new(orange, gray, 1.55, 0.1),
        );

        // add specular spheres
        /*scene.add_material(
            Sphere::new(Point3::new(0.0, 0.0, 5.0), 0.5),
            SpecularBsdf::new(gray),
        );*/

        scene
    }

    pub fn cornell_box() -> Self {
        let mut scene = Self::default();
        let upsample_table = UpsampleTable::load();

        // define color spectra
        let orange = upsample_table.get_spectrum([1.0, 0.4, 0.0]);
        let blue = upsample_table.get_spectrum([0.0, 0.1, 1.0]);
        let gray = upsample_table.get_spectrum([0.8, 0.8, 0.8]);
        let black = upsample_table.get_spectrum([0.1, 0.1, 0.1]);
        let constant = ConstantSpectrum::new(1.0);

        // build a cornell box
        let vertices = Scene::get_vertices_cornell_box();

        // add triangles
        let mut i = 0;
        while i < vertices.len() {
            if i < 6 {
                scene.add_material(
                    Triangle::new(
                        vertices[i],
                        vertices[i+1],
                        vertices[i+2]
                    ),
                    LambertianBsdf::new(orange),
                );
            } else if i < 12 {
                scene.add_material(
                    Triangle::new(
                        vertices[i],
                        vertices[i+1],
                        vertices[i+2]
                    ),
                    LambertianBsdf::new(blue),
                );
            } else {
                scene.add_material(
                    Triangle::new(
                        vertices[i],
                        vertices[i+1],
                        vertices[i+2]
                    ),
                    LambertianBsdf::new(gray),
                );
            }
            i += 3;
        }

        // add the ceiling light as 2 triangles
        /*scene.add_emissive_material(
            Triangle::new(
                Point3::new(-light_size, 0.95, 1.0 + light_size),
                Point3::new(light_size, 0.95, 1.0 + light_size),
                Point3::new(-light_size, 0.95, 1.0 - light_size),
            ),
            LambertianBsdf::new(gray),
            ConstantSpectrum::new(light_emission),
        );
        scene.add_emissive_material(
            Triangle::new(
                Point3::new(-light_size, 0.95, 1.0 - light_size),
                Point3::new(light_size, 0.95, 1.0 + light_size),
                Point3::new(light_size, 0.95, 1.0 - light_size),
            ),
            LambertianBsdf::new(gray),
            ConstantSpectrum::new(light_emission),
        );*/
        // add the ceiling light as a sphere
        let light_size = 0.1;
        let light_emission = 70.0;

        scene.add_emissive_material(
            Sphere::new(Point3::new(0.0, 0.85, 1.0), light_size),
            LambertianBsdf::new(gray),
            ConstantSpectrum::new(light_emission),
        );

        // add a sphere
        scene.add_material(
            Sphere::new(Point3::new(0.4, 0.0, 1.0), 0.25),
            LambertianBsdf::new(gray),
            //SpecularBsdf::new(ConstantSpectrum::new(2.0)),
            //FresnelBsdf::new(gray, gray, 1.55, 0.1),
        );

        scene
    }

    pub fn cornell_box_spheres() -> Self {
        let mut scene = Self::default();
        let upsample_table = UpsampleTable::load();

        // define color spectra
        let orange = upsample_table.get_spectrum([1.0, 0.4, 0.0]);
        let blue = upsample_table.get_spectrum([0.0, 0.1, 1.0]);
        let gray = upsample_table.get_spectrum([0.8, 0.8, 0.8]);
        let black = upsample_table.get_spectrum([0.1, 0.1, 0.1]);
        let constant = ConstantSpectrum::new(1.0);

        // build the box
        scene.add_material(
            Sphere::new(Point3::new(0.0, -101.0, 1.0), 100.0),
            LambertianBsdf::new(gray),
        );
        scene.add_material(
            Sphere::new(Point3::new(0.0, 101.0, 1.0), 100.0),
            LambertianBsdf::new(gray),
        );
        scene.add_material(
            Sphere::new(Point3::new(0.0, 0.0, 102.0), 100.0),
            LambertianBsdf::new(gray),
        );
        scene.add_material(
            Sphere::new(Point3::new(-101.0, 0.0, 1.0), 100.0),
            LambertianBsdf::new(orange),
        );
        scene.add_material(
            Sphere::new(Point3::new(101.0, 0.0, 1.0), 100.0),
            LambertianBsdf::new(blue),
        );

        // add the ceiling light as a sphere
        let light_size = 0.25;
        let light_emission = 120.0;

        scene.add_emissive_material(
            Sphere::new(Point3::new(0.0, 0.8, 1.0), 0.15),
            LambertianBsdf::new(constant),
            ConstantSpectrum::new(light_emission),
        );

        // add a sphere
        scene.add_material(
            Sphere::new(Point3::new(0.4, 0.0, 1.0), 0.25),
            LambertianBsdf::new(gray),
            //SpecularBsdf::new(ConstantSpectrum::new(2.0)),
            //FresnelBsdf::new(gray, gray, 1.55, 0.1),
        );

        scene
    }

    pub fn cornell_box_constant() -> Self {
        let mut scene = Self::default();
        let upsample_table = UpsampleTable::load();

        // define color spectra
        let orange = upsample_table.get_spectrum([1.0, 0.4, 0.0]);
        let blue = upsample_table.get_spectrum([0.0, 0.1, 1.0]);
        let gray = upsample_table.get_spectrum([0.8, 0.8, 0.8]);
        let black = upsample_table.get_spectrum([0.1, 0.1, 0.1]);
        let constant = ConstantSpectrum::new(1.0);

        // build the box
        // declare triangle vertices
        let vertices: [Point3; 30] = [
            // left wall
            Point3::new(-1.0, -1.0, 2.0),
            Point3::new(-1.0, 1.0, 2.0),
            Point3::new(-1.0, -1.0, 0.0),
            Point3::new(-1.0, -1.0, 0.0),
            Point3::new(-1.0, 1.0, 2.0),
            Point3::new(-1.0, 1.0, 0.0),
            // right wall
            Point3::new(1.0, -1.0, 2.0),
            Point3::new(1.0, -1.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(1.0, 1.0, 2.0),
            Point3::new(1.0, -1.0, 2.0),
            // back wall
            Point3::new(-1.0, 1.0, 2.0),
            Point3::new(-1.0, -1.0, 2.0),
            Point3::new(1.0, 1.0, 2.0),
            Point3::new(1.0, 1.0, 2.0),
            Point3::new(-1.0, -1.0, 2.0),
            Point3::new(1.0, -1.0, 2.0),
            // floor
            Point3::new(-1.0, -1.0, 0.0),
            Point3::new(1.0, -1.0, 0.0),
            Point3::new(-1.0, -1.0, 2.0),
            Point3::new(-1.0, -1.0, 2.0),
            Point3::new(1.0, -1.0, 0.0),
            Point3::new(1.0, -1.0, 2.0),
            // ceiling
            Point3::new(-1.0, 1.0, 2.0),
            Point3::new(1.0, 1.0, 2.0),
            Point3::new(-1.0, 1.0, 0.0),
            Point3::new(-1.0, 1.0, 0.0),
            Point3::new(1.0, 1.0, 2.0),
            Point3::new(1.0, 1.0, 0.0)
        ];

        // add triangles
        let mut i = 0;
        while i < vertices.len() {
            if i < 6 {
                scene.add_material(
                    Triangle::new(
                        vertices[i],
                        vertices[i+1],
                        vertices[i+2]
                    ),
                    //LambertianBsdf::new(ConstantSpectrum::new(2.0)),
                    LambertianBsdf::new(constant),
                    //SpecularBsdf::new(orange),
                    //FresnelBsdf::new(orange, gray, 1.5, 0.0),
                    //MicrofacetBsdf::new(orange, 0.5, 0.5),
                    //ConstantSpectrum::new(1.0),
                );
            } else if i < 12 {
                scene.add_material(
                    Triangle::new(
                        vertices[i],
                        vertices[i+1],
                        vertices[i+2]
                    ),
                    //LambertianBsdf::new(ConstantSpectrum::new(2.0)),
                    LambertianBsdf::new(constant),
                    //SpecularBsdf::new(blue),
                    //ConstantSpectrum::new(1.0),
                );
            } else {
                scene.add_material(
                    Triangle::new(
                        vertices[i],
                        vertices[i+1],
                        vertices[i+2]
                    ),
                    //LambertianBsdf::new(ConstantSpectrum::new(2.0)),
                    LambertianBsdf::new(constant),
                    //ConstantSpectrum::new(1.0),
                );
            }
            i += 3;
        }

        // add the ceiling light as 2 triangles
        let light_size = 0.25;
        let light_emission = 120.0;

        scene.add_emissive_material(
            Triangle::new(
                Point3::new(-light_size, 0.95, 1.0 + light_size),
                Point3::new(light_size, 0.95, 1.0 + light_size),
                Point3::new(-light_size, 0.95, 1.0 - light_size),
            ),
            LambertianBsdf::new(constant),
            ConstantSpectrum::new(light_emission),
        );
        scene.add_emissive_material(
            Triangle::new(
                Point3::new(-light_size, 0.95, 1.0 - light_size),
                Point3::new(light_size, 0.95, 1.0 + light_size),
                Point3::new(light_size, 0.95, 1.0 - light_size),
            ),
            LambertianBsdf::new(constant),
            ConstantSpectrum::new(light_emission),
        );

        // add a sphere
        scene.add_material(
            Sphere::new(Point3::new(0.4, 0.0, 1.0), 0.25),
            LambertianBsdf::new(constant)
        );

        scene
    }

    pub fn dummy() -> Self {
        let mut scene = Self::default();

        let upsample_table = UpsampleTable::load();

        // add light
        scene.add_emissive_material(
            Sphere::new(Point3::new(0.0, 0.0, 2.0), 0.5),
            LambertianBsdf::new(ConstantSpectrum::new(0.50)),
            ConstantSpectrum::new(1.0),
        );

        // floor is a very large sphere far away, ceiling too
        scene.add_material(
            Sphere::new(Point3::new(0.0, -101.0, 1.0), 100.0),
            LambertianBsdf::new(ConstantSpectrum::new(2.0)),
        );
        scene.add_material(
            Sphere::new(Point3::new(0.0, 101.0, 1.0), 100.0),
            LambertianBsdf::new(ConstantSpectrum::new(2.0)),
        );

        scene
    }

    fn add_light<G: Into<Geometry>, S: Into<Spectrum>>(&mut self, geom: G, light: S) {
        self.lights.push(PrimIndex {
            data: light.into(),
            prim_index: self.primitives.len(),
        });
        self.primitives
            .push(Primitive::new_light(geom.into(), self.lights.len() - 1));
    }

    fn add_material<G: Into<Geometry>, B: Into<Bsdf>>(&mut self, geom: G, material: B) {
        self.materials.push(PrimIndex {
            data: material.into(),
            prim_index: self.primitives.len(),
        });
        self.primitives.push(Primitive::new_material(
            geom.into(),
            self.materials.len() - 1,
        ));
    }

    fn add_emissive_material<G: Into<Geometry>, B: Into<Bsdf>, S: Into<Spectrum>>(
        &mut self,
        geom: G,
        material: B,
        light: S,
    ) {
        self.materials.push(PrimIndex {
            data: material.into(),
            prim_index: self.primitives.len(),
        });
        self.lights.push(PrimIndex {
            data: light.into(),
            prim_index: self.primitives.len(),
        });
        self.primitives.push(Primitive::new_emissive_material(
            geom.into(),
            self.materials.len() - 1,
            self.lights.len() - 1,
        ));
    }

    fn get_vertices_cornell_box() -> [Point3; 30] {
        // declare triangle vertices
        return [
            // left wall
            Point3::new(-1.0, -1.0, 2.0),
            Point3::new(-1.0, 1.0, 2.0),
            Point3::new(-1.0, -1.0, 0.0),
            Point3::new(-1.0, -1.0, 0.0),
            Point3::new(-1.0, 1.0, 2.0),
            Point3::new(-1.0, 1.0, 0.0),
            // right wall
            Point3::new(1.0, -1.0, 2.0),
            Point3::new(1.0, -1.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(1.0, 1.0, 2.0),
            Point3::new(1.0, -1.0, 2.0),
            // back wall
            Point3::new(-1.0, 1.0, 2.0),
            Point3::new(-1.0, -1.0, 2.0),
            Point3::new(1.0, 1.0, 2.0),
            Point3::new(1.0, 1.0, 2.0),
            Point3::new(-1.0, -1.0, 2.0),
            Point3::new(1.0, -1.0, 2.0),
            // floor
            Point3::new(-1.0, -1.0, 0.0),
            Point3::new(1.0, -1.0, 0.0),
            Point3::new(-1.0, -1.0, 2.0),
            Point3::new(-1.0, -1.0, 2.0),
            Point3::new(1.0, -1.0, 0.0),
            Point3::new(1.0, -1.0, 2.0),
            // ceiling
            Point3::new(-1.0, 1.0, 2.0),
            Point3::new(1.0, 1.0, 2.0),
            Point3::new(-1.0, 1.0, 0.0),
            Point3::new(-1.0, 1.0, 0.0),
            Point3::new(1.0, 1.0, 2.0),
            Point3::new(1.0, 1.0, 0.0)
        ];
    }

    fn get_vertices_closed_box(box_center: Point3, box_size: Vec3) -> [Point3; 36] {
        // build a smaller box around the light
        let bfl = box_center + Vec3::new(-box_size.x(), -box_size.y(), -box_size.z());
        let bfr = box_center + Vec3::new(box_size.x(), -box_size.y(), -box_size.z());
        let bbl = box_center + Vec3::new(-box_size.x(), -box_size.y(), box_size.z());
        let bbr = box_center + Vec3::new(box_size.x(), -box_size.y(), box_size.z());
        let tfl = box_center + Vec3::new(-box_size.x(), box_size.y(), -box_size.z());
        let tfr = box_center + Vec3::new(box_size.x(), box_size.y(), -box_size.z());
        let tbl = box_center + Vec3::new(-box_size.x(), box_size.y(), box_size.z());
        let tbr = box_center + Vec3::new(box_size.x(), box_size.y(), box_size.z());

        return [
            // floor
            bfl,
            bbl,
            bbr,
            bfl,
            bbr,
            bfr,
            // ceiling
            tfl,
            tbr,
            tbl,
            tfl,
            tfr,
            tbr,
            // front
            bfl,
            bfr,
            tfr,
            bfl,
            tfr,
            tfl,
            // back
            bbr,
            bbl,
            tbr,
            bbl,
            tbl,
            tbr,
            // left
            bbl,
            bfl,
            tfl,
            bbl,
            tfl,
            tbl,
            // right
            bfr,
            bbr,
            tfr,
            bbr,
            tbr,
            tfr,
        ];
    }

    pub fn background_emission(&self, ray: &Ray, _wavelength: Wavelength) -> SpectralSample {
        SpectralSample::splat(0.0)
    }

    pub fn intersection(&self, ray: &Ray) -> Option<(&Primitive, Intersection)> {
        let mut closest_t = INFINITY;
        let mut closest_prim_hit = None;

        // Note: for some reason, the equivalent code with iterators is *much* slower
        for prim in &self.primitives {
            match prim.intersect(ray) {
                Some((hit, t)) if t < closest_t && t > 0.0 => {
                    closest_t = t;
                    closest_prim_hit = Some((prim, hit));
                }
                _ => continue,
            }
        }

        closest_prim_hit
    }

    pub fn ray_hits_point(&self, ray: &Ray, pos: Point3) -> bool {
        let mut closest_t = INFINITY;

        for prim in &self.primitives {
            match prim.intersect(ray) {
                Some((_, t)) if t < closest_t && t > 0.0 => {
                    closest_t = t;
                }
                _ => continue,
            }
        }

        // TODO: Is this good enough?
        let target_t = (pos - ray.o()).len() / ray.d().len();
        closest_t > target_t - math::RAY_EPSILON
    }

    pub fn ray_hits_object(&self, ray: &Ray, light: &Primitive) -> bool {
        let mut closest_t = INFINITY;
        let mut closest_hit_is_obj = false;

        for prim in &self.primitives {
            match prim.intersect(ray) {
                Some((_, t)) if t < closest_t && t > 0.0 => {
                    closest_t = t;
                    closest_hit_is_obj = std::ptr::eq(prim, light);
                }
                _ => continue,
            }
        }

        closest_hit_is_obj
    }

    pub fn pick_one_light(&self, sampler: &mut Sampler) -> (&Spectrum, &Primitive, f32) {
        let light_idx = sampler.gen_array_index(self.lights.len());
        let light = &self.lights[light_idx];
        (&light.data, &self.primitives[light.prim_index], self.lights.len() as f32)
    }
}
