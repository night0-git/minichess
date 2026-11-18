use crate::chess::piece::{Color, Piece};
use crate::chess::{Bitboard, Direction, Square};
use crate::move_gen::attacks::pawn_attacks;
use crate::position::board::Board;

pub const DEFAULT_PAWN_TABLE_ENTRIES: usize = 16_384; // 256 KiB

const DOUBLED_PENALTY_MG: i32 = -10;
const DOUBLED_PENALTY_EG: i32 = -20;

const ISOLATED_PENALTY_MG: i32 = -12;
const ISOLATED_PENALTY_EG: i32 = -18;

const BACKWARD_PENALTY_MG: i32 = -8;
const BACKWARD_PENALTY_EG: i32 = -12;

/// Bonus for connected pawns (diagonally supported or phalanx) indexed by relative rank (0..8).
/// Index 1 (starting rank) is 0 so pushing center pawns on move 1 is not penalized.
const CONNECTED_MG: [i32; 8] = [0, 0, 5, 10, 18, 28, 40, 0];
const CONNECTED_EG: [i32; 8] = [0, 0, 4, 8, 16, 28, 45, 0];

/// Bonus for passed pawns indexed by relative rank (0..8).
const PASSED_MG: [i32; 8] = [0, 5, 10, 15, 28, 50, 80, 0];
const PASSED_EG: [i32; 8] = [0, 10, 15, 28, 48, 80, 125, 0];

const ADJACENT_FILES: [Bitboard; 8] = init_adjacent_files();
const FRONT_SPAN: [[Bitboard; 64]; 2] = init_front_span();
const PASSED_MASK: [[Bitboard; 64]; 2] = init_passed_mask();
const BACKWARD_SUPPORT_MASK: [[Bitboard; 64]; 2] = init_backward_support_mask();
const PHALANX_MASK: [Bitboard; 64] = init_phalanx_mask();

// Note: These operations cannot be used in const function:
// - for loop
// - trait methods (in this case bitwise methods)

/// Builds full-column bitboard masks for each file `0..8` (`a`..`h`).
const fn init_file_masks() -> [Bitboard; 8] {
    let mut masks = [Bitboard::EMPTY; 8];
    let mut file = 0;
    while file < 8 {
        masks[file] = Bitboard::new(Bitboard::FILE_A << file);
        file += 1;
    }
    masks
}

/// Builds masks of adjacent files for isolated pawn detection.
const fn init_adjacent_files() -> [Bitboard; 8] {
    let file_masks = init_file_masks();
    let mut masks = [Bitboard::EMPTY; 8];
    let mut file = 0;
    while file < 8 {
        let mut mask = 0u64;
        if file > 0 {
            mask |= file_masks[file - 1].as_u64();
        }
        if file < 7 {
            mask |= file_masks[file + 1].as_u64();
        }
        masks[file] = Bitboard::new(mask);
        file += 1;
    }
    masks
}

/// Builds same-file front-span masks to detect doubled pawns and frontmost passers.
const fn init_front_span() -> [[Bitboard; 64]; 2] {
    let mut masks = [[Bitboard::EMPTY; 64]; 2];
    let mut sq = 0;
    while sq < 64 {
        let rank = sq / 8;
        let file = sq % 8;

        let mut white_mask = 0u64;
        let mut r = rank + 1;
        while r < 8 {
            white_mask |= 1u64 << (r * 8 + file);
            r += 1;
        }
        masks[0][sq] = Bitboard::new(white_mask);

        let mut black_mask = 0u64;
        let mut r = 0;
        while r < rank {
            black_mask |= 1u64 << (r * 8 + file);
            r += 1;
        }
        masks[1][sq] = Bitboard::new(black_mask);

        sq += 1;
    }
    masks
}

/// Builds 3-file front-span masks to detect passed pawns (no enemy pawns inside).
const fn init_passed_mask() -> [[Bitboard; 64]; 2] {
    let file_masks = init_file_masks();
    let adjacent_files = init_adjacent_files();
    let mut masks = [[Bitboard::EMPTY; 64]; 2];

    let mut sq = 0;
    while sq < 64 {
        let rank = sq / 8;
        let file = sq % 8;
        let span_files = file_masks[file].as_u64() | adjacent_files[file].as_u64();

        let mut white_ranks = 0u64;
        let mut r = rank + 1;
        while r < 8 {
            white_ranks |= Bitboard::RANK_1 << (r * 8);
            r += 1;
        }
        masks[0][sq] = Bitboard::new(span_files & white_ranks);

        let mut black_ranks = 0u64;
        let mut r = 0;
        while r < rank {
            black_ranks |= Bitboard::RANK_1 << (r * 8);
            r += 1;
        }
        masks[1][sq] = Bitboard::new(span_files & black_ranks);

        sq += 1;
    }
    masks
}

