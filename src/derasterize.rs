use std::{alloc, array, fmt, iter, mem::MaybeUninit, num::NonZero, ptr};
use gnat::array::CopyArr as Arr;
use crate::{
    utils::{simd::*, Cpu, CpuLevel, CpuDispatch, dispatch_cpu},
    VoxelGrid, Octree, SharedOctree, Octnode,
};

mod transpose;

#[inline(always)]
pub(crate) fn derasterize_method_impl<O: OctreeBuffer<Alloc = alloc::Global>>(
    voxels: &VoxelGrid,
) -> O {
    struct Derasterize<'r, O: OctreeBuffer>(&'r VoxelGrid, O::Alloc);
    impl<O: OctreeBuffer> CpuDispatch for Derasterize<'_, O> {
        type Output = O;

        #[inline(always)]
        fn dispatch(self, cpu: impl Cpu) -> O {
            let Derasterize(grid, alloc) = self;
            let mut scratch = DerasterScratch::new(cpu);
            derasterize(&mut scratch, grid, alloc)
        }
    }

    dispatch_cpu(const { CpuLevel::baseline() }, Derasterize(voxels, alloc::Global))
}

// TODO: relax initialization requirement for deraster scratch

/// Reusable scratch space used to derasterize octrees.
///
/// Needed by [`derasterize`].
///
/// The layout of this type depends on the instruction set level available on the current CPU.
#[derive(Debug, Copy, Clone)]
pub struct DerasterScratch<C: Cpu> {
    cpu: C,
    layers: OctreeLayers<C::Simd>,
}

impl<C: Cpu> DerasterScratch<C> {
    /// Create some scratch space for derasterizing octrees.
    #[inline(always)]
    pub fn new(cpu: C) -> Self {
        DerasterScratch {
            cpu,
            layers: OctreeLayers::zero(cpu.simd()),
        }
    }
}

/// Build an octree from a voxel grid.
///
/// The deepest level nodes of the octree, when present, preserve exactly the material and distance
/// values of their corresponding voxels. Higher nodes, including parent nodes, are computed from
/// their direct children: their material index is the most frequent material index among their
/// children (preferring surface nodes) and their signed distance is the mean of their children's
/// signed distances (rounding ties towards even) when any of their children are surface nodes. If
/// a node has no grandchildren and all of its children are fully solid, all are fully empty, or all
/// are identical, the node's children will be collapsed and they will not appear in the octree.
/// When a node's children are collapsed, the resulting node gets the most frequent material index
/// among its children. This means that the materials of fully sold voxels are not completely
/// preserved.
///
/// This function produces octrees in depth-first order.
///
/// The octree can be drawn back to a grid with [`rasterize`](crate::rasterize).
///
/// This function is for advanced users and users with performance concerns. For an easy-to-use
/// method, see [`VoxelGrid::derasterize`].
pub fn derasterize<O: OctreeBuffer>(
    scratch: &mut DerasterScratch<impl Cpu>,
    voxels: &VoxelGrid,
    alloc: O::Alloc,
) -> O {
    #[inline(always)]
    fn capture<C: Cpu, O: OctreeBuffer>(
        scratch: &mut DerasterScratch<C>, voxels: &VoxelGrid, alloc: O::Alloc,
    ) -> O {
        derasterize_impl::<C::Simd, O>(scratch.cpu.simd(), &mut scratch.layers, voxels, alloc)
    }

    capture(scratch, voxels, alloc)
}

#[simd]
#[inline(always)]
fn derasterize_impl<S: SimdExt, O: OctreeBuffer>(
    simd: S,
    layers: &mut OctreeLayers<S>,
    voxels: &VoxelGrid,
    alloc: O::Alloc,
) -> O {
    let nodes = calc_layers(simd, cast_ref_as_simd_arr(simd, voxels), layers);
    emit_octree(simd, cast_ref_as_simd_arr(simd, voxels), layers, nodes, alloc)
}

