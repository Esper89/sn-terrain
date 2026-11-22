# Signed Distance Invariant

In some contexts, the voxels in a grid, or in an idealized grid stored as an octree, may be expected
to conform to a spatial invariant under which the signed distances of those voxels are constrained
to values that are consistent with one another. Many functions in this crate may not behave in a way
that is fully correct if this invariant does not hold; some of them may document a stronger
requirement.

This invariant is independent of the validity of the individual voxels. Broadly, the requirement is
that it must be possible for a hypothetical isosurface to produce the signed distance field in
question. The precise requirement is weaker, and is as follows:

> Any straight line that begins at a fully-solid voxel and ends at a fully-empty voxel must pass
> through two surface voxels between. The first surface voxel it passes through must have a positive
> signed distance value, and the second surface voxel it passes through must have a nonpositive
> signed distance value.

A fully-solid voxel is a voxel with a signed distance greater than +1.0, and a full-empty voxel is a
voxel with a signed distance less than or equal to −1.0; thus this invariant is upheld by any
correctly-implemented signed distance function representing a physically possible isosurface.
Depending on the circumstances, this invariant may be expected to hold across octree boundaries in
addition to within each octree.

Note that this invariant holds based on if the signed distance values of the voxels are positive or
nonpositive, but the game uses slightly different criteria to determine if a voxel has terrain or
not; voxels with nonnegative signed distance values have terrain, while voxels with negative signed
distance values do not have terrain.
