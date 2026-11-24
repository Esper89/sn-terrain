use std::{alloc, fmt, mem, num::NonZero, ops, ptr};
use triomphe::ThinArc;

/// A terrain octree in the game's data format.
///
/// The octree structure and data contained may be invalid. Each octree is guaranteed to contain at
/// least one node and at most [`u16::MAX`] nodes.
///
/// This type is a dynamically-sized wrapper around a slice of [`Octnode`]s.
///
#[doc = include_str!("dist-invariant.md")]
#[derive(Debug)]
#[repr(transparent)]
pub struct Octree {
    // INVARIANT: The length of `nodes` must fit in a `NonZero<u16>`.
    nodes: [Octnode],
}

impl Octree {
    /// An octree with a single empty root node.
    pub const EMPTY: &Self = Octree::new_array(&[Octnode::EMPTY]);

    /// Create a reference to an octree from a slice of nodes.
    ///
    /// If the input slice is empty, the returned octree has a single empty root node. If the input
    /// slice contains more than [`u16::MAX`] nodes, the returned octree is truncated.
    pub const fn new(nodes: &[Octnode]) -> &Self {
        if nodes.is_empty() { return Octree::EMPTY }
        let len = if nodes.len() > u16::MAX as usize { u16::MAX as usize } else { nodes.len() };
        let (nodes, _) = nodes.split_at(len);

        // SAFETY: The length of `nodes` fits in a `NonZero<u16>`.
        unsafe { Octree::new_unchecked(nodes) }
    }

    /// Create an exclusive reference to an octree from a slice of nodes.
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

    /// Create an exclusive reference to an octree from a slice of nodes without checking if the
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

    /// Create a reference to an octree from a fixed-length array of nodes.
    ///
    /// If the input array is empty or contains more than [`u16::MAX`] nodes, this will panic at
    /// compile-time.
    pub const fn new_array<const N: usize>(nodes: &[Octnode; N]) -> &Self {
        const { if N == 0 || N > u16::MAX as usize { panic!("invalid octree length") } }

        // SAFETY: The length of `nodes` fits in a `NonZero<u16>`.
        unsafe { Octree::new_unchecked(nodes) }
    }

    /// Create an exclusive reference to an octree from a fixed-length array of nodes.
    ///
    /// If the input array is empty or contains more than [`u16::MAX`] nodes, this will panic at
    /// compile-time.
    pub const fn new_array_mut<const N: usize>(nodes: &mut [Octnode; N]) -> &mut Self {
        const { if N == 0 || N > u16::MAX as usize { panic!("invalid octree length") } }

        // SAFETY: The length of `nodes` fits in a `NonZero<u16>`.
        unsafe { Octree::new_unchecked_mut(nodes) }
    }