type DivCeil<A, B> = gnat::eval!((A + B - 1) / B);
type Level0Chunks<W> = gnat::eval!(DivCeil(32768, W));
type Level1Chunks<W> = gnat::eval!(DivCeil(4096, W));
type Level2Chunks<W> = gnat::eval!(DivCeil(512, W));
type Level3Chunks<W> = gnat::eval!(DivCeil(64, W));
type Level4Chunks<W> = gnat::eval!(DivCeil(8, W));
type Level5Chunks<W> = gnat::eval!(DivCeil(1, W));

struct OctreeLayers<S: SimdExt> {
    level1: Arr<LayerChunk<S>, Level1Chunks<S::Width>>,
    level2: Arr<LayerChunk<S>, Level2Chunks<S::Width>>,
    level3: Arr<LayerChunk<S>, Level3Chunks<S::Width>>,
    level4: Arr<LayerChunk<S>, Level4Chunks<S::Width>>,
    level5: Arr<LayerChunk<S>, Level5Chunks<S::Width>>,
}

#[derive(Copy, Clone)]
struct LayerChunk<S: SimdExt> {
    mat: S::u8s,
    dist: S::u8s,
    is_parent: S::mask8s,
}

impl<S: SimdExt> OctreeLayers<S> {
    #[inline(always)]
    fn zero(simd: S) -> Self {
        OctreeLayers {
            level1: Arr::of(LayerChunk::zero(simd)),
            level2: Arr::of(LayerChunk::zero(simd)),
            level3: Arr::of(LayerChunk::zero(simd)),
            level4: Arr::of(LayerChunk::zero(simd)),
            level5: Arr::of(LayerChunk::zero(simd)),
        }
    }
}

impl<S: SimdExt> LayerChunk<S> {
    #[inline(always)]
    fn zero(simd: S) -> Self {
        LayerChunk {
            mat: S::u8s::splat(simd, 0),
            dist: S::u8s::splat(simd, 0),
            is_parent: S::mask8s::splat(simd, false),
        }
    }
}

impl<S: SimdExt> Copy for OctreeLayers<S> { }
impl<S: SimdExt> Clone for OctreeLayers<S> { fn clone(&self) -> Self { *self } }

impl<S: SimdExt> fmt::Debug for OctreeLayers<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("OctreeLayers")
            .field("level1", &self.level1)
            .field("level2", &self.level2)
            .field("level3", &self.level3)
            .field("level4", &self.level4)
            .field("level5", &self.level5)
            .finish()
    }
}

impl<S: SimdExt> fmt::Debug for LayerChunk<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("LayerChunk")
            .field("mat", &self.mat)
            .field("dist", &self.dist)
            .field("is_parent", &self.is_parent.to_vector())
            .finish()
    }
}

#[simd]
fn calc_layers<S: SimdExt>(
    simd: S,
    voxels: &Arr<S::u16s, gnat::eval!(Level0Chunks(S::Width / 2))>,
    layers: &mut OctreeLayers<S>,
) -> NonZero<usize> {
    let mut nodes = NonZero::<usize>::MIN;
    let mut count_chunk = {
        #[inline(always)] |chunk: &LayerChunk<S>| {
            nodes = nodes.checked_add(chunk.is_parent.to_bitmask().count_ones() as usize * 8)
                .expect("total nodes should be in range for usize");
        }
    };

    calc_first_layer(simd, voxels, &mut layers.level1, &mut count_chunk);
    calc_parent_layer(simd, &layers.level1, &mut layers.level2, &mut count_chunk);
    calc_parent_layer(simd, &layers.level2, &mut layers.level3, &mut count_chunk);
    calc_parent_layer(simd, &layers.level3, &mut layers.level4, &mut count_chunk);
    calc_parent_layer(simd, &layers.level4, &mut layers.level5, &mut count_chunk);

    nodes
}

