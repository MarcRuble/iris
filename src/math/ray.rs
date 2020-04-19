#![allow(dead_code)]

use super::{Point3, Vec3, World};

#[derive(Debug, Clone)]
pub struct Ray<System = World> {
    o: Point3<System>,
    d: Vec3<System>,
    t_max: f32,
}

impl<S> Ray<S> {
    pub fn new(o: Point3<S>, d: Vec3<S>) -> Self {
        Self {
            o,
            d: d.normalize(),
            t_max: std::f32::INFINITY,
        }
    }

    pub fn o(&self) -> Point3<S> {
        self.o
    }

    pub fn d(&self) -> Vec3<S> {
        self.d
    }

    pub fn new_t_max(o: Point3<S>, d: Vec3<S>, t_max: f32) -> Self {
        Self { o, d, t_max }
    }

    pub fn point_at(&self, t: f32) -> Point3<S> {
        self.o + self.d * t
    }
}
