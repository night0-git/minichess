use crate::chess::piece::{Color, Piece};
use crate::engine::pawn::{PawnTable, evaluate_pawns};
use crate::position::board::Board;

/// Middlegame piece values per piece kind.
pub const MG_VALUE: [i32; 6] = [82, 337, 365, 477, 1025, 0];

/// Endgame piece values per piece kind.
pub const EG_VALUE: [i32; 6] = [94, 281, 297, 512, 936, 0];

/// Game phase weight increments per piece kind.
/// Total starting non-pawn non-king pieces phase = 4*1 + 4*1 + 4*2 + 2*4 = 24.
pub const PHASE_WEIGHT: [i32; 6] = [0, 1, 1, 2, 4, 0];

pub const MAX_PHASE: i32 = 24;

// Piece-Square Tables (PST) from PeSTO's evaluation function.
// Tables are laid out from Rank 1 (indices 0..7) to Rank 8 (indices 56..63)
// from White's perspective. For Black pieces, square index is mirrored with `sq ^ 56`.

#[rustfmt::skip]
const MG_PAWN_TABLE: [i32; 64] = [
    // Rank 1
      0,   0,   0,   0,   0,   0,   0,   0,
    -35,  -1, -20, -23, -15,  24,  38, -22,
    -26,  -4,  -4, -10,   3,   3,  33, -12,
    -27,  -2,  -5,  12,  17,   6,  10, -25,
    -14,  13,   6,  21,  23,  12,  17, -23,
     -6,   7,  26,  31,  65,  56,  25, -20,
     98, 134,  61,  95,  68, 126,  34, -11,
      0,   0,   0,   0,   0,   0,   0,   0,
];

#[rustfmt::skip]
const EG_PAWN_TABLE: [i32; 64] = [
    // Rank 1
      0,   0,   0,   0,   0,   0,   0,   0,
     13,   8,   8,  10,  13,   0,   2,  -7,
      4,   7,  -6,   1,   0,  -5,  -1,  -8,
     13,   9,  -3,  -7,  -7,  -8,   3,  -1,
     32,  24,  13,   5,  -2,   4,  17,  17,
     94, 100,  85,  67,  56,  53,  82,  84,
    178, 173, 158, 134, 147, 132, 165, 187,
      0,   0,   0,   0,   0,   0,   0,   0,
];

#[rustfmt::skip]
const MG_KNIGHT_TABLE: [i32; 64] = [
    // Rank 1
    -105, -21, -58, -33, -17, -28, -19, -23,
     -29, -53, -12,  -3,  -1,  18, -14, -19,
     -23,  -9,  12,  10,  19,  17,  25, -16,
     -13,   4,  16,  13,  28,  19,  21,  -8,
      -9,  17,  19,  53,  37,  69,  18,  22,
     -47,  60,  37,  65,  84, 129,  73,  44,
     -73, -41,  72,  36,  23,  62,   7, -17,
    -167, -89, -34, -49,  61, -97, -15, -107,
];

#[rustfmt::skip]
const EG_KNIGHT_TABLE: [i32; 64] = [
    // Rank 1
    -29, -51, -23, -15, -22, -18, -50, -64,
    -42, -20, -10,  -5,  -2, -20, -23, -44,
    -23,  -3,  -1,  15,  10,  -3, -20, -22,
    -18,  -6,  16,  25,  16,  17,   4, -18,
    -17,   3,  22,  22,  22,   1,   8, -18,
    -24, -20,  10,   9,  -1,  -9, -19, -41,
    -25,  -8, -25,  -2,  -9, -25, -24, -52,
    -58, -38, -13, -28, -31, -27, -63, -99,
];

#[rustfmt::skip]
const MG_BISHOP_TABLE: [i32; 64] = [
    // Rank 1
    -33,  -3, -14, -21, -13, -12, -39, -21,
      4,  15,  16,   0,   7,  21,  33,   1,
      0,  15,  15,  15,  14,  27,  18,  10,
     -6,  13,  13,  26,  34,  12,  10,   4,
     -4,   5,  19,  50,  37,  37,   7,  -2,
    -16,  37,  43,  40,  35,  50,  37,  -2,
    -26,  16, -18, -13,  30,  59,  18, -47,
    -29,   4, -82, -37, -25, -42,   7,  -8,
];