#[simd]
#[inline(always)]
fn calc_first_layer<S: SimdExt>(
    simd: S,
    voxels: &Arr<S::u16s, gnat::eval!(Level0Chunks(S::Width / 2))>,
    layer: &mut Arr<LayerChunk<S>, Level1Chunks<S::Width>>,
    mut count_chunk: impl FnMut(&LayerChunk<S>),
) {
    let voxel_chunks: &Arr<[S::u16s; 16], Level1Chunks<S::Width>> = chunk_arr_ref(voxels);
    for (chunk_voxels, slot) in voxel_chunks.as_slice().iter().zip(layer.as_mut_slice()) {
        let first = S::u16s::splat(simd, chunk_voxels[0][0]);
        let identical = chunk_voxels.iter()
            .fold(S::mask16s::splat(simd, true), #[inline(always)] |a, b| a & b.simd_eq(first))
            .all_true();

        let chunk = if identical {
            let [mat, dist] = first[0].to_le_bytes();
            LayerChunk {
                mat: S::u8s::splat(simd, mat),
                dist: S::u8s::splat(simd, dist),
                is_parent: S::mask8s::splat(simd, false),
            }
        } else {
            let voxel_bytes: &[S::u8s; 16] = cast_simd_arr_ref(chunk_voxels);
            let interleaved = transpose::transpose_u8_Wx16(simd, *voxel_bytes);
            let mat = array::from_fn(#[inline(always)] |i| interleaved[2 * i]);
            let dist = array::from_fn(#[inline(always)] |i| interleaved[2 * i + 1]);

            calc_layer_chunk(simd, &mat, &dist, S::mask8s::splat(simd, false))
        };

        count_chunk(&chunk);
        *slot = chunk;
    }
}

#[simd]
#[inline(always)]
fn calc_parent_layer<S: SimdExt, P: gnat::Nat, L: gnat::Nat>(
    simd: S,
    prev: &Arr<LayerChunk<S>, P>,
    layer: &mut Arr<LayerChunk<S>, L>,
    mut count_chunk: impl FnMut(&LayerChunk<S>),
) {
    let (chunk_children, rem) = prev.as_slice().as_chunks::<8>();
    let mut remainder = [LayerChunk::zero(simd); 8];
    remainder[..rem.len()].copy_from_slice(rem);
    let chunk_children = chunk_children.iter().chain(iter::once(&remainder));

    for (children, slot) in chunk_children.zip(layer.as_mut_slice()) {
        let mat = transpose::transpose_u8_Wx8(simd, {
            children.map(#[inline(always)] |chunk| chunk.mat)
        });
        let dist = transpose::transpose_u8_Wx8(simd, {
            children.map(#[inline(always)] |chunk| chunk.dist)
        });

        let is_parent = {
            transpose::transpose_u8_Wx8(simd, {
                children.map(#[inline(always)] |chunk| chunk.is_parent.to_vector().cast_unsigned())
            })
            .map(#[inline(always)] |child| S::mask8s::from_vector(child.cast_signed()))
        };
        let is_grandparent = is_parent.iter()
            .fold(S::mask8s::splat(simd, false), #[inline(always)] |a, &b| a | b);

        let chunk = calc_layer_chunk(simd, &mat, &dist, is_grandparent);
        count_chunk(&chunk);
        *slot = chunk;
    }
}

#[simd]
#[inline(always)]
fn calc_layer_chunk<S: SimdExt>(
    simd: S, mat: &[S::u8s; 8], dist: &[S::u8s; 8], is_grandparent: S::mask8s,
) -> LayerChunk<S> {
    let empty: [S::mask8s; 8] = array::from_fn(#[inline(always)] |i| {
        mat[i].simd_eq(S::u8s::splat(simd, 0))
    });
    let nonsurface: [S::mask8s; 8] = array::from_fn(#[inline(always)] |i| {
        dist[i].simd_eq(S::u8s::splat(simd, 0))
    });

    let mut all_mats_eq = S::mask8s::splat(simd, true);
    let mut all_dists_eq = S::mask8s::splat(simd, true);
    let mut all_empty = S::mask8s::splat(simd, true);
    let mut all_nonsurface = S::mask8s::splat(simd, true);
    let mut any_empty = S::mask8s::splat(simd, false);
    for i in 0..8 {
        all_mats_eq &= mat[i].simd_eq(mat[0]);
        all_dists_eq &= dist[i].simd_eq(dist[0]);
        all_empty &= empty[i];
        all_nonsurface &= nonsurface[i];
        any_empty &= empty[i];
    }

    let identical = all_mats_eq & all_dists_eq;
    let homogenous_children = identical | (all_nonsurface & (all_empty | !any_empty));
    let is_parent = is_grandparent | !homogenous_children;

    let chunk_mat = if (!all_mats_eq).to_bitmask() == 0 {
        mat[0]
    } else {
        calc_chunk_mat(simd, mat, &empty, &nonsurface)
    };

    if is_parent.to_bitmask() == 0 {
        return LayerChunk {
            mat: chunk_mat,
            dist: dist[0],
            is_parent: S::mask8s::splat(simd, false),
        }
    }

    let parent_dist = calc_parent_dist(simd, dist, &empty, &nonsurface);
    let parent_dist = parent_dist.max({
        chunk_mat.simd_ne(S::u8s::splat(simd, 0))
            .select(S::u8s::splat(simd, 1), S::u8s::splat(simd, 0))
    });

    LayerChunk {
        mat: chunk_mat,
        dist: is_parent.select(parent_dist, dist[0]),
        is_parent,
    }
}

#[simd]
#[inline(always)]
fn calc_chunk_mat<S: SimdExt>(
    simd: S, mat: &[S::u8s; 8], empty: &[S::mask8s; 8], nonsurface: &[S::mask8s; 8],
) -> S::u8s {
    let weight: [S::u8s; 8] = array::from_fn(#[inline(always)] |i| {
        nonsurface[i].select(S::u8s::splat(simd, 0x01), S::u8s::splat(simd, 0x11))
    });

    let (mut res, mut max_points) = (mat[0], S::u8s::splat(simd, 0));
    for c in 0..8 {
        let mut votes = S::u8s::splat(simd, 0);
        for v in 0..8 {
            votes += mat[c].simd_eq(mat[v]).select(weight[v], S::u8s::splat(simd, 0));
        }

        let mask = nonsurface[c].select(S::u8s::splat(simd, 0x0F), S::u8s::splat(simd, 0xFF));
        let points = empty[c].select(S::u8s::splat(simd, 0), votes & mask);

        let new_max_points = max_points.max(points);
        res = new_max_points.simd_eq(max_points).select(res, mat[c]);
        max_points = new_max_points;
    }

    res
}

#[simd]
#[inline(always)]
fn calc_parent_dist<S: SimdExt>(
    simd: S, dist: &[S::u8s; 8], empty: &[S::mask8s; 8], nonsurface: &[S::mask8s; 8],
) -> S::u8s {
    let (mut sum_lo, mut sum_hi) = (S::u16s::splat(simd, 0), S::u16s::splat(simd, 0));
    for i in 0..8 {
        let dist = nonsurface[i].select(
            empty[i].select(S::u8s::splat(simd, 1), S::u8s::splat(simd, 252)),
            dist[i],
        );

        let (dist_lo, dist_hi) = dist.widen();
        sum_lo += dist_lo; sum_hi += dist_hi;
    }

    let mean_lo = (sum_lo + 3 + ((sum_lo >> 3) & 1)) >> 3;
    let mean_hi = (sum_hi + 3 + ((sum_hi >> 3) & 1)) >> 3;
    mean_lo.relaxed_narrow(mean_hi)
}

#[simd]
fn emit_octree<S: SimdExt, O: OctreeBuffer>(
    simd: S,
    voxels: &[u16x8<S>; 4096], layers: &OctreeLayers<S>,
    nodes: NonZero<usize>, alloc: O::Alloc,
) -> O {
    let nodes = NonZero::<u16>::try_from(nodes)
        .expect("octree buffer len should not overflow u16");

    let mut buf = <O as OctreeBufferSealed>::ReverseEmit::alloc(nodes, alloc);
    let root = emit_children(simd, voxels, layers, &mut buf, MaxDepth::<3>, 0);
    buf.finish(root)
}

#[simd]
#[inline(always)]
fn emit_children<S: SimdExt>(
    simd: S,
    voxels: &[u16x8<S>; 4096], layers: &OctreeLayers<S>,
    buf: &mut impl ReverseEmitOctree,
    depth: impl EmitDepth, idx: usize,
) -> u32 {
    let layer = depth.layer(layers);
    let (node_chunk, lane) = (&layer.as_slice()[idx / S::u8s::LEN], idx % S::u8s::LEN);

    let (mat, dist) = (node_chunk.mat[lane], node_chunk.dist[lane]);
    let voxel = mat as u32 | (dist as u32) << 8;
    if !node_chunk.is_parent.test(lane) { return voxel }

    let children = depth.descend(
        &mut *buf,

        #[inline(always)] |buf, depth| {
            let mut children = [0; 8];
            for c in (0..8).rev() {
                children[c] = emit_children(simd, voxels, layers, buf, depth, 8 * idx + c);
            }
            u32x8::load_array(simd, children)
        },

        #[inline(always)] |buf| {
            emit_grandchildren(simd, voxels, &layers.level1, buf, idx)
        },
    );

    let pos = buf.push_siblings(children);
    voxel | (pos as u32) << 16
}

#[simd]
#[inline(always)]
fn emit_grandchildren<S: SimdExt>(
    simd: S,
    voxels: &[u16x8<S>; 4096], layer: &Arr<LayerChunk<S>, Level1Chunks<S::Width>>,
    buf: &mut impl ReverseEmitOctree,
    idx: usize,
) -> u32x8<S> {
    const { assert!(S::u8s::LEN % 8 == 0); }
    let subchunks = const { S::u8s::LEN / 8 };
    let (children_chunk, subchunk_idx) = (&layer.as_slice()[idx / subchunks], idx % subchunks);

    let is_parent = children_chunk.is_parent.to_bitmask() >> (subchunk_idx * 8);
    let mut children = [0; 8];
    for c in (0..8).rev() {
        if is_parent & (1 << c) != 0 {
            let (grandchildren_lo, grandchildren_hi) = voxels[8 * idx + c].widen();
            let grandchildren = grandchildren_lo.combine(grandchildren_hi);
            children[c] = buf.push_siblings(grandchildren) as u32;
        }
    }
    let children = u32x8::load_array(simd, children);

    #[inline(always)]
    fn load_subchunk_field<S: SimdExt>(chunk_field: &S::u8s, subchunk_idx: usize) -> u8x16<S> {
        let simd = chunk_field.token();
        let subchunk = chunk_field.as_slice().as_chunks::<8>().0[subchunk_idx];
        u8x16::load_array(simd, bytemuck::must_cast([subchunk, [0; 8]]))
    }

    let mat = load_subchunk_field(&children_chunk.mat, subchunk_idx);
    let dist = load_subchunk_field(&children_chunk.dist, subchunk_idx);
    let (voxel_lo, voxel_hi) = mat.zip_low(dist).bitcast::<u16x8<S>>().widen();
    let voxel = voxel_lo.combine(voxel_hi);

    children << 16 | voxel
}

trait EmitDepth: Copy {
    type Descend: EmitDepth;

    fn descend<T, R>(
        self, val: T,
        descend: impl FnOnce(T, Self::Descend) -> R,
        done: impl FnOnce(T) -> R,
    ) -> R;

    type LevelChunks<W: gnat::Nat>: gnat::Nat;

    fn layer<'l, S: SimdExt>(
        &self, layers: &'l OctreeLayers<S>,
    ) -> &'l Arr<LayerChunk<S>, Self::LevelChunks<S::Width>>;
}

#[derive(Copy, Clone)]
struct MaxDepth<const N: usize>;

macro_rules! impl_emit_depth {
    ($($depth:literal => $layer:ident: $num:ident),* $(,)?) => {$(
        impl EmitDepth for MaxDepth<$depth> {
            type Descend = MaxDepth<{ $depth - 1 }>;

            #[inline(always)]
            fn descend<T, R>(
                self, val: T,
                descend: impl FnOnce(T, Self::Descend) -> R,
                _: impl FnOnce(T) -> R,
            ) -> R {
                descend(val, MaxDepth)
            }

            type LevelChunks<W: gnat::Nat> = $num<W>;

            #[inline(always)]
            fn layer<'l, S: SimdExt>(
                &self, layers: &'l OctreeLayers<S>,
            ) -> &'l Arr<LayerChunk<S>, $num<S::Width>> {
                &layers.$layer
            }
        }
    )*};
}
impl_emit_depth!(3 => level5: Level5Chunks, 2 => level4: Level4Chunks, 1 => level3: Level3Chunks);

