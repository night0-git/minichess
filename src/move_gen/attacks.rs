//! This module provides methods to find pieces' attacks.
//! Look-up tables are used for leaping pieces (Knight, Pawn, King).
//! Magic bitboards are used for sliding pieces (Bishop, Rook, Queen).
//! <https://www.chessprogramming.org/Magic_Bitboards>

mod leaping_attacks;
mod sliding_attacks;

use crate::chess::piece::Color;
use crate::chess::{Bitboard, Square};
use leaping_attacks::{KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS};

#[inline(always)]
pub fn knight_attacks(sq: Square) -> Bitboard {
    KNIGHT_ATTACKS[sq.as_usize()]
}

#[inline(always)]
pub fn king_attacks(sq: Square) -> Bitboard {
    KING_ATTACKS[sq.as_usize()]
}

#[inline(always)]
pub fn pawn_attacks(sq: Square, attacker_col: Color) -> Bitboard {
    PAWN_ATTACKS[attacker_col as usize][sq.as_usize()]
}

#[inline(always)]
pub fn bishop_attacks(sq: Square, blockers: Bitboard) -> Bitboard {
    sliding_attacks::get_bishop_attacks(sq, blockers)
}

#[inline(always)]
pub fn rook_attacks(sq: Square, blockers: Bitboard) -> Bitboard {
    sliding_attacks::get_rook_attacks(sq, blockers)
}

#[inline(always)]
pub fn queen_attacks(sq: Square, blockers: Bitboard) -> Bitboard {
    bishop_attacks(sq, blockers) | rook_attacks(sq, blockers)
}

pub static RAY_BETWEEN: [[Bitboard; 64]; 64] =
    include!(concat!(env!("OUT_DIR"), "/ray_between.rs"));
pub static LINE_THROUGH: [[Bitboard; 64]; 64] =
    include!(concat!(env!("OUT_DIR"), "/line_through.rs"));

/// Returns a bitboard representing the squares between `sq1` and `sq2`,
/// excluding `sq1` and `sq2`.
/// If the squares are not orthogonally or diagonally aligned, or are
/// adjacent, returns an empty bitboard.
#[inline(always)]
pub fn ray_between(sq1: Square, sq2: Square) -> Bitboard {
    RAY_BETWEEN[sq1.as_usize()][sq2.as_usize()]
}

/// Returns a bitboard representing the entire line intersecting `sq1` and `sq2`.
/// If the squares are not orthogonally or diagonally aligned, returns an empty bitboard.
#[inline(always)]
pub fn line_through(sq1: Square, sq2: Square) -> Bitboard {
    LINE_THROUGH[sq1.as_usize()][sq2.as_usize()]
}
