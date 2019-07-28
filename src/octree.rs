use hashbrown::hash_set;
use hashbrown::HashSet;

use crate::geo::{Bbox, Vec3};

const MAX_LEAF_SIZE: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Octree {
    root: Option<Node>,
    outside: HashSet<Vec3>,
    len: usize,
    rebuilt_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Node {
    Branch { children: Box<[Node; 8]>, bbox: Bbox },
    Leaf { points: HashSet<Vec3>, bbox: Bbox },
}

#[derive(Debug)]
struct OctreeIter<'o> {
    len: usize,
    stack: Vec<&'o Node>,
    current: Option<hash_set::Iter<'o, Vec3>>,
}

impl Octree {
    pub fn new() -> Self {
        Octree {
            root: None,
            outside: HashSet::with_capacity(MAX_LEAF_SIZE),
            len: 0,
            rebuilt_count: 0,
        }
    }

    pub fn with_hint(bbox: Bbox) -> Self {
        Octree {
            root: Some(Node::new(bbox, HashSet::new())),
            outside: HashSet::with_capacity(MAX_LEAF_SIZE),
            len: 0,
            rebuilt_count: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn rebuilt_count(&self) -> usize {
        self.rebuilt_count
    }

    pub fn iter(&self) -> impl Iterator<Item = &Vec3> {
        let stack = self.root.as_ref().map_or_else(Vec::new, |c| vec![c]);

        OctreeIter { stack, current: None, len: self.len }.chain(self.outside.iter())
    }

    pub fn add(&mut self, p: Vec3) {
        if !self.root.as_ref().map_or(false, |n| n.bbox().contains(p)) {
            if self.outside.insert(p) {
                self.len += 1;
            }

            if self.outside.len() > MAX_LEAF_SIZE {
                // eprintln!("warning: too many points outside bbox, rebuilding tree");

                let outside_bbox = self
                    .outside
                    .iter()
                    .fold(Bbox::new(*self.outside.iter().next().unwrap()), |b, p| b.expand(*p));

                let b = match &self.root {
                    None => outside_bbox,
                    Some(n) => n.bbox().union(&outside_bbox),
                };

                // scale bounding box to avoid rebuilding the tree in the near future
                let b = b.scale(2);

                self.root =
                    Some(Node::new(b, self.iter().chain(self.outside.iter()).cloned().collect()));
                self.outside.clear();
                self.rebuilt_count += 1;
            }

            return;
        }

        if self.root.as_mut().unwrap().add(p) {
            self.len += 1;
        }
    }

    pub fn nearest(&self, p: Vec3) -> Option<(Vec3, i64)> {
        let closest = self.root.as_ref().and_then(|n| n.nearest(p));

        let closest_outside =
            self.outside.iter().map(|pt| (*pt, pt.dist2(p))).min_by_key(|(_, d)| *d);

        match (closest, closest_outside) {
            (None, None) => None,
            (Some(n), None) | (None, Some(n)) => Some(n),
            (Some(n1), Some(n2)) => {
                if n1.1 < n2.1 {
                    Some(n1)
                } else {
                    Some(n2)
                }
            }
        }
    }
}

impl Node {
    pub fn new(bbox: Bbox, data: HashSet<Vec3>) -> Self {
        if data.len() <= MAX_LEAF_SIZE {
            return Node::Leaf { points: data, bbox };
        }

        let c = bbox.center();
        let sub_bboxes = split_bbox(&bbox, c);
        let mut sub_data = [
            HashSet::with_capacity(MAX_LEAF_SIZE),
            HashSet::with_capacity(MAX_LEAF_SIZE),
            HashSet::with_capacity(MAX_LEAF_SIZE),
            HashSet::with_capacity(MAX_LEAF_SIZE),
            HashSet::with_capacity(MAX_LEAF_SIZE),
            HashSet::with_capacity(MAX_LEAF_SIZE),
            HashSet::with_capacity(MAX_LEAF_SIZE),
            HashSet::with_capacity(MAX_LEAF_SIZE),
        ];

        for p in data {
            let bbox_id = partition_pt(p, c);
            sub_data[bbox_id].insert(p);
        }

        let child = |i: usize, sub_data: &mut [HashSet<Vec3>; 8]| {
            // swap to avoid to clone
            let mut d = HashSet::new();
            std::mem::swap(&mut d, &mut sub_data[i]);

            Node::new(sub_bboxes[i].clone(), d)
        };

        let children = Box::new([
            child(0, &mut sub_data),
            child(1, &mut sub_data),
            child(2, &mut sub_data),
            child(3, &mut sub_data),
            child(4, &mut sub_data),
            child(5, &mut sub_data),
            child(6, &mut sub_data),
            child(7, &mut sub_data),
        ]);

        Node::Branch { bbox, children }
    }

    pub fn add(&mut self, p: Vec3) -> bool {
        match self {
            Node::Leaf { points, bbox } => {
                let inserted = points.insert(p);

                if points.len() > MAX_LEAF_SIZE {
                    let mut t = HashSet::new();
                    std::mem::swap(&mut t, points);

                    *self = Node::new(bbox.clone(), t);
                }

                inserted
            }
            Node::Branch { children, bbox } => {
                let i = partition_pt(p, bbox.center());
                children[i].add(p)
            }
        }
    }

    pub fn nearest(&self, p: Vec3) -> Option<(Vec3, i64)> {
        match self {
            Node::Leaf { points, .. } => {
                points.iter().map(|pt| (*pt, pt.dist2(p))).min_by_key(|(_, d)| *d)
            }
            Node::Branch { children, bbox } => {
                let enclosing_bbox_id = partition_pt(p, bbox.center());

                let mut nearest = children[enclosing_bbox_id].nearest(p);

                for (bbox_id, child) in children.iter().enumerate() {
                    if bbox_id == enclosing_bbox_id {
                        continue;
                    }

                    match nearest {
                        None => {
                            nearest = child.nearest(p);
                        }
                        Some((_, min_dist)) => {
                            if child.bbox().dist2(p) >= min_dist {
                                continue;
                            }

                            if let Some((n, d)) = child.nearest(p) {
                                if d < min_dist {
                                    nearest = Some((n, d));
                                }
                            }
                        }
                    }
                }

                nearest
            }
        }
    }

    pub fn bbox(&self) -> &Bbox {
        match self {
            Node::Branch { bbox, .. } | Node::Leaf { bbox, .. } => bbox,
        }
    }
}

impl<'o> Iterator for OctreeIter<'o> {
    type Item = &'o Vec3;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(p) = self.current.as_mut().and_then(|c| c.next()) {
                self.len -= 1;
                break Some(p);
            }

            let n = self.stack.pop()?;

            match n {
                Node::Branch { children, .. } => {
                    self.stack.extend(children.iter());
                }
                Node::Leaf { points, .. } => {
                    self.current = Some(points.iter());
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl std::iter::FromIterator<Vec3> for Octree {
    fn from_iter<T: IntoIterator<Item = Vec3>>(iter: T) -> Self {
        let pts = iter.into_iter().collect::<HashSet<_>>();

        let bbox = {
            let mut pts = pts.iter();

            let bbox = match pts.next() {
                None => return Octree::new(),
                Some(first) => Bbox::new(*first),
            };

            pts.fold(bbox, |b, p| b.expand(*p))
        };

        let len = pts.len();
        Octree {
            outside: HashSet::with_capacity(MAX_LEAF_SIZE),
            root: Some(Node::new(bbox, pts)),
            len,
            rebuilt_count: 0,
        }
    }
}

fn split_bbox(bbox: &Bbox, c: Vec3) -> [Bbox; 8] {
    debug_assert!(bbox.contains(c));

    let u = bbox.upper();
    let l = bbox.lower();

    [
        // top
        Bbox::new(c).expand(Vec3::new(l.x, l.y, l.z)),
        Bbox::new(c).expand(Vec3::new(l.x, l.y, u.z)),
        Bbox::new(c).expand(Vec3::new(u.x, l.y, l.z)),
        Bbox::new(c).expand(Vec3::new(u.x, l.y, u.z)),
        // bottom
        Bbox::new(c).expand(Vec3::new(l.x, u.y, l.z)),
        Bbox::new(c).expand(Vec3::new(l.x, u.y, u.z)),
        Bbox::new(c).expand(Vec3::new(u.x, u.y, l.z)),
        Bbox::new(c).expand(Vec3::new(u.x, u.y, u.z)),
    ]
}

fn partition_pt(p: Vec3, c: Vec3) -> usize {
    if p.x <= c.x && p.y <= c.y && p.z <= c.z {
        0
    } else if p.x <= c.x && p.y <= c.y && p.z >= c.z {
        1
    } else if p.x >= c.x && p.y <= c.y && p.z <= c.z {
        2
    } else if p.x >= c.x && p.y <= c.y && p.z >= c.z {
        3
    } else if p.x <= c.x && p.y >= c.y && p.z <= c.z {
        4
    } else if p.x <= c.x && p.y >= c.y && p.z >= c.z {
        5
    } else if p.x >= c.x && p.y >= c.y && p.z <= c.z {
        6
    } else if p.x >= c.x && p.y >= c.y && p.z >= c.z {
        7
    } else {
        unreachable!()
    }
}

impl Default for Octree {
    fn default() -> Self {
        Octree::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn nearest_neighbor_is_the_same_as_iterating(
            pts in prop::collection::hash_set((-3000_i64..3000, -3000_i64..3000, -3000_i64..3000), 10000),
            p in (-3000_i64..3000, -3000_i64..3000, -3000_i64..3000)
        ) {
            let pts: HashSet<_> = pts.into_iter().map(|(x,y,z)| Vec3::new(x,y,z)).collect();
            let p = Vec3::new(p.0, p.1, p.2);

            let octree: Octree = pts.clone().into_iter().collect();

            let nn = octree.nearest(p).map(|(_, d)| d);
            let nn2 = pts.iter().map(|pp| pp.dist2(p)).min();
            prop_assert_eq!(nn, nn2);
        }
    }
}