impl EmitDepth for MaxDepth<0> {
    type Descend = AtDepth;

    #[inline(always)]
    fn descend<T, R>(
        self, val: T,
        _: impl FnOnce(T, AtDepth) -> R,
        done: impl FnOnce(T) -> R,
    ) -> R {
        done(val)
    }

    type LevelChunks<W: gnat::Nat> = Level2Chunks<W>;

    #[inline(always)]
    fn layer<'l, S: SimdExt>(
        &self, layers: &'l OctreeLayers<S>,
    ) -> &'l Arr<LayerChunk<S>, Level2Chunks<S::Width>> {
        &layers.level2
    }
}

#[derive(Copy, Clone)]
enum AtDepth { }

impl EmitDepth for AtDepth {
    type Descend = AtDepth;

    #[inline(always)]
    fn descend<T, R>(
        self, _: T,
        _: impl FnOnce(T, AtDepth) -> R,
        _: impl FnOnce(T) -> R,
    ) -> R {
        match self { }
    }

    type LevelChunks<W: gnat::Nat> = gnat::lit!(0);

    #[inline(always)]
    fn layer<'l, S: SimdExt>(
        &self, _: &'l OctreeLayers<S>,
    ) -> &'l Arr<LayerChunk<S>, gnat::lit!(0)> {
        match *self { }
    }
}