/// Builds adjacent-file rear/abreast masks to detect backward pawns lacking support.
const fn init_backward_support_mask() -> [[Bitboard; 64]; 2] {
    let adjacent_files = init_adjacent_files();
    let mut masks = [[Bitboard::EMPTY; 64]; 2];

    let mut sq = 0;
    while sq < 64 {
        let rank = sq / 8;
        let file = sq % 8;
        let adj = adjacent_files[file].as_u64();

        let mut white_ranks = 0u64;
        let mut r = 0;
        while r <= rank {
            white_ranks |= Bitboard::RANK_1 << (r * 8);
            r += 1;
        }
        masks[0][sq] = Bitboard::new(adj & white_ranks);

        let mut black_ranks = 0u64;
        let mut r = rank;
        while r < 8 {
            black_ranks |= Bitboard::RANK_1 << (r * 8);
            r += 1;
        }
        masks[1][sq] = Bitboard::new(adj & black_ranks);

        sq += 1;
    }
    masks
}

/// Builds horizontally adjacent same-rank masks to detect phalanx pawns.
const fn init_phalanx_mask() -> [Bitboard; 64] {
    let mut masks = [Bitboard::EMPTY; 64];
    let mut sq = 0;
    while sq < 64 {
        let file = sq % 8;
        let mut mask = 0u64;
        if file > 0 {
            mask |= 1u64 << (sq - 1);
        }
        if file < 7 {
            mask |= 1u64 << (sq + 1);
        }
        masks[sq] = Bitboard::new(mask);
        sq += 1;
    }
    masks
}

/// Cached pawn structure evaluation entry keyed by `Board::state.pawn_key`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PawnEntry {
    pub key: u64,
    pub mg: i16,
    pub eg: i16,
    pub valid: bool,
}

/// Direct-mapped cache for pawn-only structure evaluations.
#[derive(Debug, Clone)]
pub struct PawnTable {
    entries: Vec<PawnEntry>,
    mask: usize,
}

impl PawnTable {
    /// Creates a new pawn hash table with at least `capacity` slots rounded down
    /// to a power of two (minimum 1).
    pub fn new(capacity: usize) -> Self {
        let mut count = capacity.max(1).next_power_of_two();
        if count > capacity.max(1) && count > 1 {
            count /= 2;
        }
        Self {
            entries: vec![PawnEntry::default(); count],
            mask: count - 1,
        }
    }

    /// Looks up the cached pawn structure `(mg, eg)` score for `board`, or computes
    /// and caches it on a miss.
    #[inline]
    pub fn probe_or_eval(&mut self, board: &Board) -> (i32, i32) {
        let key = board.state.pawn_key;
        let idx = (key as usize) & self.mask;
        let entry = self.entries[idx];
        if entry.valid && entry.key == key {
            return (entry.mg as i32, entry.eg as i32);
        }

        let (mg, eg) = evaluate_pawns(board);
        self.entries[idx] = PawnEntry {
            key,
            mg: mg as i16,
            eg: eg as i16,
            valid: true,
        };
        (mg, eg)
    }

    /// Clears all cached entries.
    pub fn clear(&mut self) {
        for entry in &mut self.entries {
            *entry = PawnEntry::default();
        }
    }
}

impl Default for PawnTable {
    fn default() -> Self {
        Self::new(DEFAULT_PAWN_TABLE_ENTRIES)
    }
}

/// Computes the pawn structure `(mg, eg)` score from White's perspective
/// (`white - black`).
pub fn evaluate_pawns(board: &Board) -> (i32, i32) {
    let white_pawns = board.bitboard(Piece::WPawn);
    let black_pawns = board.bitboard(Piece::BPawn);

    let (w_mg, w_eg) = eval_side_pawns(Color::White, white_pawns, black_pawns);
    let (b_mg, b_eg) = eval_side_pawns(Color::Black, black_pawns, white_pawns);

    (w_mg - b_mg, w_eg - b_eg)
}