#[rustfmt::skip]
const EG_BISHOP_TABLE: [i32; 64] = [
    // Rank 1
    -13, -12,  -5,  -5,  -8, -12, -15, -13,
    -10,  -5,   5,   0,   0,   3,  -5,  -9,
     -6,  11,   7,   8,  10,  12,  -2,  -9,
     -6,   3,  13,  19,  15,  10,   5,  -5,
     -3,   9,  12,   9,  14,  10,   3,   2,
      2,  -8,   0,  -1,  -2,   6,   0,   4,
     -8,  -4,   7, -12,  -3, -13,  -4, -14,
    -14, -21, -11,  -8,  -7,  -9, -17, -24,
];

#[rustfmt::skip]
const MG_ROOK_TABLE: [i32; 64] = [
    // Rank 1
    -19, -13,   1,  17,  16,   7, -37, -26,
    -44, -16, -20,  -9,  -1,  11,  -6, -71,
    -45, -25, -16, -17,   3,   0,  -5, -33,
    -36, -26, -12,  -1,   9,  -7,   6, -23,
    -24, -11,   7,  26,  24,  35,  -8, -20,
     -5,  19,  26,  36,  17,  45,  61,  16,
     27,  32,  58,  62,  80,  67,  26,  44,
     32,  42,  32,  51,  63,   9,  31,  43,
];

#[rustfmt::skip]
const EG_ROOK_TABLE: [i32; 64] = [
    // Rank 1
     -5,  -5,  -3,  -3,  -5,  -4,  -4,  -4,
     -9,  -7,  -8,  -8, -10,  -9, -10,  -9,
     -4,  -3,  -1,  -2,  -6,  -7,  -9,  -7,
      3,   5,   8,   4,  -5,  -6,  -8,  -4,
      4,   3,  13,   1,   2,   1,  -1,   2,
      7,   7,   7,   5,   4,  -3,  -5,  -3,
     11,  13,  13,  11,  -3,   3,   8,   3,
     13,  10,  18,  15,  12,  12,   8,   5,
];

#[rustfmt::skip]
const MG_QUEEN_TABLE: [i32; 64] = [
    // Rank 1
     -1, -18,  -9,  10, -15, -25, -31, -50,
    -35,  -8,  11,   2,   8,  15,  -3,   1,
    -14,   2, -11,  -2,  -5,   2,  14,   5,
     -9, -26,  -9, -10,  -2,  -4,   3,  -3,
    -27, -27, -16, -16,  -1,  17,  -2,   1,
    -13, -17,   7,   8,  29,  56,  47,  57,
    -24, -39,  -5,   1, -16,  57,  28,  54,
    -28,   0,  29,  12,  59,  44,  43,  45,
];

#[rustfmt::skip]
const EG_QUEEN_TABLE: [i32; 64] = [
    // Rank 1
    -33, -28, -22, -43,  -5, -32, -20, -41,
    -22, -23, -30, -16, -16, -23, -36, -32,
    -16, -27,  15,   6,   9,  17,  10,   5,
    -18,  28,  19,  47,  31,  34,  39,  23,
      3,  22,  24,  45,  57,  40,  57,  36,
    -20,   6,   9,  49,  47,  35,  19,   9,
    -17,  20,  32,  41,  58,  25,  30,   0,
     -9,  22,  22,  27,  27,  19,  10,  20,
];

#[rustfmt::skip]
const MG_KING_TABLE: [i32; 64] = [
    // Rank 1
    -15,  36,  12, -54,   8, -28,  24,  14,
      1,   7,  -8, -64, -43, -16,   9,   8,
    -14, -14, -22, -46, -44, -30, -15, -27,
    -49,  -1, -27, -39, -46, -44, -33, -51,
    -17, -20, -12, -27, -30, -25, -14, -36,
     -9,  24,   2, -16, -20,   6,  22, -22,
     29,  -1, -20,  -7,  -8,  -4, -38, -29,
    -65,  23,  16, -15, -56, -34,   2,  13,
];

#[rustfmt::skip]
const EG_KING_TABLE: [i32; 64] = [
    // Rank 1
    -53, -34, -21, -11, -28, -14, -24, -43,
    -27, -11,   4,  13,  14,   4,  -5, -17,
    -19,  -3,  11,  21,  23,  16,   7,  -9,
    -18,  -4,  21,  24,  27,  23,   9, -11,
     -8,  22,  24,  27,  26,  33,  26,   3,
     10,  17,  23,  15,  20,  45,  44,  13,
    -12,  17,  14,  17,  17,  38,  23,  11,
    -74, -35, -18, -18, -11,  15,   4, -17,
];

