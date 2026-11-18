use std::{alloc, num::NonZero, ptr};

/// A terrain octree in the game's data format.
///
/// The octree structure and data contained may be invalid.
///
/// Each octree is guaranteed to contain at least one node and at most [`u16::MAX`] nodes.
#[derive(Debug, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct Octree {
    // INVARIANT: The length of `nodes` must fit in a `NonZero<u16>`.
    nodes: [Octnode],
}

/// A node in a terrain octree in the game's data format.
///
/// The fields may be invalid.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C, packed)]
pub struct Octnode {
    /// Material index of this node, or zero for an empty node.
    ///
    /// Byte value 255 is invalid.
    ///
    /// Valid material indices may not map to valid materials when used in-game, depending on which
    /// game is running and which mods are installed. This must be handled carefully, as invalid
    /// terrain materials can crash the game's world streamer.
    pub mat: u8,

    /// Signed distance from this node to the isosurface in range 1–252, encoded as
    /// `d * 126 + 126`, or zero for a node that's far from the isosurface.
    ///
    /// Byte values 253–255 are invalid.
    ///
    /// If the material index of this node is zero, the signed distance should be zero too.
    pub dist: u8,

    /// Index of this node's first child in the containing octree, or zero for a leaf node.
    ///
    /// Indices where any of the children would be outside the bounds of the containing octree, and
    /// indices that contain one of this node's parents, are invalid.
    pub child: u16,
}

impl Octree {
    /// An octree with a single empty root node.
    pub const EMPTY: &Self = Octree::new(&[Octnode::EMPTY]);

    /// Create a reference to an octree from a slice of nodes.
    ///
    /// If the input slice is empty, the returned octree has a single empty root node. If the input
    /// slice contains more than [`u16::MAX`] nodes, the returned octree is truncated.
    pub const fn new(nodes: &[Octnode]) -> &Self {
        let nodes = if nodes.is_empty() { &[Octnode::EMPTY] } else { nodes };
        let len = if nodes.len() > u16::MAX as usize { u16::MAX as usize } else { nodes.len() };
        let (nodes, _) = nodes.split_at(len);

        // SAFETY: The length of `nodes` fits in a `NonZero<u16>`.
        unsafe { Octree::new_unchecked(nodes) }
    }

    /// Create a mutable reference to an octree from a slice of nodes.
    ///
    /// If the input slice is empty, returns `None`. If the input slice contains more than
    /// [`u16::MAX`] nodes, the returned octree is truncated.
    pub const fn new_mut(nodes: &mut [Octnode]) -> Option<&mut Self> {
        if nodes.is_empty() { return None }
        let len = if nodes.len() > u16::MAX as usize { u16::MAX as usize } else { nodes.len() };
        let (nodes, _) = nodes.split_at_mut(len);

        // SAFETY: The length of `nodes` fits in a `NonZero<u16>`.
        unsafe { Some(Octree::new_unchecked_mut(nodes)) }
    }

    /// Create a boxed octree from a boxed slice of nodes.
    ///
    /// If the input slice is empty, the returned octree has a single empty root node. If the input
    /// slice contains more than [`u16::MAX`] nodes, the returned octree is truncated.
    pub fn new_boxed<A: alloc::Allocator>(nodes: Box<[Octnode], A>) -> Box<Self, A> {
        if nodes.is_empty() {
            let (_, alloc) = Box::into_raw_with_allocator(nodes);
            Octree::alloc_zeroed(NonZero::<u16>::MIN, alloc)
        } else if nodes.len() > u16::MAX as usize {
            let len = nodes.len();
            let (ptr, alloc) = Box::into_non_null_with_allocator(nodes);
            let ptr = ptr.cast::<Octnode>();

            // SAFETY: `len` is already known to be a valid size for this layout.
            let old_layout = unsafe {
                alloc::Layout::array::<Octnode>(len).unwrap_unchecked()
            };
            let new_layout = const {
                let Ok(layout) = alloc::Layout::array::<Octnode>(u16::MAX as usize) else {
                    panic!("max octree size should be valid")
                };
                layout
            };

            // SAFETY: `ptr` comes from a box that was allocated in `alloc`. `old_layout` is the
            // layout of the current data in the box, and `new_layout` is a valid layout with a
            // smaller size.
            let ptr = unsafe {
                alloc::Allocator::shrink(&alloc, ptr.cast::<u8>(), old_layout, new_layout)
            };
            let Ok(ptr) = ptr else { alloc::handle_alloc_error(new_layout) };
            let ptr = ptr.cast::<Octnode>().as_ptr();

            let ptr = ptr::slice_from_raw_parts_mut(ptr, u16::MAX as usize);

            // SAFETY: `ptr` comes from a box that was allocated in `alloc`. `Octree` is
            // `repr(transparent)` and has identical layout to `[Octnode]`. The length of `ptr` fits
            // in a `NonZero<u16>` and is the length that `ptr` was just shrunk to in `alloc`.
            unsafe { Box::from_raw_in(ptr as *mut Octree, alloc) }
        } else {
            // SAFETY: The length of `ptr` fits in a `NonZero<u16>`.
            unsafe { Octree::new_unchecked_boxed(nodes) }
        }
    }

