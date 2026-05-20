//! Gets Bishop and Rook attacks.

use super::{Bitboard, Square};

#[derive(Debug, Clone, Copy)]
pub struct Magic {
    pub mask: u64,
    pub magic: u64,
    pub shift: u8,
    pub offset: usize,
}

pub static BISHOP_MAGICS: [Magic; 64] = include!(concat!(env!("OUT_DIR"), "/bishop_magics.rs"));
pub static ROOK_MAGICS: [Magic; 64] = include!(concat!(env!("OUT_DIR"), "/rook_magics.rs"));

pub static BISHOP_ATTACKS: [Bitboard; 5248] = include!(concat!(env!("OUT_DIR"), "/bishop_attacks.rs"));
pub static ROOK_ATTACKS: [Bitboard; 102400] = include!(concat!(env!("OUT_DIR"), "/rook_attacks.rs"));

#[inline(always)]
pub fn get_bishop_attacks(sq: Square, blockers: Bitboard) -> Bitboard {
    let magic = &BISHOP_MAGICS[sq.as_usize()];
    let occupancy = blockers.as_u64() & magic.mask;
    let hash = (occupancy.wrapping_mul(magic.magic)) >> magic.shift;

    BISHOP_ATTACKS[magic.offset + hash as usize]
}

#[inline(always)]
pub fn get_rook_attacks(sq: Square, blockers: Bitboard) -> Bitboard {
    let magic = &ROOK_MAGICS[sq.as_usize()];
    let occupancy = blockers.as_u64() & magic.mask;
    let hash = (occupancy.wrapping_mul(magic.magic)) >> magic.shift;

    ROOK_ATTACKS[magic.offset + hash as usize]
}