    /// Create a boxed octree from a fixed-length boxed array of nodes.
    ///
    /// If the input array is empty or contains more than [`u16::MAX`] nodes, this will panic at
    /// compile-time.
    pub fn new_array_boxed<const N: usize, A: alloc::Allocator>(
        nodes: Box<[Octnode; N], A>
    ) -> Box<Self, A> {
        const { if N == 0 || N > u16::MAX as usize { panic!("invalid octree length") } }

        // SAFETY: The length of `nodes` fits in a `NonZero<u16>`.
        unsafe { Octree::new_unchecked_boxed(nodes) }
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

    /// Parse a reference to an octree from a slice of bytes, returning the remaining bytes.
    ///
    /// The first two bytes are interpreted as the (little-endian) length of the octree. If the
    /// octree would be longer than the remaining bytes, it is truncated. If the octree would have
    /// zero nodes, a single-node empty octree is returned instead.
    ///
    /// If the input contains less than two bytes, `None` is returned and zero remaining bytes are
    /// returned.
    pub const fn parse(bytes: &[u8]) -> (Option<&Self>, &[u8]) {
        let [len_a, len_b, bytes @ ..] = bytes else { return (None, &[]) };
        let len = u16::from_le_bytes([*len_a, *len_b]);

        let split = len as usize * 4;
        let split = if split > bytes.len() { bytes.len() & !3 } else { split };
        let (bytes, rem) = bytes.split_at(split);

        // SAFETY: `bytes.len()` is a multiple of 4.
        let nodes = unsafe { bytes.as_chunks_unchecked::<4>() };
        let nodes = bytemuck::must_cast_slice::<[u8; 4], Octnode>(nodes);

        if nodes.is_empty() {
            (Some(Octree::EMPTY), rem)
        } else {
            // SAFETY: `nodes` is not empty and its length is derived from a `u16`.
            unsafe { (Some(Octree::new_unchecked(nodes)), rem) }
        }
    }

    /// Parse an exclusive reference to an octree from a slice of bytes, returning the remaining
    /// bytes.
    ///
    /// The first two bytes are interpreted as the (little-endian) length of the octree. If the
    /// octree would be longer than the remaining bytes, it is truncated. If the octree would have
    /// zero nodes, `None` is returned instead.
    ///
    /// If the input contains less than two bytes, `None` is returned and zero remaining bytes are
    /// returned.
    pub const fn parse_mut(bytes: &mut [u8]) -> (Option<&mut Self>, &mut [u8]) {
        let [len_a, len_b, bytes @ ..] = bytes else { return (None, &mut []) };
        let len = u16::from_le_bytes([*len_a, *len_b]);

        let split = len as usize * 4;
        let split = if split > bytes.len() { bytes.len() & !3 } else { split };
        let (bytes, rem) = bytes.split_at_mut(split);

        // SAFETY: `bytes.len()` is a multiple of 4.
        let nodes = unsafe { bytes.as_chunks_unchecked_mut::<4>() };
        let nodes = bytemuck::must_cast_slice_mut::<[u8; 4], Octnode>(nodes);

        if nodes.is_empty() {
            (None, rem)
        } else {
            // SAFETY: `nodes` is not empty and its length is derived from a `u16`.
            unsafe { (Some(Octree::new_unchecked_mut(nodes)), rem) }
        }
    }

    /// The nodes in this octree.
    ///
    /// Guaranteed to not be empty.
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

    /// The bytes that make up the nodes in this octree.
    ///
    /// Guaranteed to be at least four bytes long.
    pub const fn as_bytes(&self) -> &[u8] {
        bytemuck::must_cast_slice(self.nodes())
    }

    /// Exclusive access to the bytes that make up the nodes in this octree.
    ///
    /// Guaranteed to be at least four bytes long.
    pub const fn as_bytes_mut(&mut self) -> &mut [u8] {
        bytemuck::must_cast_slice_mut(self.nodes_mut())
    }

    /// Consume this octree, returning the bytes that make up its nodes.
    ///
    /// Guaranteed to be at least four bytes long.
    pub fn into_bytes<A: alloc::Allocator>(self: Box<Self, A>) -> Box<[u8], A> {
        let (nodes, alloc) = Box::into_raw_with_allocator(self);
        let nodes = nodes as *mut [Octnode];
        let bytes = ptr::slice_from_raw_parts_mut(nodes.cast::<u8>(), nodes.len() * 4);

        // SAFETY: `Octree` is `repr(transparent)` and has identical layout to `[Octnode]`, which
        // is four times the size of `[u8]`. `Octnode` and `u8` have the same alignment and the
        // length of `bytes` is four times the length of `nodes`.
        unsafe { Box::from_raw_in(bytes, alloc) }
    }

    /// The length of this octree in nodes.
    pub const fn len(&self) -> NonZero<u16> {
        // SAFETY: The length of `self.nodes` fits in a `NonZero<u16>` per invariants.
        unsafe { NonZero::new_unchecked(self.nodes.len() as u16) }
    }

    /// If this octree contains only a single empty root node.
    pub const fn is_empty(&self) -> bool {
        self.len().get() == 1 && self.root().mat == 0 && self.root().dist == 0
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
    #[inline(always)]
    pub fn node_children(&self, node: &Octnode) -> Option<&[Octnode; 8]> {
        debug_assert!(self.nodes.element_offset(node).is_some(), "node is not in octree");
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

/// A cheaply-clonable immutable shared terrain octree in the game's data format.
///
/// The octree structure and data contained may be invalid. Each octree is guaranteed to contain at
/// least one node and at most [`u16::MAX`] nodes.
///
/// This type is implemented as a thin pointer to a reference-counted buffer holding a slice of
/// [`Octnode`]s. Single-node octrees (containing only a root node) are stored inline instead of
/// using a heap-allocated buffer. On both 32- and 64-bit platforms, both this type and an
/// [`Option`] of it are 8 bytes.
///
/// `Option<SharedOctree>::None` is guaranteed to be represented as all-zero bytes.
#[repr(C)]
pub struct SharedOctree {
    // INVARIANT: `ptr` may not be constructed from a pointer with provenance at compile-time.

    // INVARIANT: If the address of `ptr` is a multiple of two, `ptr` must be a valid pointer
    // created by transmuting `ThinArc<(), Octnode>`. The length of the corresponding `[Octnode]`
    // slice must fit in a `NonZero<u16>` and must be at least 2 nodes.

    ptr: ptr::NonNull<()>,
    pad: [u8; {
        5usize
            .next_multiple_of(mem::align_of::<ptr::NonNull<()>>())
            .saturating_sub(mem::size_of::<ptr::NonNull<()>>())
    }],
}

impl SharedOctree {
    /// An octree with a single empty root node.
    pub const EMPTY: Self = SharedOctree {
        ptr: ptr::NonNull::without_provenance(NonZero::<usize>::MIN),
        pad: [0; _],
    };

    /// Copy an octree into a shared buffer.
    pub fn new(octree: &Octree) -> Self {
        if octree.len().get() == 1 {
            SharedOctree::new_inline([*octree.root()])
        } else {
            SharedOctree::new_heap(ThinArc::from_header_and_slice((), octree.nodes()))
        }
    }

    /// Construct an octree from a single root node.
    ///
    /// This can be used at compile-time. The root node is stored inline.
    pub const fn from_root(root: Octnode) -> Self { SharedOctree::new_inline([root]) }

    /// Get a reference to the underlying octree.
    ///
    /// This type already [`Deref`](ops::Deref)-coerces to [`Octree`]. Unlike deref coercion, this
    /// method can be used at compile-time.
    pub const fn as_octree(&self) -> &Octree {
        match self.unpack_ref() {
            SharedOctreeRef::Inline(nodes) => Octree::new_array(nodes),
            SharedOctreeRef::Heap(nodes) => {
                let nodes = const_deref_thin_arc(nodes);

                // SAFETY: The length of `nodes` fits in a `NonZero<u16>` per invariants.
                unsafe { Octree::new_unchecked(nodes) }
            },
        }
    }

    /// The reference count of the underlying shared octree buffer.
    ///
    /// If this octree is stored inline instead of in a reference-counted buffer, this returns zero.
    pub fn ref_count(&self) -> usize {
        match self.unpack_ref() {
            SharedOctreeRef::Inline(_) => 0,
            SharedOctreeRef::Heap(arc) => ThinArc::strong_count(arc),
        }
    }

    /// Compare reference equality of two shared octrees.
    ///
    /// Returns if the two octrees refer to the same data. Octrees that point to the same shared
    /// heap buffer are reference-equal. Inline octrees that contain the same value are also
    /// reference-equal.
    pub fn ref_eq(&self, other: &Self) -> bool {
        match (self.unpack_ref(), other.unpack_ref()) {
            (SharedOctreeRef::Inline(lhs), SharedOctreeRef::Inline(rhs))
                => lhs == rhs,

            (SharedOctreeRef::Heap(lhs), SharedOctreeRef::Heap(rhs))
                => lhs.heap_ptr().addr() == rhs.heap_ptr().addr(),

            _ => false,
        }
    }

    #[cfg(feature = "editor")]
    pub(crate) fn heap_space(&self) -> usize {
        let len = self.len().get() as usize;
        if len > 1 {
            len * mem::size_of::<Octnode>() + const { mem::size_of::<usize>() * 2 }
        } else {
            0
        }
    }

    const fn new_inline(nodes: [Octnode; 1]) -> Self {
        let mut octree = SharedOctree::EMPTY;

        // SAFETY: `SharedOctree` is always at least 5 bytes in size, which is 1 byte larger than
        // `Octnode`. All bytes of `SharedOctree` are always initialized. `Octnode` is
        // `bytemuck::Pod`. The first byte of `SharedOctree::EMPTY` is non-zero and is not mutated
        // by this operation, so `octree.ptr` does not become zero.
        unsafe {
            let inline = &mut *(&raw mut octree).cast::<[Octnode; 1]>().byte_add(1);
            *inline = nodes;
        }

        octree
    }

    fn new_heap(nodes: ThinArc<(), Octnode>) -> Self {
        // SAFETY: `ThinArc` is currently implemented as a `repr(transparent)` wrapper around
        // `ptr::NonNull`. We depend on an exact version of `triomphe` to prevent this from
        // changing.
        let ptr = unsafe { mem::transmute::<ThinArc<(), Octnode>, ptr::NonNull<()>>(nodes) };

        debug_assert!(ptr.addr().get().is_multiple_of(2));
        SharedOctree { pad: [0; _], ptr }
    }

    const fn is_inline(&self) -> bool {
        // SAFETY: If this function is running at compile-time, per invariants, it is not possible
        // for `self.ptr` to be a pointer with provenance; it must have been constructed from an
        // integer.
        let addr = unsafe { mem::transmute::<ptr::NonNull<()>, usize>(self.ptr) };

        !addr.is_multiple_of(2)
    }

    const fn unpack_ref(&self) -> SharedOctreeRef<'_> {
        if self.is_inline() {
            // SAFETY: `SharedOctree` is always at least 5 bytes in size, which is 1 byte larger
            // than `Octnode`. All bytes of `SharedOctree` aside from trailing padding are always
            // initialized. `Octnode` is `bytemuck::Pod`.
            let nodes = unsafe {
                &*(self as *const SharedOctree).cast::<[Octnode; 1]>().byte_add(1)
            };
            SharedOctreeRef::Inline(nodes)
        } else {
            // SAFETY: `ThinArc` is currently implemented as a `repr(transparent)` wrapper around
            // `ptr::NonNull`. We depend on an exact version of `triomphe` to prevent this from
            // changing.
            let arc = unsafe { &*(&raw const self.ptr).cast::<ThinArc<(), Octnode>>() };
            SharedOctreeRef::Heap(arc)
        }
    }
}

enum SharedOctreeRef<'a> {
    Inline(&'a [Octnode; 1]),
    Heap(&'a ThinArc<(), Octnode>),
}

impl Drop for SharedOctree {
    fn drop(&mut self) {
        if !self.is_inline() {
            // SAFETY: `ThinArc` is currently implemented as a `repr(transparent)` wrapper around
            // `ptr::NonNull`. We depend on an exact version of `triomphe` to prevent this from
            // changing.
            let arc = unsafe { &mut *(&raw mut self.ptr).cast::<ThinArc<(), Octnode>>() };

            // SAFETY: `Drop::drop` is only ever called once.
            unsafe { ptr::drop_in_place(arc) };
        }
    }
}

impl ops::Deref for SharedOctree {
    type Target = Octree;
    fn deref(&self) -> &Octree { self.as_octree() }
}

impl fmt::Debug for SharedOctree {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("SharedOctree").field(&self.as_octree()).finish()
    }
}

impl Clone for SharedOctree {
    fn clone(&self) -> Self {
        match self.unpack_ref() {
            SharedOctreeRef::Inline(nodes) => SharedOctree::new_inline(*nodes),
            SharedOctreeRef::Heap(nodes) => SharedOctree::new_heap(nodes.clone()),
        }
    }
}

impl Default for SharedOctree {
    fn default() -> Self { SharedOctree::EMPTY }
}

// SAFETY: `SharedOctree` contains only inline data or a pointer to a `ThinArc<(), Octnode>`, which
// is `Send`.
unsafe impl Send for SharedOctree { }

// SAFETY: `SharedOctree` contains only inline data or a pointer to a `ThinArc<(), Octnode>`, which
// is `Sync`.
unsafe impl Sync for SharedOctree { }

// SAFETY: `Option<SharedOctree>::None` is `0` per constant assertion.
unsafe impl bytemuck::ZeroableInOption for SharedOctree { }

const _: () = {
    if mem::size_of::<ThinArc<(), Octnode>>() != mem::size_of::<ptr::NonNull<()>>() {
        panic!("thin arc not ptr sized")
    }

    if mem::align_of::<ThinArc<(), Octnode>>() != mem::align_of::<ptr::NonNull<()>>() {
        panic!("thin arc not ptr aligned")
    }

    if mem::size_of::<Option<ThinArc<(), Octnode>>>() != mem::size_of::<ThinArc<(), Octnode>>() {
        panic!("thin arc has no niche")
    }

    // SAFETY: `ThinArc` is currently implemented as a `repr(transparent)` wrapper around
    // `ptr::NonNull`, meaning all of the bytes of `Option<ThinArc>::None` are initialized. We
    // depend on an exact version of `triomphe` to prevent this from changing.
    let niche = unsafe { mem::transmute::<Option<ThinArc<(), Octnode>>, *const ()>(None) };
    if !niche.is_null() {
        panic!("thin arc niche is not at zero")
    }
};

const _: () = {
    if mem::size_of::<SharedOctree>() < 5 {
        panic!("shared octree too small")
    }

    if mem::size_of::<Option<SharedOctree>>() != mem::size_of::<SharedOctree>() {
        panic!("shared octree has no niche")
    }

    let none = Option::<SharedOctree>::None;

    // SAFETY: `Option<SharedOctree>` is the same size as `SharedOctree`, so we know the `Option`'s
    // discriminant is stored in a niche of `SharedOctree` somehow. The only niche `SharedOctree`
    // has is its `ptr` field, and `SharedOctree` contains no padding bytes, so the compiler could
    // reasonably only store `Option`'s discriminant in `ptr`. We use this to assume that `ptr` is
    // initialized. If the compiler somehow chose another way to store `Option<SharedOctree>`'s
    // discriminant without failing our previous assertions, this would be undefined behavior.
    let niche = unsafe {
        *(&raw const none).byte_add(mem::offset_of!(SharedOctree, ptr)).cast::<*const ()>()
    };

    if !niche.is_null() {
        panic!("shared octree niche is not at zero")
    }
    mem::forget(none);
};

const fn const_deref_thin_arc(arc: &ThinArc<(), Octnode>) -> &[Octnode] {
    // SAFETY: The value from `arc` is unsafely copied and immediately turned back into a pointer.
    // The current version of `triomphe::Arc::from_thin` does not panic under these circumstances,
    // so `arc` will not be dropped twice. We depend on an exact version of `triomphe` to prevent
    // this from changing.
    let data = unsafe { &*triomphe::Arc::into_raw(triomphe::Arc::from_thin(ptr::read(arc))) };
    &data.slice
}

#[cfg(feature = "ffi")]
impl crate::ffi::SharedOctreeExt for SharedOctree {
    fn into_ptr(self) -> crate::ffi::SharedOctreePtr {
        if !self.is_inline() {
            // SAFETY: `ThinArc` is currently implemented as a `repr(transparent)` wrapper around
            // `ptr::NonNull`. We depend on an exact version of `triomphe` to prevent this from
            // changing.
            let arc = unsafe { mem::transmute::<ptr::NonNull<()>, ThinArc<(), Octnode>>(self.ptr) };
            let ptr = triomphe::Arc::into_raw(triomphe::Arc::from_thin(arc));

            // SAFETY: The pointer returned by `into_raw` includes the length header, so we skip
            // past that to get the pointer to the actual octree.
            let ptr = unsafe { ptr.byte_add(mem::size_of::<usize>()) as *const [Octnode] };
            let len = (ptr.len() * 4) as i32;
            let ptr = ptr.cast::<u8>();

            mem::forget(self);
            return crate::ffi::SharedOctreePtr { ptr, len }
        }

        // SAFETY: `SharedOctree` is always at least 5 bytes in size, which is 1 byte larger than
        // `Octnode`. All bytes of `SharedOctree` aside from trailing padding are always
        // initialized. `Octnode` is `bytemuck::Pod`.
        let [root] = unsafe { *(&raw const self).cast::<[Octnode; 1]>().byte_add(1) };

        static ROOT_NODES: [[Octnode; 1]; 1 << 16] = seq_macro::seq!(N in 0x0000..=0xFFFF {
            [#({
                let [mat, dist] = (N as u16).to_le_bytes();
                [Octnode { mat, dist, child: 0 }]
            },)*]
        });

        let idx = u16::from_le_bytes([root.mat, root.dist]) as usize;
        let root: &'static Octree = Octree::new(&ROOT_NODES[idx]);

        let ptr = (root as *const Octree).cast::<u8>();
        let len = 4;
        crate::ffi::SharedOctreePtr { ptr, len }
    }

    unsafe fn from_ptr(ptr: crate::ffi::SharedOctreePtr) -> Self {
        let ptr = ptr::slice_from_raw_parts(ptr.ptr.cast::<Octnode>(), ptr.len as usize / 4);
        let ptr = ptr as *const Octree;

        // SAFETY: `ptr` was returned from a previous call to `into_ptr` per safety contract, so
        // it's valid for reads right now and points to a valid octree.
        let octree = unsafe { &*ptr };
        if let [root] = octree.nodes() {
            SharedOctree::from_root(*root)
        } else {
            type ArcTarget = triomphe::HeaderSlice<triomphe::HeaderWithLength<()>, [Octnode]>;
            let ptr = ptr as *const ArcTarget;

            // SAFETY: The pointer needed by `from_raw` requires the length header, so we move the
            // pointer back to get it.
            let ptr = unsafe { ptr.byte_sub(mem::size_of::<usize>()) };

            // SAFETY: `ptr` was produced by `triomphe::Arc::into_raw`.
            let arc = unsafe { triomphe::Arc::into_thin(triomphe::Arc::from_raw(ptr)) };

            SharedOctree::new_heap(arc)
        }
    }
}

/// A node in a terrain octree in the game's data format.
///
/// The fields may be invalid. It is legal for any of the functions in this crate to interpret
/// invalid values however best fits the circumstances, including by aliasing them to valid values
/// and even in situations where you may not expect it, except in circumstances that specifically
/// document otherwise.
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
    /// indices that contain one of this node's ancestors, are invalid.
    ///
    /// If the signed distance of this node is zero, the node should not have children.
    pub child: u16,
}