    /// Create a reference to an octree from a slice of nodes without checking if the slice's length
    /// is in range.
    ///
    /// # Safety
    ///
    /// The input slice's length must fit in a `NonZero<u16>`. That is, it must be in range
    /// 1–65 535.
    pub const unsafe fn new_unchecked(nodes: &[Octnode]) -> &Self {
        // SAFETY: `Octree` is `repr(transparent)` and has identical layout to `[Octnode]`. The
        // length of `nodes` fits in a `NonZero<u16>` per safety contract.
        unsafe { &*(nodes as *const [Octnode] as *const Octree) }
    }

    /// Create a mutable reference to an octree from a slice of nodes without checking if the
    /// slice's length is in range.
    ///
    /// # Safety
    ///
    /// The input slice's length must fit in a `NonZero<u16>`. That is, it must be in range
    /// 1–65 535.
    pub const unsafe fn new_unchecked_mut(nodes: &mut [Octnode]) -> &mut Self {
        // SAFETY: `Octree` is `repr(transparent)` and has identical layout to `[Octnode]`. The
        // length of `nodes` fits in a `NonZero<u16>` per safety contract.
        unsafe { &mut *(nodes as *mut [Octnode] as *mut Octree) }
    }

    /// Create a boxed octree from a boxed slice of nodes without checking if the slice's length
    /// is in range.
    ///
    /// # Safety
    ///
    /// The input slice's length must fit in a `NonZero<u16>`. That is, it must be in range
    /// 1–65 535.
    pub unsafe fn new_unchecked_boxed<A: alloc::Allocator>(
        nodes: Box<[Octnode], A>,
    ) -> Box<Self, A> {
        let (ptr, alloc) = Box::into_raw_with_allocator(nodes);

        // SAFETY: `ptr` comes from a box that was allocated in `alloc`. `Octree` is
        // `repr(transparent)` and has identical layout to `[Octnode]`. The length of `ptr` fits
        // in a `NonZero<u16>` per safety contract.
        unsafe { Box::from_raw_in(ptr as *mut Octree, alloc) }
    }

    /// Allocate a boxed octree with `len` empty nodes.
    pub fn alloc_zeroed<A: alloc::Allocator>(len: NonZero<u16>, alloc: A) -> Box<Self, A> {
        let len = len.get() as usize;
        const {
            if alloc::Layout::array::<Octnode>(u16::MAX as usize).is_err() {
                panic!("max octree size should be valid")
            }
        }

        // SAFETY: `len` could not possibly produce an invalid layout size per constant assertion.
        let layout = unsafe { alloc::Layout::array::<Octnode>(len).unwrap_unchecked() };

        let Ok(ptr) = alloc::Allocator::allocate_zeroed(&alloc, layout) else {
            alloc::handle_alloc_error(layout)
        };
        let ptr = ptr::slice_from_raw_parts_mut(ptr.cast::<Octnode>().as_ptr(), len);
        let ptr = ptr as *mut Octree;

        // SAFETY: `ptr` was just allocated in `alloc`. The nodes are zero-initialized, and all-zero
        // is a valid bit-pattern for `Octnode`. The length of the slice pointer is correct and fits
        // in a `NonZero<u16>`.
        unsafe { Box::from_raw_in(ptr, alloc) }
    }

