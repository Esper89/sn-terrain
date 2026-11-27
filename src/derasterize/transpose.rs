use crate::utils::simd::*;

// Algorithm for transposing a W × N matrix of bytes into a N × W matrix of bytes, where W is the
// current SIMD level's native vector width and N is either 16 or 8.

#[expect(nonstandard_style)]
#[simd]
#[inline(always)]
pub fn transpose_u8_Wx16<S: SimdExt>(simd: S, matrix: [S::u8s; 16]) -> [S::u8s; 16] {
    let matrix = simd.downcast_u8s_arr(
        matrix, gather_blocks_u8_16x16, gather_blocks_u8_32x16, gather_blocks_u8_64x16,
    );

    transpose_blocks_u8_x16(simd, matrix)
}

#[simd]
#[inline(always)]
fn transpose_blocks_u8_x16<S: SimdExt>(simd: S, matrix: [S::u8s; 16]) -> [S::u8s; 16] {
    #[inline(always)]
    fn step<S: SimdExt>(simd: S, matrix: [S::u8s; 16]) -> [S::u8s; 16] {
        let block_zip_lo = { #[inline(always)] |a, b| simd.zip_low_within_blocks_u8s(a, b) };
        let block_zip_hi = { #[inline(always)] |a, b| simd.zip_high_within_blocks_u8s(a, b) };

        [
            block_zip_lo(matrix[0], matrix[8]), block_zip_hi(matrix[0], matrix[8]),
            block_zip_lo(matrix[1], matrix[9]), block_zip_hi(matrix[1], matrix[9]),
            block_zip_lo(matrix[2], matrix[10]), block_zip_hi(matrix[2], matrix[10]),
            block_zip_lo(matrix[3], matrix[11]), block_zip_hi(matrix[3], matrix[11]),
            block_zip_lo(matrix[4], matrix[12]), block_zip_hi(matrix[4], matrix[12]),
            block_zip_lo(matrix[5], matrix[13]), block_zip_hi(matrix[5], matrix[13]),
            block_zip_lo(matrix[6], matrix[14]), block_zip_hi(matrix[6], matrix[14]),
            block_zip_lo(matrix[7], matrix[15]), block_zip_hi(matrix[7], matrix[15]),
        ]
    }

    step(simd, step(simd, step(simd, step(simd, matrix))))
}

#[simd]
#[inline(always)]
fn gather_blocks_u8_16x16<S: SimdExt>(simd: S, matrix: [u8x16<S>; 16]) -> [u8x16<S>; 16] {
    matrix
}

#[simd]
#[inline(always)]
fn gather_blocks_u8_32x16<S: SimdExt>(simd: S, matrix: [u8x32<S>; 16]) -> [u8x32<S>; 16] {
    let matrix: [u8x16<S>; 32] = cast_simd_arr(matrix);
    let matrix = [
        matrix[0], matrix[16],
        matrix[1], matrix[17],
        matrix[2], matrix[18],
        matrix[3], matrix[19],
        matrix[4], matrix[20],
        matrix[5], matrix[21],
        matrix[6], matrix[22],
        matrix[7], matrix[23],
        matrix[8], matrix[24],
        matrix[9], matrix[25],
        matrix[10], matrix[26],
        matrix[11], matrix[27],
        matrix[12], matrix[28],
        matrix[13], matrix[29],
        matrix[14], matrix[30],
        matrix[15], matrix[31],
    ];
    cast_simd_arr(matrix)
}

#[simd]
#[inline(always)]
fn gather_blocks_u8_64x16<S: SimdExt>(simd: S, matrix: [u8x64<S>; 16]) -> [u8x64<S>; 16] {
    let matrix: [u8x16<S>; 64] = cast_simd_arr(matrix);
    let matrix = [
        matrix[0], matrix[16], matrix[32], matrix[48],
        matrix[1], matrix[17], matrix[33], matrix[49],
        matrix[2], matrix[18], matrix[34], matrix[50],
        matrix[3], matrix[19], matrix[35], matrix[51],
        matrix[4], matrix[20], matrix[36], matrix[52],
        matrix[5], matrix[21], matrix[37], matrix[53],
        matrix[6], matrix[22], matrix[38], matrix[54],
        matrix[7], matrix[23], matrix[39], matrix[55],
        matrix[8], matrix[24], matrix[40], matrix[56],
        matrix[9], matrix[25], matrix[41], matrix[57],
        matrix[10], matrix[26], matrix[42], matrix[58],
        matrix[11], matrix[27], matrix[43], matrix[59],
        matrix[12], matrix[28], matrix[44], matrix[60],
        matrix[13], matrix[29], matrix[45], matrix[61],
        matrix[14], matrix[30], matrix[46], matrix[62],
        matrix[15], matrix[31], matrix[47], matrix[63],
    ];
    cast_simd_arr(matrix)
}

#[expect(nonstandard_style)]
#[simd]
#[inline(always)]
pub fn transpose_u8_Wx8<S: SimdExt>(simd: S, matrix: [S::u8s; 8]) -> [S::u8s; 8] {
    let matrix = simd.downcast_u8s_arr(
        matrix, gather_blocks_u8_16x8, gather_blocks_u8_32x8, gather_blocks_u8_64x8,
    );

    transpose_blocks_u8_x8(simd, matrix)
}

#[simd]
#[inline(always)]
fn transpose_blocks_u8_x8<S: SimdExt>(simd: S, matrix: [S::u8s; 8]) -> [S::u8s; 8] {
    #[inline(always)]
    fn step<S: SimdExt>(simd: S, matrix: [S::u8s; 8]) -> [S::u8s; 8] {
        let block_zip_lo = { #[inline(always)] |a, b| simd.zip_low_within_blocks_u8s(a, b) };
        let block_zip_hi = { #[inline(always)] |a, b| simd.zip_high_within_blocks_u8s(a, b) };

        [
            block_zip_lo(matrix[0], matrix[4]), block_zip_hi(matrix[0], matrix[4]),
            block_zip_lo(matrix[1], matrix[5]), block_zip_hi(matrix[1], matrix[5]),
            block_zip_lo(matrix[2], matrix[6]), block_zip_hi(matrix[2], matrix[6]),
            block_zip_lo(matrix[3], matrix[7]), block_zip_hi(matrix[3], matrix[7]),
        ]
    }

    step(simd, step(simd, step(simd, step(simd, matrix))))
}

#[simd]
#[inline(always)]
fn gather_blocks_u8_16x8<S: SimdExt>(simd: S, matrix: [u8x16<S>; 8]) -> [u8x16<S>; 8] {
    matrix
}

#[simd]
#[inline(always)]
fn gather_blocks_u8_32x8<S: SimdExt>(simd: S, matrix: [u8x32<S>; 8]) -> [u8x32<S>; 8] {
    let matrix: [u8x16<S>; 16] = cast_simd_arr(matrix);
    let matrix = [
        matrix[0], matrix[8],
        matrix[1], matrix[9],
        matrix[2], matrix[10],
        matrix[3], matrix[11],
        matrix[4], matrix[12],
        matrix[5], matrix[13],
        matrix[6], matrix[14],
        matrix[7], matrix[15],
    ];
    cast_simd_arr(matrix)
}

#[simd]
#[inline(always)]
fn gather_blocks_u8_64x8<S: SimdExt>(simd: S, matrix: [u8x64<S>; 8]) -> [u8x64<S>; 8] {
    let matrix: [u8x16<S>; 32] = cast_simd_arr(matrix);
    let matrix = [
        matrix[0], matrix[8], matrix[16], matrix[24],
        matrix[1], matrix[9], matrix[17], matrix[25],
        matrix[2], matrix[10], matrix[18], matrix[26],
        matrix[3], matrix[11], matrix[19], matrix[27],
        matrix[4], matrix[12], matrix[20], matrix[28],
        matrix[5], matrix[13], matrix[21], matrix[29],
        matrix[6], matrix[14], matrix[22], matrix[30],
        matrix[7], matrix[15], matrix[23], matrix[31],
    ];
    cast_simd_arr(matrix)
}

#[cfg(test)]
mod test {
    use super::*;
    use std::array;
    use rand::{prelude::*, rngs};
    use crate::utils::test_all_cpu_levels;

    #[expect(nonstandard_style)]
    #[test]
    fn transpose_u8_Wx16() {
        test_all_cpu_levels!(|cpu| {
            let simd = cpu.simd();
            for seed in 227..291 {
                let input = random_matrix(simd, seed);
                let matrix = super::transpose_u8_Wx16(simd, input);
                let expect = naive_transpose(simd, input);
                assert!(simd_arr_eq(&matrix, &expect), "{cpu:?}");
            }
        });
    }

    #[expect(nonstandard_style)]
    #[test]
    fn transpose_u8_Wx8() {
        test_all_cpu_levels!(|cpu| {
            let simd = cpu.simd();
            for seed in 227..291 {
                let input = random_matrix(simd, seed);
                let matrix = super::transpose_u8_Wx8(simd, input);
                let expect = naive_transpose(simd, input);
                assert!(simd_arr_eq(&matrix, &expect), "{cpu:?}");
            }
        });
    }

    fn random_matrix<S: SimdExt, const C: usize, T>(simd: S, seed: u64) -> [T; C]
    where T: SimdBase<S, Element = u8> {
        let mut rng = rngs::Xoshiro256PlusPlus::seed_from_u64(seed);
        array::from_fn(|_| T::from_fn(simd, |_| rng.random()))
    }

    fn naive_transpose<S: SimdExt, const C: usize, T>(simd: S, matrix: [T; C]) -> [T; C]
    where T: SimdBase<S, Element = u8> {
        fn width<S: SimdExt>(_: S) -> usize { S::u8s::LEN }
        let rows = width(simd);

        let mut out = array::from_fn(|_| T::splat(simd, 0));
        for row in 0..rows {
            for col in 0..C {
                let idx = row * C + col;
                out[col][row] = matrix[idx / rows][idx % rows];
            }
        }
        out
    }
}