fn eval_side_pawns(us: Color, friendly_pawns: Bitboard, enemy_pawns: Bitboard) -> (i32, i32) {
    if friendly_pawns.is_empty() {
        return (0, 0);
    }

    let friendly_attacks = match us {
        Color::White => {
            friendly_pawns.shift(Direction::NorthWest) | friendly_pawns.shift(Direction::NorthEast)
        }
        Color::Black => {
            friendly_pawns.shift(Direction::SouthWest) | friendly_pawns.shift(Direction::SouthEast)
        }
    };

    let color_idx = us as usize;
    let mut mg = 0;
    let mut eg = 0;

    for sq in friendly_pawns {
        let sq_idx = sq.as_usize();
        let rank = sq.rank() as usize;
        let file = sq.file() as usize;
        let rel_rank = if us == Color::White { rank } else { 7 - rank };

        // Doubled: at least one friendly pawn stands ahead on the same file.
        if (FRONT_SPAN[color_idx][sq_idx] & friendly_pawns).is_any() {
            mg += DOUBLED_PENALTY_MG;
            eg += DOUBLED_PENALTY_EG;
        }

        // Isolated vs Backward (mutually exclusive so penalties do not stack unduly).
        let is_isolated = (ADJACENT_FILES[file] & friendly_pawns).is_empty();
        if is_isolated {
            mg += ISOLATED_PENALTY_MG;
            eg += ISOLATED_PENALTY_EG;
        } else if (BACKWARD_SUPPORT_MASK[color_idx][sq_idx] & friendly_pawns).is_empty() {
            let stop_sq = match us {
                Color::White if rank < 7 => Some(Square::new(sq.as_u8() + 8)),
                Color::Black if rank > 0 => Some(Square::new(sq.as_u8() - 8)),
                _ => None,
            };
            if let Some(stop) = stop_sq
                && (pawn_attacks(stop, us) & enemy_pawns).is_any()
            {
                mg += BACKWARD_PENALTY_MG;
                eg += BACKWARD_PENALTY_EG;
            }
        }

        // Connected: defended diagonally by a friendly pawn or paired as a phalanx.
        if friendly_attacks.contains(sq) || (PHALANX_MASK[sq_idx] & friendly_pawns).is_any() {
            mg += CONNECTED_MG[rel_rank];
            eg += CONNECTED_EG[rel_rank];
        }

        // Passed: no enemy pawns ahead on same or adjacent files, and frontmost on file.
        if (PASSED_MASK[color_idx][sq_idx] & enemy_pawns).is_empty()
            && (FRONT_SPAN[color_idx][sq_idx] & friendly_pawns).is_empty()
        {
            mg += PASSED_MG[rel_rank];
            eg += PASSED_EG[rel_rank];
        }
    }

    (mg, eg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symmetrical_starting_pawns_score_zero() {
        let board = Board::default();
        assert_eq!(evaluate_pawns(&board), (0, 0));
    }

    #[test]
    fn doubled_pawns_are_penalized() {
        // White has doubled pawns on e2/e3 vs Black's healthy d7/e7.
        let doubled = Board::from_fen("4k3/3pp3/8/8/8/4P3/4P3/4K3 w - - 0 1").unwrap();
        let (mg, eg) = evaluate_pawns(&doubled);
        assert!(
            mg < 0 && eg < 0,
            "Expected doubled penalty, got ({mg}, {eg})"
        );
    }

    #[test]
    fn isolated_pawn_is_penalized() {
        let blocked_isolated = Board::from_fen("4k3/3p4/8/8/3P4/8/8/4K3 w - - 0 1").unwrap();
        let (b_mg, b_eg) = eval_side_pawns(
            Color::White,
            blocked_isolated.bitboard(Piece::WPawn),
            blocked_isolated.bitboard(Piece::BPawn),
        );
        assert_eq!((b_mg, b_eg), (ISOLATED_PENALTY_MG, ISOLATED_PENALTY_EG));
    }

    #[test]
    fn backward_pawn_is_penalized() {
        // White has pawns on c5 (ahead, cannot support d3) and d3; Black has b7/c7/d7/e5.
        // d3 has no adjacent friendly pawns on ranks <= 3 and its stop square d4 is attacked by e5.
        let clean = Board::from_fen("4k3/1ppp4/8/2P1p3/8/3P4/8/4K3 w - - 0 1").unwrap();
        let (clean_mg, clean_eg) = eval_side_pawns(
            Color::White,
            clean.bitboard(Piece::WPawn),
            clean.bitboard(Piece::BPawn),
        );
        assert_eq!(
            (clean_mg, clean_eg),
            (BACKWARD_PENALTY_MG, BACKWARD_PENALTY_EG)
        );
    }

    #[test]
    fn connected_pawns_receive_bonus() {
        // White pawns on d4 and e4 (rank 4 phalanx), Black pawns on d6 and f6 (disconnected, not isolated? e-file empty so d6/f6 are isolated).
        // Compare White d4+e4 vs White d4+f4 (both blocked by Black pawns on d7/e7/f7 so no passers).
        let connected = Board::from_fen("4k3/3ppp2/8/8/3PP3/8/8/4K3 w - - 0 1").unwrap();
        let disconnected = Board::from_fen("4k3/3p1p2/8/8/3P1P2/8/8/4K3 w - - 0 1").unwrap();

        let (conn_mg, conn_eg) = eval_side_pawns(
            Color::White,
            connected.bitboard(Piece::WPawn),
            connected.bitboard(Piece::BPawn),
        );
        let (disc_mg, disc_eg) = eval_side_pawns(
            Color::White,
            disconnected.bitboard(Piece::WPawn),
            disconnected.bitboard(Piece::BPawn),
        );
        assert!(conn_mg > disc_mg && conn_eg > disc_eg);
    }

    #[test]
    fn passed_pawn_bonus_scales_with_advancement_and_is_not_doubled() {
        // White passed pawn on e5 vs e6 (with Black pawns on a7/h7).
        let rank5 = Board::from_fen("4k3/p6p/8/4P3/8/8/8/4K3 w - - 0 1").unwrap();
        let rank6 = Board::from_fen("4k3/p6p/4P3/8/8/8/8/4K3 w - - 0 1").unwrap();

        let (mg5, eg5) = eval_side_pawns(
            Color::White,
            rank5.bitboard(Piece::WPawn),
            rank5.bitboard(Piece::BPawn),
        );
        let (mg6, eg6) = eval_side_pawns(
            Color::White,
            rank6.bitboard(Piece::WPawn),
            rank6.bitboard(Piece::BPawn),
        );
        assert!(mg6 > mg5 && eg6 > eg5);
        assert!(eg6 > mg6);

        // Doubled passed pawns on e5/e6 should only award ONE passed pawn bonus (for e6) plus doubled penalty.
        let doubled_passer = Board::from_fen("4k3/p6p/4P3/4P3/8/8/8/4K3 w - - 0 1").unwrap();
        let (dp_mg, dp_eg) = eval_side_pawns(
            Color::White,
            doubled_passer.bitboard(Piece::WPawn),
            doubled_passer.bitboard(Piece::BPawn),
        );
        assert_eq!(
            (dp_mg, dp_eg),
            (
                mg6 + DOUBLED_PENALTY_MG + ISOLATED_PENALTY_MG,
                eg6 + DOUBLED_PENALTY_EG + ISOLATED_PENALTY_EG
            )
        );
    }

    #[test]
    fn pawn_table_caches_hits_including_pawnless_endgame() {
        let mut table = PawnTable::new(16);

        // Pawnless position has pawn_key == 0; valid flag must still handle it cleanly.
        let pawnless = Board::from_fen("4k3/8/8/8/8/8/8/4K2R w - - 0 1").unwrap();
        assert_eq!(pawnless.state.pawn_key, 0);
        assert_eq!(table.probe_or_eval(&pawnless), (0, 0));
        assert_eq!(table.probe_or_eval(&pawnless), (0, 0));

        // Non-trivial pawn structure: miss then hit must match evaluate_pawns.
        let pos = Board::from_fen("4k3/pp3ppp/2p5/3pP3/3P4/2P5/PP3PPP/4K3 w - - 0 1").unwrap();
        let expected = evaluate_pawns(&pos);
        assert_eq!(table.probe_or_eval(&pos), expected);
        assert_eq!(table.probe_or_eval(&pos), expected);
    }
}