/// An octree buffer, typically allocated on the heap.
pub trait OctreeBuffer: Sized + seal_octree_buffer::Sealed {
    /// The allocator type used by this octree buffer on the heap.
    type Alloc: alloc::Allocator;
}

use seal_octree_buffer::Sealed as OctreeBufferSealed;
mod seal_octree_buffer {
    use super::*;

    pub trait Sealed {
        #[expect(private_bounds)]
        type ReverseEmit: ReverseEmitOctree<Octree = Self>;
    }
}

trait ReverseEmitOctree {
    type Octree: OctreeBuffer;
    fn alloc(len: NonZero<u16>, alloc: <Self::Octree as OctreeBuffer>::Alloc) -> Self;
    fn push_siblings(&mut self, siblings: u32x8<impl Simd>) -> u16;
    fn finish(self, root: u32) -> Self::Octree;
}

impl<A: alloc::Allocator> OctreeBuffer for Box<Octree, A> {
    type Alloc = A;
}

impl<A: alloc::Allocator> OctreeBufferSealed for Box<Octree, A> {
    type ReverseEmit = ReverseEmitBoxedOctree<A>;
}

use reverse_emit_boxed_octree::ReverseEmitBoxedOctree;
mod reverse_emit_boxed_octree {
    use super::*;