impl Octnode {
    /// An empty, zeroed leaf node.
    pub const EMPTY: Self = Octnode { mat: 0, dist: 0, child: 0 };
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_octree() {
        let bytes = &[
            0x09, 0x00,
            0x04, 0x00, 0x01, 0x00,
            0x04, 0x00, 0x02, 0x00,
            0x03, 0x00, 0x03, 0x00,
            0x04, 0x00, 0x04, 0x00,
            0x03, 0x00, 0x05, 0x00,
            0x04, 0x00, 0x06, 0x00,
            0x02, 0x00, 0x07, 0x00,
            0x04, 0x00, 0x08, 0x00,
            0x02, 0x00, 0x09, 0x00,

            0x00, 0x00,

            0xFF, 0xFF,
            0x00, 0x00, 0xFF, 0xFF,

            0xCC,
        ];

        let (octree, bytes) = Octree::parse(bytes);
        let octree = octree.expect("should be an octree");

        assert_eq!(octree.len().get(), 9);
        assert_eq!(octree.root(), &Octnode { mat: 4, dist: 0, child: 1 });
        assert_eq!(octree.node_children(octree.root()), Some(&[
            Octnode { mat: 4, dist: 0, child: 2 },
            Octnode { mat: 3, dist: 0, child: 3 },
            Octnode { mat: 4, dist: 0, child: 4 },
            Octnode { mat: 3, dist: 0, child: 5 },
            Octnode { mat: 4, dist: 0, child: 6 },
            Octnode { mat: 2, dist: 0, child: 7 },
            Octnode { mat: 4, dist: 0, child: 8 },
            Octnode { mat: 2, dist: 0, child: 9 },
        ]));

        let (octree, bytes) = Octree::parse(bytes);
        let octree = octree.expect("should be an octree");

        assert_eq!(octree.len().get(), 1);
        assert_eq!(octree.root(), &Octnode::EMPTY);

        let (octree, bytes) = Octree::parse(bytes);
        let octree = octree.expect("should be an octree");

        assert_eq!(octree.len().get(), 1);
        assert_eq!(octree.root(), &Octnode { mat: 0, dist: 0, child: u16::MAX });

        assert_eq!(bytes, &[0xCC]);
        let (octree, bytes) = Octree::parse(bytes);
        assert!(octree.is_none());
        assert_eq!(bytes, &[]);
    }

