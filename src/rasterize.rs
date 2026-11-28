use crate::{
    utils::{simd::*, Cpu, CpuLevel, CpuDispatch, dispatch_cpu},
    Octree, Octnode, VoxelGrid, Voxel,
};

#[inline(always)]
pub(crate) fn rasterize_method_impl(octree: &Octree) -> VoxelGrid {
    struct Rasterize<'r>(&'r Octree, &'r mut VoxelGrid);
    impl CpuDispatch for Rasterize<'_> {
        type Output = ();

        #[inline(always)]
        fn dispatch(self, cpu: impl Cpu) {
            let Rasterize(octree, grid) = self;
            rasterize(cpu, octree, grid);
        }
    }

    let mut grid = VoxelGrid::EMPTY;
    dispatch_cpu(const { CpuLevel::baseline() }, Rasterize(octree, &mut grid));
    grid
}

// TODO: relax initialization requirement for voxel grid

/// Draw an octree to a voxel grid.
///
/// If the signed distance values of the nodes in the octree are invalid, they are clamped to valid
/// ones. This includes removing the signed distance entirely from any node with no material index.
///
/// Nodes are descended as specified in [`node_children`](Octree::node_children); in particular, a
/// node without a material index or without a signed distance is always considered to be a leaf
/// node. The octree is not descended more than five levels, and nodes that have one of their
/// ancestors as their child will still be descended as though the tree structure were valid.
///
/// This function is fastest when used on octrees that are in depth-first order.
///
/// The voxel grid can be built back into an octree with [`derasterize`](crate::derasterize).
///
/// This function is for advanced users and users with performance concerns. For an easy-to-use
/// method, see [`Octree::rasterize`].
pub fn rasterize(cpu: impl Cpu, octree: &Octree, grid: &mut VoxelGrid) {
    grid.0.rasterize_node(cpu.simd(), octree, octree.root());
}

trait RasterizeNode {
    fn rasterize_node(&mut self, simd: impl SimdExt, octree: &Octree, node: &Octnode);
}

macro_rules! impl_rasterize_node {
    ($vol:literal => $next:literal $(=> $subseq:literal)*) => {
        impl RasterizeNode for [Voxel; $vol] {
            #[simd]
            #[inline(always)]
            fn rasterize_node(&mut self, simd: impl SimdExt, octree: &Octree, node: &Octnode) {
                match octree.node_children(node) {
                    Some(children) => {
                        let voxels: &mut [[Voxel; $next]; 8] = bytemuck::must_cast_mut(self);
                        for (voxels, child) in voxels.iter_mut().zip(children) {
                            voxels.rasterize_node(simd, octree, child);
                        }
                    },

                    None => {
                        let mut node = *node;
                        if node.mat == 0 {
                            node.dist = 0;
                        } else {
                            node.dist = node.dist.clamp(0, 252);
                        }

                        let node: u32 = bytemuck::must_cast(node);
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
    #[simd]
    #[inline(always)]
    fn rasterize_node(&mut self, simd: impl SimdExt, octree: &Octree, node: &Octnode) {
        *self = match octree.node_children(node) {
            Some(children) => {
                let mut children = *children;
                for child in &mut children {
                    if child.mat == 0 {
                        child.dist = 0;
                    } else {
                        child.dist = child.dist.clamp(0, 252);
                    }
                }

                let children: [u32; 8] = bytemuck::must_cast(children);
                bytemuck::must_cast(children.map(#[inline(always)] |c| c as u16))
            },

            None => {
                let mut node = *node;
                if node.mat == 0 {
                    node.dist = 0;
                } else {
                    node.dist = node.dist.clamp(0, 252);
                }

                let node: u32 = bytemuck::must_cast(node);
                bytemuck::must_cast([node as u16; 8])
            },
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{utils::test_all_cpu_levels, test::OctreeCase};

    #[test]
    fn rasterize() {
        test_all_cpu_levels!(|cpu| {
            let mut grid = VoxelGrid::EMPTY;
            for OctreeCase { name, octree, grid: expect, .. } in OctreeCase::all() {
                super::rasterize(cpu, octree, &mut grid);
                assert_eq!(&grid, expect, "{name} ({cpu:?})");
            }
        });
    }

    #[test]
    fn rasterize_vanilla() {
        test_all_cpu_levels!(|cpu| {
            let mut grid = VoxelGrid::EMPTY;
            for OctreeCase { name, vanilla, grid: expect, .. } in OctreeCase::all() {
                let Some(octree) = vanilla else { continue };
                super::rasterize(cpu, octree, &mut grid);
                assert_eq!(&grid, expect, "{name} ({cpu:?})");
            }
        });
    }
}