    pub struct ReverseEmitBoxedOctree<A: alloc::Allocator> {
        // INVARIANT: All elements of `buf` from `curr` onwards must be initialized. `curr` must not
        // be greater than the length of `buf`. `buf` must be at least one node in size. `curr` and
        // the length of `buf` each must not overflow `u16`.
        buf: Box<[MaybeUninit<Octnode>], A>,
        curr: usize,
    }

    impl<A: alloc::Allocator> ReverseEmitOctree for ReverseEmitBoxedOctree<A> {
        type Octree = Box<Octree, A>;

        #[inline(always)]
        fn alloc(len: NonZero<u16>, alloc: A) -> Self {
            let len = len.get() as usize;
            let layout = alloc::Layout::array::<MaybeUninit<Octnode>>(len)
                .expect("octree buffer len should not overflow isize");

            let Ok(ptr) = alloc::Allocator::allocate(&alloc, layout) else {
                alloc::handle_alloc_error(layout)
            };

            let ptr = ptr::slice_from_raw_parts_mut(
                ptr.as_ptr().cast::<MaybeUninit<Octnode>>(), len,
            );

            // SAFETY: `MaybeUninit<Octnode>` is valid when uninitialized, and `ptr` was
            // successfully allocated by `alloc` with the correct size and alignment.
            let buf = unsafe { Box::from_raw_in(ptr, alloc) };
            ReverseEmitBoxedOctree { buf, curr: len }
        }