    /// Allocate a boxed octree as a clone of this octree.
    pub fn alloc_cloned<A: alloc::Allocator>(&self, alloc: A) -> Box<Self, A> {
        let len = self.len().get() as usize;

        // SAFETY: `len` is already known to be a valid size for this layout.
        let layout = unsafe { alloc::Layout::array::<Octnode>(len).unwrap_unchecked() };

        let Ok(ptr) = alloc::Allocator::allocate(&alloc, layout) else {
            alloc::handle_alloc_error(layout)
        };
        let ptr = ptr.cast::<Octnode>().as_ptr();

        // SAFETY: `self.nodes` and `ptr` have the same length and are separate allocations that can
        // not overlap. For the entire range of `len`, `nodes` is initialized and valid for reads
        // and `ptr` is allocated and valid for writes. `Octnode` is `Copy`.
        unsafe { ptr::copy_nonoverlapping(self.nodes.as_ptr(), ptr, len) }
        let ptr = ptr::slice_from_raw_parts_mut(ptr, len) as *mut Octree;

        // SAFETY: `ptr` was just allocated in `alloc`. The nodes were initialized by copying from
        // `self.nodes`. The length of the slice pointer is correct and fits in a `NonZero<u16>`.
        unsafe { Box::from_raw_in(ptr, alloc) }
    }

    /// The nodes in this octree.
    ///
    /// Guaranteed to not be empty.
    #[inline(always)]
    pub const fn nodes(&self) -> &[Octnode] { &self.nodes }

    /// Exclusive access to the nodes in this octree.
    ///
    /// Guaranteed to not be empty.
    pub const fn nodes_mut(&mut self) -> &mut [Octnode] { &mut self.nodes }

    /// Consume this octree, returning its nodes.
    ///
    /// Guaranteed to not be empty.
    pub fn into_nodes<A: alloc::Allocator>(self: Box<Self, A>) -> Box<[Octnode], A> {
        let (nodes, alloc) = Box::into_raw_with_allocator(self);

        // SAFETY: `Octree` is `repr(transparent)` and has identical layout to `[Octnode]`.
        unsafe { Box::from_raw_in(nodes as *mut [Octnode], alloc) }
    }

    /// The length of this octree in nodes.
    pub const fn len(&self) -> NonZero<u16> {
        // SAFETY: The length of `self.nodes` fits in a `NonZero<u16>` per invariants.
        unsafe { NonZero::new_unchecked(self.nodes.len() as u16) }
    }

    /// The root node (first node) of this octree.
    pub const fn root(&self) -> &Octnode {
        // SAFETY: `self.nodes` is guaranteed to be non-empty per invariants.
        unsafe { self.nodes.first().unwrap_unchecked() }
    }

    /// Get the children of a node in this octree, if it has children.
    ///
    /// Returns `None` if the node does not have children or if its children would be outside the
    /// bounds of this octree. The result could still be invalid even if this function returns
    /// `Some`, if the node's children contain one of its parents.
    ///
    /// If the node is not part of this octree, this function may panic, return `None`, or return
    /// nonsense.
    #[cfg_attr(debug_assertions, track_caller)]
    #[inline(always)]
    pub fn node_children(&self, node: &Octnode) -> Option<&[Octnode; 8]> {
        #[cfg(debug_assertions)]
        if self.nodes.element_offset(node).is_none() {
            panic!("node is not in octree")
        }

        if node.child == 0 { None } else { self.nodes.get(node.child as usize..)?.first_chunk() }
    }
}

impl ToOwned for Octree {
    type Owned = Box<Self>;
    fn to_owned(&self) -> Box<Self> { self.alloc_cloned(alloc::Global) }
}

impl AsRef<[Octnode]> for Octree {
    fn as_ref(&self) -> &[Octnode] { &self.nodes }
}

impl AsMut<[Octnode]> for Octree {
    fn as_mut(&mut self) -> &mut [Octnode] { &mut self.nodes }
}

impl Default for &'static Octree {
    fn default() -> Self { Octree::EMPTY }
}

impl<A: alloc::Allocator + Default> Default for Box<Octree, A> {
    fn default() -> Self { Octree::alloc_zeroed(NonZero::<u16>::MIN, A::default()) }
}

impl<A: alloc::Allocator + Clone> Clone for Box<Octree, A> {
    fn clone(&self) -> Self { self.alloc_cloned(Box::allocator(self).clone()) }
}

impl Octnode {
    /// An empty, zeroed leaf node.
    pub const EMPTY: Self = Octnode { mat: 0, dist: 0, child: 0 };
}
