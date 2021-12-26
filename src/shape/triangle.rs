use crate::{
    math::{self, Local, Point3, Ray, Vec3},
    sampling::{self, Sampler},
    shape::{Intersection, Shape},
};

const EPSILON: f32 = 0.0000001;

#[derive(Debug, Clone)]
pub struct Triangle {
    v1: Point3,
    v2: Point3,
    v3: Point3,
    center: Point3,
    normal: Vec3
}

impl Triangle {
    pub fn new(v1: Point3, v2: Point3, v3: Point3) -> Self {
        // compute center of mass
        let sum_vec = v1.to_vec() + v2.to_vec() + v3.to_vec();
        let center = (sum_vec / 3.0).to_point();

        // compute surface normal
        let normal = (v2 - v1).cross(v3 - v1).normalize();

        Self { v1, v2, v3, center, normal }
    }

    fn local_to_world(&self, vec: Vec3<Local>) -> Vec3 {
        self.center.to_vec() + vec.coerce_system()
    }

    fn surface_area(&self) -> f32 {
        0.5 * (self.v2 - self.v1).cross(self.v3 - self.v1).len()
    }
}

impl Shape for Triangle {
    fn intersect(&self, ray: &Ray) -> Option<(Intersection, f32)> {
        // get sides of triangle
        let a = self.v3 - self.v1;
        let b = self.v2 - self.v1;

        // check direction of ray with edges
        let p = ray.d().cross(a);
        let det = b.dot(p);

        if det.abs() < EPSILON {
            // ray is parallel to triangle surface
            return None;
        }

        // compute help vectors
        let t = ray.o() - self.v1;
        let q = t.cross(b);

        // get (u,v) as barycentric coordinates
        let idet = 1.0 / det;
        let u = t.dot(p) * idet;
        let v = ray.d().dot(q) * idet;

        if u < 0.0 || u > 1.0 || v < 0.0 || u+v > 1.0 {
            // ray hits triangle plane out of traingle area
            return None;
        }

        // get intersection point
        let distance = a.dot(q) * idet;

        if distance < EPSILON {
            // triangle is behind ray origin
            return None;
        }

        let hit_point = ray.point_at(distance);
        let hit_normal = self.normal;
        // TODO: properly calculate those two with derivatives:
        // https://pbr-book.org/3ed-2018/Shapes/Triangle_Meshes#fragment-Computedeltasfortrianglepartialderivatives-0
        let tangeant = Vec3::splat(0.5);//Vec3::new(0.0, 1.0, 0.0).cross(hit_normal).normalize();
        let bitangeant = Vec3::splat(0.5);//hit_normal.cross(tangeant);
        let back_face = hit_normal.dot(ray.d()) >= 0.0;

        Some((
            Intersection {
                point: hit_point,
                normal: hit_normal,
                tangeant,
                bitangeant,
                back_face
            },
            distance
        ))
    }

    fn sample(&self, hit: &Intersection, sampler: &mut Sampler) -> (Point3, f32) {
        // sample a point on the triangle
        let (b1, b2, b3) = (sampler.gen_0_1(), sampler.gen_0_1(), sampler.gen_0_1());
        let triangle_point = (b1 * self.v1.to_vec() + b2 * self.v2.to_vec() + b3 * self.v3.to_vec()).to_point();

        // offset so that the point of intersection lies on the right side of the triangle plane
        let hit_point = if hit.back_face {
            math::offset_origin(hit.point, -hit.normal)
        } else {
            math::offset_origin(hit.point, hit.normal)
        };

        // check if the triangle is reachable from intersection point
        let ray = Ray::spawn(hit_point, (triangle_point - hit_point).normalize(), hit.normal);
        if let Some((light_hit, _)) = self.intersect(&ray) {
            // ray hits surface
            let pdf = light_hit.point.distance_squared(hit_point) / (
                light_hit.normal.dot((light_hit.point - hit_point).normalize()) * self.surface_area()
            );
            return (triangle_point, pdf.max(0.001)); // prevent artifacts from tiny pdf
        } else {
            // ray misses surface
            return (triangle_point, 0.0);
        }
    }

    fn pdf(&self, hit: &Intersection, wi: Vec3) -> f32 {
        // offset so that the point of intersection lies on the right side of the triangle plane
        let hit_point = if hit.back_face {
            math::offset_origin(hit.point, -hit.normal)
        } else {
            math::offset_origin(hit.point, hit.normal)
        };

        // check if the triangle is reachable
        let ray = Ray::spawn(hit_point, wi, hit.normal);
        if let Some((light_hit, _)) = self.intersect(&ray) {
            // ray hits surface
            let pdf = light_hit.point.distance_squared(hit_point) / (
                light_hit.normal.dot((light_hit.point - hit_point).normalize()) * self.surface_area()
            );
            return pdf.max(0.001); // prevent artifacts from tiny pdf
        } else {
            // ray misses surface
            return 0.0;
        }
    }
}