        #[simd]
        #[inline(always)]
        fn push_siblings(&mut self, siblings: u32x8<impl Simd>) -> u16 {
            let Some(idx) = self.curr.checked_sub(8) else {
                panic!("octree buffer capacity overflow")
            };

            // SAFETY: `idx` is less than `curr`, which is no greater than the length of `buf` per
            // invariants.
            unsafe {
                (&raw mut self.buf[0])
                    .add(idx)
                    .cast::<u32x8<_>>()
                    .write_unaligned(siblings)
            }

            self.curr = idx;
            idx as u16
        }

        #[inline(always)]
        fn finish(mut self, root: u32) -> Box<Octree, A> {
            let Some(root_idx) = self.curr.checked_sub(1) else {
                panic!("octree buffer capacity overflow")
            };

            assert_eq!(root_idx, 0, "octree buffer not filled");

            // SAFETY: `buf` is always at least one node in size per invariants.
            unsafe { (&raw mut self.buf[0]).cast::<u32>().write_unaligned(root) }

            // SAFETY: All nodes from `curr` onwards were initialized per invariants, and the root
            // node was just initialized.
            let nodes = unsafe { self.buf.assume_init() };

            // SAFETY: The length of `nodes` is in range for `NonZero<u16>` per invariants.
            unsafe { Octree::new_unchecked_boxed(nodes) }
        }
    }
}

impl OctreeBuffer for SharedOctree {
    type Alloc = alloc::Global;
}

impl OctreeBufferSealed for SharedOctree {
    type ReverseEmit = ReverseEmitSharedOctree;
}

use reverse_emit_shared_octree::ReverseEmitSharedOctree;
mod reverse_emit_shared_octree {
    use super::*;

    pub struct ReverseEmitSharedOctree {
        // INVARIANT: If `buf` is `Some`, all elements of `buf` from `curr` onwards must be
        // initialized. `curr` must not be greater than the length of `buf`. `buf` must be at least
        // two nodes in size. `curr` and the length of `buf` each must not overflow `u16`.
        buf: Option<ptr::NonNull<[Octnode]>>,
        curr: usize,
    }

    impl ReverseEmitOctree for ReverseEmitSharedOctree {
        type Octree = SharedOctree;

        #[inline(always)]
        fn alloc(len: NonZero<u16>, _: alloc::Global) -> Self {
            let buf = SharedOctree::alloc_heap_ptr(len);
            let curr = len.get() as usize;

            ReverseEmitSharedOctree { buf, curr }
        }

        #[simd]
        #[inline(always)]
        fn push_siblings(&mut self, siblings: u32x8<impl Simd>) -> u16 {
            let Some(buf) = self.buf else {
                panic!("octree buffer capacity overflow")
            };

            let Some(idx) = self.curr.checked_sub(8) else {
                panic!("octree buffer capacity overflow")
            };

            // SAFETY: `idx` is less than `curr`, which is no greater than the length of `buf` per
            // invariants. `u32x8` is guaranteed to have the same memory layout as `[Octnode; 8]`
            // excepting alignment.
            unsafe {
                buf.cast::<Octnode>()
                    .add(idx)
                    .cast::<u32x8<_>>()
                    .write_unaligned(siblings)
            }

            self.curr = idx;
            idx as u16
        }