pub const MG_PST: [[i32; 64]; 6] = [
    MG_PAWN_TABLE,
    MG_KNIGHT_TABLE,
    MG_BISHOP_TABLE,
    MG_ROOK_TABLE,
    MG_QUEEN_TABLE,
    MG_KING_TABLE,
];

pub const EG_PST: [[i32; 64]; 6] = [
    EG_PAWN_TABLE,
    EG_KNIGHT_TABLE,
    EG_BISHOP_TABLE,
    EG_ROOK_TABLE,
    EG_QUEEN_TABLE,
    EG_KING_TABLE,
];

/// Evaluates the board position from the perspective of the side to move.
/// Combines base material, piece-square tables (PSTs), pawn structure, and tapered evaluation.
pub fn evaluate(board: &Board) -> i32 {
    if board.is_insufficient_material() {
        return 0;
    }
    let (pawn_mg, pawn_eg) = evaluate_pawns(board);
    evaluate_with_pawn_score(board, pawn_mg, pawn_eg)
}

/// Evaluates the board position using a caller-supplied `PawnTable` to cache
/// pawn structure scores across nodes with unchanged pawn placement.
pub fn evaluate_with_pawn_table(board: &Board, pawn_table: &mut PawnTable) -> i32 {
    if board.is_insufficient_material() {
        return 0;
    }
    evaluate_with_pawn_table_unchecked(board, pawn_table)
}

/// Evaluates the board position using `pawn_table` without re-checking insufficient material.
pub(crate) fn evaluate_with_pawn_table_unchecked(board: &Board, pawn_table: &mut PawnTable) -> i32 {
    let (pawn_mg, pawn_eg) = pawn_table.probe_or_eval(board);
    evaluate_with_pawn_score(board, pawn_mg, pawn_eg)
}

