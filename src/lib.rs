use rand::Rng;

pub mod geo;
pub use geo::Vec3;
pub mod octree;

use crate::geo::Bbox;
use crate::octree::Octree;

#[derive(Debug, Clone)]
pub struct Dla {
    spawn_radius: i64,
    attraction_radius: i64,
    attraction_radius2: i64,

    cells: Octree,
    bbox: Bbox,
}

impl Dla {
    pub fn new(
        spawn_radius: u32,
        attraction_radius: u16,
        seeds: impl IntoIterator<Item = Vec3>,
    ) -> Option<Self> {
        let cells: Octree = seeds.into_iter().collect();

        let mut cells_it = cells.iter();
        let first_p = cells_it.next()?;
        let bbox = cells_it.fold(Bbox::new(*first_p), |b, p| b.expand(*p));

        Some(Dla {
            cells,
            bbox,
            spawn_radius: i64::from(spawn_radius),
            attraction_radius: i64::from(attraction_radius),
            attraction_radius2: i64::from(attraction_radius).pow(2),
        })
    }

    pub fn cells(&self) -> impl Iterator<Item = &Vec3> {
        self.cells.iter()
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn bbox(&self) -> Bbox {
        self.bbox.clone()
    }

    pub fn add<R: Rng>(&mut self, rng: &mut R) -> Vec3 {
        let spawn_bbox = self
            .bbox
            .expand(self.bbox.lower() - self.spawn_radius)
            .expand(self.bbox.upper() + self.spawn_radius);

        let respawn_cell = |rng: &mut R| {
            Vec3::new(
                rng.gen_range(spawn_bbox.lower().x..=spawn_bbox.upper().x),
                rng.gen_range(spawn_bbox.lower().y..=spawn_bbox.upper().y),
                rng.gen_range(spawn_bbox.lower().z..=spawn_bbox.upper().z),
            )
        };

        let mut cell = respawn_cell(rng);

        loop {
            match self.stuck(cell) {
                Some(n) => {
                    let d = cell - n;
                    cell = n + Vec3::new(d.x.signum(), d.y.signum(), d.z.signum());

                    self.cells.add(cell);
                    self.bbox = self.bbox.expand(cell);

                    break;
                }
                None => {
                    let mut motion = || {
                        let d = if rng.gen::<f32>() < 0.5 { -1 } else { 1 };
                        rng.gen_range(1..self.attraction_radius / 2) * d
                    };

                    let d = Vec3::new(motion(), motion(), motion());

                    cell = cell + d * self.attraction_radius;

                    if !spawn_bbox.contains(cell) {
                        cell = respawn_cell(rng);
                    }
                }
            }
        }

        cell
    }

    pub fn stuck(&self, p: Vec3) -> Option<Vec3> {
        let (n, d2) = self.cells.nearest(p)?;

        if d2 <= self.attraction_radius2 {
            Some(n)
        } else {
            None
        }
    }
}