    #[test]
    fn shared_octree() {
        let nodes = &[
            Octnode { mat: 1, dist: 126, child: 1 },
            Octnode { mat: 1, dist: 126, child: 1000 },
            Octnode { mat: 2, dist: 126, child: 1000 },
            Octnode { mat: 3, dist: 126, child: 1000 },
            Octnode { mat: 4, dist: 126, child: 1000 },
            Octnode { mat: 5, dist: 126, child: 1000 },
            Octnode { mat: 6, dist: 126, child: 1000 },
            Octnode { mat: 7, dist: 126, child: 1000 },
            Octnode { mat: 8, dist: 126, child: 1000 },
        ];

        let octree1 = SharedOctree::new(Octree::new(nodes));
        assert_eq!(octree1.ref_count(), 1);

        let octree2 = octree1.clone();
        assert_eq!(octree2.ref_count(), 2);

        assert_eq!(octree1.nodes(), octree2.nodes());
        assert_eq!(octree1.nodes(), nodes);
        assert!(octree1.ref_eq(&octree2));

        drop(octree1);
        assert_eq!(octree2.ref_count(), 1);

        let inline = SharedOctree::new(Octree::new_array(&[
            Octnode { mat: 255, dist: 255, child: 65_535 },
        ]));
        assert_eq!(inline.ref_count(), 0);
        assert_eq!(inline.nodes(), [Octnode { mat: 255, dist: 255, child: 65_535 }]);
    }