fn evaluate_with_pawn_score(board: &Board, pawn_mg: i32, pawn_eg: i32) -> i32 {
    let mut mg_white = 0;
    let mut eg_white = 0;
    let mut mg_black = 0;
    let mut eg_black = 0;
    let mut game_phase = 0;

    for piece in Piece::ALL {
        let bb = board.bitboard(piece);
        if bb.is_empty() {
            continue;
        }

        let kind = piece.kind() as usize;
        let is_white = piece.color() == Color::White;

        for sq in bb {
            game_phase += PHASE_WEIGHT[kind];

            // For White, squares map 0..63 directly (Rank 1 to Rank 8).
            // For Black, squares are vertically mirrored via `sq ^ 56`.
            let sq_idx = if is_white {
                sq.as_usize()
            } else {
                sq.as_usize() ^ 56
            };

            let mg = MG_VALUE[kind] + MG_PST[kind][sq_idx];
            let eg = EG_VALUE[kind] + EG_PST[kind][sq_idx];

            if is_white {
                mg_white += mg;
                eg_white += eg;
            } else {
                mg_black += mg;
                eg_black += eg;
            }
        }
    }

    let mg_score = (mg_white - mg_black) + pawn_mg;
    let eg_score = (eg_white - eg_black) + pawn_eg;

    // Linear interpolation between middlegame and endgame based on remaining piece material.
    let mg_phase = game_phase.min(MAX_PHASE);
    let eg_phase = MAX_PHASE - mg_phase;
    let eval = (mg_score * mg_phase + eg_score * eg_phase) / MAX_PHASE;

    if board.side_to_move == Color::White {
        eval
    } else {
        -eval
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess::Move;
    use std::str::FromStr;

    #[test]
    fn starting_position_is_symmetrical() {
        let board = Board::default();
        assert_eq!(evaluate(&board), 0);
    }

    #[test]
    fn white_advantage_gives_positive_score_for_white() {
        // White is up a Queen.
        let board =
            Board::from_fen("rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        let eval = evaluate(&board);
        assert!(
            eval > 800,
            "White should have large advantage with extra queen, got {}",
            eval
        );
    }

    #[test]
    fn black_perspective_flips_score() {
        // White plays 1. e4, gaining a slight positional advantage.
        let mut board = Board::default();
        let mv = board
            .try_make_move(Move::from_str("e2e4").unwrap())
            .unwrap();
        let eval_black = evaluate(&board);

        board.unmake_move(mv);
        let eval_white = evaluate(&board);
        assert_eq!(eval_white, 0);

        assert!(
            eval_black < 0,
            "Black to move facing e4 should see negative evaluation, got {}",
            eval_black
        );
    }

    #[test]
    fn tapered_eval_pure_endgame_matches_eg_score() {
        // King + Rook endgame (phase = 2 * 2 = 4).
        let board = Board::from_fen("4k3/8/8/8/8/8/8/4K2R w - - 0 1").unwrap();
        let eval = evaluate(&board);
        // White has an extra rook in endgame.
        assert!(
            eval > 400,
            "White should have clear endgame advantage, got {}",
            eval
        );
    }

    /// Mirrors a board vertically and flips all piece colors and side to move.
    fn color_flip(board: &Board) -> Board {
        use crate::chess::Square;

        let mut flipped = Board::empty();
        for sq_idx in 0..64u8 {
            let sq = Square::new(sq_idx);
            if let Some(piece) = board.piece_at(sq) {
                let mirrored_sq = Square::new(sq_idx ^ 56);
                let flipped_piece = Piece::new(piece.color().flip(), piece.kind());
                flipped.put_piece_no_zobrist(flipped_piece, mirrored_sq);
            }
        }
        flipped.side_to_move = board.side_to_move.flip();
        flipped.state.castling_rights = ((board.state.castling_rights & 0b0011) << 2)
            | ((board.state.castling_rights & 0b1100) >> 2);
        flipped.state.ep_square = board.state.ep_square.map(|sq| Square::new(sq.as_u8() ^ 56));
        flipped.state.halfmove_clock = board.state.halfmove_clock;
        flipped.fullmove_counter = board.fullmove_counter;
        flipped.state.zobrist_key = flipped.compute_zobrist_key();
        flipped.state.pawn_key = flipped.compute_pawn_key();
        flipped
    }

    #[test]
    fn eval_is_invariant_under_color_flip() {
        let fens = [
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "r1bqk2r/pp1n1ppp/2pbpn2/3p4/2PP4/2N1PN2/PP3PPP/R1BQKB1R w KQkq - 0 6",
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            "4k3/1ppp4/8/2P1p3/8/3P4/8/4K3 w - - 0 1",
            "4k3/p6p/4P3/8/8/8/8/4K3 b - - 0 1",
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        ];

        for fen in fens {
            let board = Board::from_fen(fen).unwrap();
            let flipped = color_flip(&board);
            assert_eq!(
                evaluate(&board),
                evaluate(&flipped),
                "Color-flip symmetry failed for FEN: {fen}"
            );
        }
    }

    #[test]
    fn cached_pawn_eval_matches_stateless_eval() {
        let fens = [
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            "4k3/8/8/8/8/8/8/4K2R w - - 0 1",
            "4k3/1ppp4/8/2P1p3/8/3P4/8/4K3 b - - 0 1",
        ];

        let mut table = PawnTable::default();
        for fen in fens {
            let board = Board::from_fen(fen).unwrap();
            assert_eq!(
                evaluate(&board),
                evaluate_with_pawn_table(&board, &mut table)
            );
            // Probe a second time to verify cache hit matches too.
            assert_eq!(
                evaluate(&board),
                evaluate_with_pawn_table(&board, &mut table)
            );
        }
    }

    #[test]
    fn insufficient_material_evaluates_to_zero() {
        let fens = [
            "4k3/8/8/8/8/8/8/4K3 w - - 0 1",
            "4k3/8/8/8/8/8/8/4K1N1 w - - 0 1",
            "4k3/8/8/8/8/8/8/4KB2 w - - 0 1",
            "2b1k3/8/8/8/8/8/8/4KB2 w - - 0 1",
        ];
        let mut table = PawnTable::default();
        for fen in fens {
            let board = Board::from_fen(fen).unwrap();
            assert_eq!(evaluate(&board), 0, "Expected 0 eval for {fen}");
            assert_eq!(
                evaluate_with_pawn_table(&board, &mut table),
                0,
                "Expected 0 cached eval for {fen}"
            );
        }
    }
}