        #[inline(always)]
        fn finish(self, root: u32) -> SharedOctree {
            let Some(buf) = self.buf else {
                return SharedOctree::from_root(bytemuck::must_cast(root))
            };

            let Some(root_idx) = self.curr.checked_sub(1) else {
                panic!("octree buffer capacity overflow")
            };

            assert_eq!(root_idx, 0, "octree buffer not filled");

            // SAFETY: `buf` is always at least one node in size per invariants.
            unsafe { buf.cast::<u32>().write_unaligned(root) }

            // SAFETY: `buf` came from `alloc_heap_ptr` and its address, provenance and length are
            // all unmodified. All nodes from `curr` onwards were initialized per invariants, and
            // the root node was just initialized.
            unsafe { SharedOctree::from_heap_ptr(buf) }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::utils::test_all_cpu_levels;

    #[test]
    fn derasterize_empty_grid() {
        test_all_cpu_levels!(|cpu| {
            let grid = VoxelGrid::EMPTY;

            let mut scratch = DerasterScratch::new(cpu);
            let octree = derasterize::<Box<Octree>>(&mut scratch, &grid, alloc::Global);

            let expect = Octree::EMPTY.nodes();
            assert_eq!(octree.nodes(), expect, "{cpu:?}");
        })
    }

    #[test]
    fn derasterize_sphere_grid() {
        test_all_cpu_levels!(|cpu| {
            let grid = const {
                &VoxelGrid(bytemuck::must_cast(*include_bytes!("../tests/cases/sphere-grid.bin")))
            };

            let mut scratch = DerasterScratch::new(cpu);
            let octree = derasterize::<Box<Octree>>(&mut scratch, &grid, alloc::Global);

            let expect = bytemuck::cast_slice(include_bytes!("../tests/cases/sphere-octree.bin"));
            assert_eq!(octree.nodes(), expect, "{cpu:?}");
        });
    }

    #[test]
    fn derasterize_sphere_then_empty() {
        test_all_cpu_levels!(|cpu| {
            let mut scratch = DerasterScratch::new(cpu);

            let grid = const {
                &VoxelGrid(bytemuck::must_cast(*include_bytes!("../tests/cases/sphere-grid.bin")))
            };
            let octree = derasterize::<Box<Octree>>(&mut scratch, &grid, alloc::Global);
            let expect = bytemuck::cast_slice(include_bytes!("../tests/cases/sphere-octree.bin"));
            assert_eq!(octree.nodes(), expect, "{cpu:?}");

            let grid = VoxelGrid::EMPTY;
            let octree = derasterize::<Box<Octree>>(&mut scratch, &grid, alloc::Global);
            let expect = Octree::EMPTY.nodes();
            assert_eq!(octree.nodes(), expect, "{cpu:?}");
        });
    }

    #[test]
    fn derasterize_shared_octree() {
        test_all_cpu_levels!(|cpu| {
            let mut scratch = DerasterScratch::new(cpu);

            let grid = const {
                &VoxelGrid(bytemuck::must_cast(*include_bytes!("../tests/cases/sphere-grid.bin")))
            };
            let octree = derasterize::<SharedOctree>(&mut scratch, &grid, alloc::Global);
            let expect = bytemuck::cast_slice(include_bytes!("../tests/cases/sphere-octree.bin"));
            assert_eq!(octree.nodes(), expect, "{cpu:?}");

            let grid = VoxelGrid::EMPTY;
            let octree = derasterize::<SharedOctree>(&mut scratch, &grid, alloc::Global);
            let expect = Octree::EMPTY.nodes();
            assert_eq!(octree.nodes(), expect, "{cpu:?}");
        });
    }
}
