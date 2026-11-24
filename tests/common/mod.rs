#![allow(dead_code)]

use std::collections::HashSet;
use std::str::FromStr;

use minichess::chess::piece::Color;
use minichess::chess::{Bitboard, Move, Piece, Square};
use minichess::position::Board;

/// Parses a UCI move string.
pub fn parse_mv(uci: &str) -> Move {
    Move::from_str(uci).expect("valid UCI move")
}

/// Converts a slice of moves into a set of UCI strings for order-independent comparison.
pub fn uci_set(moves: &[Move]) -> HashSet<String> {
    moves.iter().map(|m| m.to_string()).collect()
}

/// Returns `true` if `moves` contains any move matching `uci`.
pub fn has_uci(moves: &[Move], uci: &str) -> bool {
    moves.iter().any(|m| m.to_string() == uci)
}

/// Verifies that no generated move appears more than once.
pub fn assert_unique_moves(moves: &[Move]) {
    let mut seen = HashSet::with_capacity(moves.len());
    for mv in moves {
        assert!(
            seen.insert(mv.to_string()),
            "duplicate generated move: {mv}"
        );
    }
}

/// Verifies board representation consistency after state transitions.
pub fn assert_board_invariants(board: &Board) {
    let mut white_mask = Bitboard::EMPTY;
    let mut black_mask = Bitboard::EMPTY;
    let mut all_pieces = Bitboard::EMPTY;

    for piece in Piece::ALL {
        let bb = board.bitboard(piece);
        assert!(
            (all_pieces & bb).is_empty(),
            "overlapping piece bitboards for {piece} in {}",
            board.to_fen()
        );
        all_pieces |= bb;
        match piece.color() {
            Color::White => white_mask |= bb,
            Color::Black => black_mask |= bb,
        }
    }

    assert_eq!(board.us(Color::White), white_mask);
    assert_eq!(board.us(Color::Black), black_mask);
    assert_eq!(board.occupancy(), white_mask | black_mask);

    for sq_idx in 0..64u8 {
        let sq = Square::new(sq_idx);
        let from_bitboards = Piece::ALL
            .into_iter()
            .find(|&p| board.bitboard(p).contains(sq));
        assert_eq!(
            board.piece_at(sq),
            from_bitboards,
            "mailbox mismatch at {sq} in {}",
            board.to_fen()
        );
    }

    assert_eq!(
        board.state.zobrist_key,
        board.compute_zobrist_key(),
        "incremental zobrist key drifted in {}",
        board.to_fen()
    );
}
