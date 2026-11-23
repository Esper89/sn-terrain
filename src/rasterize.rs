use crate::{Octree, Octnode, VoxelGrid, Voxel};

/// Draw an octree to a voxel grid.
///
/// The material and distance values of the nodes in the octree are preserved exactly. Nodes with
/// any out-of-bounds children are treated as leaf nodes. The octree is not descended more than
/// five levels, and nodes that have one of their ancestors as their child will still be descended
/// as though the tree structure were valid.
pub fn rasterize(octree: &Octree, grid: &mut VoxelGrid) {
    grid.0.rasterize_node(octree, octree.root());
}

trait RasterizeNode {
    fn rasterize_node(&mut self, octree: &Octree, node: &Octnode);
}

macro_rules! impl_rasterize_node {
    ($vol:literal => $next:literal $(=> $subseq:literal)*) => {
        impl RasterizeNode for [Voxel; $vol] {
            #[inline(always)]
            fn rasterize_node(&mut self, octree: &Octree, node: &Octnode) {
                match octree.node_children(node) {
                    Some(children) => {
                        let voxels: &mut [[Voxel; $next]; 8] = bytemuck::must_cast_mut(self);
                        for (voxels, child) in voxels.iter_mut().zip(children) {
                            voxels.rasterize_node(octree, child);
                        }
                    },

                    None => {
                        let node: u32 = bytemuck::must_cast(*node);
                        self.fill(bytemuck::must_cast(node as u16));
                    },
                }
            }
        }

        impl_rasterize_node!($next $(=> $subseq)*);
    };

    ($done:literal) => { };
}
impl_rasterize_node!(32768 => 4096 => 512 => 64 => 8);

impl RasterizeNode for [Voxel; 8] {
    #[inline(always)]
    fn rasterize_node(&mut self, octree: &Octree, node: &Octnode) {
        *self = match octree.node_children(node) {
            Some(children) => {
                let children: [u32; 8] = bytemuck::must_cast(*children);
                bytemuck::must_cast(children.map(|c| c as u16))
            },

            None => {
                let node: u32 = bytemuck::must_cast(*node);
                bytemuck::must_cast([node as u16; 8])
            },
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn rasterize_empty_grid() {
        let octree = Octree::EMPTY;

        let mut grid = VoxelGrid([Voxel { mat: 1, dist: 1 }; _]);
        rasterize(octree, &mut grid);

        let expect = VoxelGrid::EMPTY;
        assert_eq!(grid, expect);
    }

    #[test]
    fn rasterize_sphere_grid() {
        let octree = Octree::new(bytemuck::cast_slice({
            include_bytes!("../tests/cases/sphere-octree.bin")
        }));

        let mut grid = VoxelGrid::EMPTY;
        rasterize(octree, &mut grid);

        let expect = const {
            VoxelGrid(bytemuck::must_cast(*include_bytes!("../tests/cases/sphere-grid.bin")))
        };
        assert_eq!(grid, expect);
    }

    #[test]
    fn rasterize_sphere_then_empty() {
        let mut grid = VoxelGrid::EMPTY;

        let octree = Octree::new(bytemuck::cast_slice({
            include_bytes!("../tests/cases/sphere-octree.bin")
        }));
        rasterize(octree, &mut grid);
        let expect = const {
            VoxelGrid(bytemuck::must_cast(*include_bytes!("../tests/cases/sphere-grid.bin")))
        };
        assert_eq!(grid, expect);

        let octree = Octree::EMPTY;
        rasterize(octree, &mut grid);
        let expect = VoxelGrid::EMPTY;
        assert_eq!(grid, expect);
    }
}