    #[cfg(feature = "ffi")]
    #[test]
    fn shared_octree_ptr() {
        use crate::ffi::SharedOctreeExt;
        let nodes = &[
            Octnode { mat: 1, dist: 126, child: 1 },
            Octnode { mat: 1, dist: 126, child: 1000 },
            Octnode { mat: 2, dist: 126, child: 1000 },
            Octnode { mat: 3, dist: 126, child: 1000 },
            Octnode { mat: 4, dist: 126, child: 1000 },
            Octnode { mat: 5, dist: 126, child: 1000 },
            Octnode { mat: 6, dist: 126, child: 1000 },
            Octnode { mat: 7, dist: 126, child: 1000 },
            Octnode { mat: 8, dist: 126, child: 1000 },
        ];

        let octree1 = SharedOctree::new(Octree::new(nodes));
        assert_eq!(octree1.ref_count(), 1);

        let octree2 = octree1.clone();
        assert_eq!(octree2.ref_count(), 2);

        let octree1 = octree1.into_ptr();
        assert_eq!(octree2.ref_count(), 2);

        // SAFETY: `octree1` is a valid `SharedOctree` returned by `into_ptr` and its length is
        // guaranteed to be at least one node.
        assert_eq!(
            unsafe { *octree1.ptr.cast::<Octnode>() },
            Octnode { mat: 1, dist: 126, child: 1 },
        );

        // SAFETY: `octree1` came directory from `into_ptr`.
        let octree1 = unsafe { SharedOctree::from_ptr(octree1) };
        assert!(octree1.ref_eq(&octree2));
        assert_eq!(octree1.nodes(), nodes);

        assert_eq!(octree2.ref_count(), 2);
        drop(octree1);
        assert_eq!(octree2.ref_count(), 1);

        let inline = SharedOctree::new(Octree::new_array(&[
            Octnode { mat: 255, dist: 255, child: 65_535 },
        ]));
        assert_eq!(inline.ref_count(), 0);

        // SAFETY: The result of `into_ptr` is directly passed back to `from_ptr`.
        let inline = unsafe { SharedOctree::from_ptr(inline.into_ptr()) };
        assert_eq!(inline.nodes(), [Octnode { mat: 255, dist: 255, child: 0 }]);
    }
}
