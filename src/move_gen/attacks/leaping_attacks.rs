//! Gets Knight, Pawn and King attacks.

use super::Bitboard;
use crate::chess::Direction;

pub static KNIGHT_ATTACKS: [Bitboard; 64] = get_knight_attacks();
pub static KING_ATTACKS: [Bitboard; 64] = get_king_attacks();
pub static PAWN_ATTACKS: [[Bitboard; 64]; 2] = get_pawn_attacks();

const fn get_knight_attacks() -> [Bitboard; 64] {
let mut table = [Bitboard::new(0); 64];
    let mut sq = 0;
    while sq < 64 {
        let b = Bitboard::new(1u64 << sq);

        let nne = b.shift(Direction::North).shift(Direction::NorthEast);
        let nnw = b.shift(Direction::North).shift(Direction::NorthWest);
        let sse = b.shift(Direction::South).shift(Direction::SouthEast);
        let ssw = b.shift(Direction::South).shift(Direction::SouthWest);
        let een = b.shift(Direction::East).shift(Direction::NorthEast);
        let ees = b.shift(Direction::East).shift(Direction::SouthEast);
        let wwn = b.shift(Direction::West).shift(Direction::NorthWest);
        let wws = b.shift(Direction::West).shift(Direction::SouthWest);

        table[sq] = Bitboard::new(
            nne.as_u64() | nnw.as_u64() | sse.as_u64() | ssw.as_u64() |
            een.as_u64() | ees.as_u64() | wwn.as_u64() | wws.as_u64()
        );
        
        sq += 1;
    }
    table
}

const fn get_king_attacks() -> [Bitboard; 64] {
    let mut table = [Bitboard::new(0); 64];
    let mut sq = 0;
    while sq < 64 {
        let b = Bitboard::new(1u64 << sq);
        
        let attacks = b.shift(Direction::North).as_u64()
                    | b.shift(Direction::South).as_u64()
                    | b.shift(Direction::East).as_u64()
                    | b.shift(Direction::West).as_u64()
                    | b.shift(Direction::NorthEast).as_u64()
                    | b.shift(Direction::NorthWest).as_u64()
                    | b.shift(Direction::SouthEast).as_u64()
                    | b.shift(Direction::SouthWest).as_u64();

        table[sq] = Bitboard::new(attacks);
        sq += 1;
    }
    table
}

const fn get_pawn_attacks() -> [[Bitboard; 64]; 2] {
    let mut table = [[Bitboard::new(0); 64]; 2];
    let mut sq = 0;
    while sq < 64 {
        let b = Bitboard::new(1u64 << sq);

        let white_attacks = b.shift(Direction::NorthWest).as_u64()
                          | b.shift(Direction::NorthEast).as_u64();
        table[0][sq] = Bitboard::new(white_attacks);

        let black_attacks = b.shift(Direction::SouthWest).as_u64()
                          | b.shift(Direction::SouthEast).as_u64();
        table[1][sq] = Bitboard::new(black_attacks);

        sq += 1;
    }
    table
}
