use super::attacks;
use crate::chess::piece::{Color, Kind};
use crate::chess::{Bitboard, Flag, Move, Piece, Square};
use crate::position::Board;

const PROMO_KINDS: [Kind; 4] = [Kind::Queen, Kind::Rook, Kind::Bishop, Kind::Knight];

/// Generates all valid chess moves, taking into account King safety.
pub fn gen_all_moves(board: &Board) -> Vec<Move> {
    let mut moves = Vec::with_capacity(256);
    let us = board.side_to_move;
    let them = us.flip();

    let king_sq = board.bitboard(Piece::new(us, Kind::King)).lsb();
    let checkers = get_attackers(board, king_sq, them);
    let num_checkers = checkers.count();
    let pinned = get_pinned_pieces(board, king_sq, us);

    // King must always be able to move regardless of check status
    moves.append(&mut gen_king_moves(board, king_sq, us, !board.us(us)));

    // If checked by 2 or more pieces, mathematically only the King could
    // move to escape the check, so we exit early
    if num_checkers > 1 {
        return moves;
    }

    // Determine the valid target squares for all other pieces
    let target_mask = if num_checkers == 1 {
        let checker_sq = checkers.lsb();
        // Pieces can only capture the checking piece or block its ray
        let checker_bb = Bitboard::new(checker_sq.mask());
        checker_bb | attacks::ray_between(king_sq, checker_sq)
    } else {
        // Not in check
        Bitboard::new(!0)
    };

    // Generate pseudo-legal moves masked by pins and the target_mask
    moves.append(&mut gen_piece_moves(
        board,
        us,
        Kind::Knight,
        pinned,
        target_mask,
        king_sq,
    ));
    moves.append(&mut gen_piece_moves(
        board,
        us,
        Kind::Bishop,
        pinned,
        target_mask,
        king_sq,
    ));
    moves.append(&mut gen_piece_moves(
        board,
        us,
        Kind::Rook,
        pinned,
        target_mask,
        king_sq,
    ));
    moves.append(&mut gen_piece_moves(
        board,
        us,
        Kind::Queen,
        pinned,
        target_mask,
        king_sq,
    ));

    moves.append(&mut gen_pawn_moves(board, us, pinned, target_mask, king_sq));
    if num_checkers == 0 {
        moves.append(&mut gen_castling_moves(board, us, king_sq));
    }

    moves
}

/// Generates all necessary moves for quiescence search (evasions if in check, captures otherwise).
pub fn gen_qsearch_moves(board: &Board) -> Vec<Move> {
    let us = board.side_to_move;
    let them = us.flip();

    let king_sq = board.bitboard(Piece::new(us, Kind::King)).lsb();
    let checkers = get_attackers(board, king_sq, them);

    // If in check, route to the standard generator to find all legal evasions.
    if !checkers.is_empty() {
        return gen_all_moves(board);
    }

    // If not in check, generate strictly captures inline.
    let mut moves = Vec::with_capacity(64);
    let pinned = get_pinned_pieces(board, king_sq, us);

    // The target_mask is strictly the enemy pieces.
    // This forces the helper functions to drop all quiet moves.
    let target_mask = board.us(them);

    moves.append(&mut gen_king_moves(board, king_sq, us, target_mask));

    moves.append(&mut gen_piece_moves(
        board,
        us,
        Kind::Knight,
        pinned,
        target_mask,
        king_sq,
    ));
    moves.append(&mut gen_piece_moves(
        board,
        us,
        Kind::Bishop,
        pinned,
        target_mask,
        king_sq,
    ));
    moves.append(&mut gen_piece_moves(
        board,
        us,
        Kind::Rook,
        pinned,
        target_mask,
        king_sq,
    ));
    moves.append(&mut gen_piece_moves(
        board,
        us,
        Kind::Queen,
        pinned,
        target_mask,
        king_sq,
    ));

    let promo_rank_mask = match us {
        Color::White => Bitboard::new(Bitboard::RANK_8),
        Color::Black => Bitboard::new(Bitboard::RANK_1),
    };
    // Allow captures OR promotions
    let target_mask = board.us(them) | promo_rank_mask;
    moves.append(&mut gen_pawn_moves(board, us, pinned, target_mask, king_sq));

    // Castling is inherently a quiet move and is intentionally omitted here.

    moves
}

/// Generates valid king moves
fn gen_king_moves(board: &Board, king_sq: Square, us: Color, target_mask: Bitboard) -> Vec<Move> {
    let king_attacks = attacks::king_attacks(king_sq) & target_mask;
    let them = us.flip();
    let their_pieces = board.us(them);
    let mut moves = Vec::new();

    // Temporarily clear the king from the blockers mask to expose X-ray attacks behind it
    let blockers_without_king = board.occupancy() & !Bitboard::new(king_sq.mask());

    for to_sq in king_attacks {
        // Simulate enemy attacks on the target square using the modified occupancy
        let mut is_attacked = false;

        // Leaping pieces
        if !(attacks::knight_attacks(to_sq) & board.bitboard(Piece::new(them, Kind::Knight)))
            .is_empty()
            || !(attacks::pawn_attacks(to_sq, us) & board.bitboard(Piece::new(them, Kind::Pawn)))
                .is_empty()
            || !(attacks::king_attacks(to_sq) & board.bitboard(Piece::new(them, Kind::King)))
                .is_empty()
        {
            is_attacked = true;
        } else {
            // Sliding pieces
            let bishops_queens = board.bitboard(Piece::new(them, Kind::Bishop))
                | board.bitboard(Piece::new(them, Kind::Queen));
            if !(attacks::bishop_attacks(to_sq, blockers_without_king) & bishops_queens).is_empty()
            {
                is_attacked = true;
            } else {
                let rooks_queens = board.bitboard(Piece::new(them, Kind::Rook))
                    | board.bitboard(Piece::new(them, Kind::Queen));
                if !(attacks::rook_attacks(to_sq, blockers_without_king) & rooks_queens).is_empty()
                {
                    is_attacked = true;
                }
            }
        }

        if !is_attacked {
            let flag = if their_pieces.contains(to_sq) {
                Flag::Capture
            } else {
                Flag::Quiet
            };
            moves.push(Move::new_with_flag(king_sq, to_sq, flag));
        }
    }

    moves
}

/// Generates valid moves for Knight, Bishop, Rook, Queen
fn gen_piece_moves(
    board: &Board,
    us: Color,
    kind: Kind,
    pinned: Bitboard,
    target_mask: Bitboard,
    king_sq: Square,
) -> Vec<Move> {
    let pieces = board.bitboard(Piece::new(us, kind));
    let blockers = board.occupancy();
    let their_pieces = board.us(us.flip());
    let mut moves = Vec::new();

    for from in pieces {
        let mut attacks = match kind {
            Kind::Knight => attacks::knight_attacks(from),
            Kind::Bishop => attacks::bishop_attacks(from, blockers),
            Kind::Rook => attacks::rook_attacks(from, blockers),
            Kind::Queen => {
                attacks::bishop_attacks(from, blockers) | attacks::rook_attacks(from, blockers)
            }
            _ => unreachable!("Invalid type for gen_piece_moves"),
        };

        // Filter 1: Cannot capture own pieces
        attacks &= !board.us(us);

        // Filter 2: Must resolve checks (block or capture)
        attacks &= target_mask;

        // Filter 3: Absolute pin restrictions
        if pinned.contains(from) {
            let pin_mask = attacks::line_through(king_sq, from);
            attacks &= pin_mask;
        }

        for to in attacks {
            let flag = if their_pieces.contains(to) {
                Flag::Capture
            } else {
                Flag::Quiet
            };
            moves.push(Move::new_with_flag(from, to, flag));
        }
    }

    moves
}

/// Generates valid pawn moves
fn gen_pawn_moves(
    board: &Board,
    us: Color,
    pinned: Bitboard,
    target_mask: Bitboard,
    king_sq: Square,
) -> Vec<Move> {
    let pawns = board.bitboard(Piece::new(us, Kind::Pawn));
    let occupancy = board.occupancy();
    let their_pieces = board.us(us.flip());
    let mut moves = Vec::new();

    let (forward_shift, start_rank, promo_rank) = match us {
        Color::White => (8, 1, 7),
        Color::Black => (-8, 6, 0),
    };

    for from in pawns {
        // Determine local movement mask based on absolute pins
        let move_mask = if pinned.contains(from) {
            target_mask & attacks::line_through(king_sq, from)
        } else {
            target_mask
        };

        // Single and double pushes
        let single_push_sq = Square::new((from.as_usize() as i8 + forward_shift) as u8);
        if !occupancy.contains(single_push_sq) {
            // Check if single push resolves checks or stays on pin ray
            if move_mask.contains(single_push_sq) {
                if single_push_sq.rank() == promo_rank {
                    for kind in PROMO_KINDS {
                        moves.push(Move::new_promo(from, single_push_sq, kind));
                    }
                } else {
                    moves.push(Move::new_with_flag(from, single_push_sq, Flag::Quiet));
                }
            }

            // Double push
            if from.rank() == start_rank {
                let double_push_sq = Square::new((from.as_usize() as i8 + forward_shift * 2) as u8);
                if !occupancy.contains(double_push_sq) && move_mask.contains(double_push_sq) {
                    moves.push(Move::new_with_flag(
                        from,
                        double_push_sq,
                        Flag::DoublePawnPush,
                    ));
                }
            }
        }

        // Standard captures
        let mut captures = attacks::pawn_attacks(from, us) & their_pieces & move_mask;
        while !captures.is_empty() {
            let to = captures.lsb();
            captures.clear_bit(to);
            if to.rank() == promo_rank {
                for kind in PROMO_KINDS {
                    moves.push(Move::new_promo_capture(from, to, kind));
                }
            } else {
                moves.push(Move::new_with_flag(from, to, Flag::Capture));
            }
        }

        // En passant
        if let Some(ep_sq) = board.state.ep_square {
            let ep_bb = Bitboard::new(ep_sq.mask());
            let ep_capture = attacks::pawn_attacks(from, us) & ep_bb;

            if !ep_capture.is_empty() && is_ep_legal(board, from, ep_sq, king_sq, us) {
                moves.push(Move::new_with_flag(from, ep_sq, Flag::EpCapture));
            }
        }
    }

    moves
}

/// Generates valid castling moves
fn gen_castling_moves(board: &Board, us: Color, king_sq: Square) -> Vec<Move> {
    let occupancy = board.occupancy();
    let them = us.flip();
    let mut moves = Vec::new();

    // Kingside Castling
    if board.can_castle_kingside(us) {
        let (f_sq, g_sq) = match us {
            Color::White => (Square::new(5), Square::new(6)), // f1, g1
            Color::Black => (Square::new(61), Square::new(62)), // f8, g8
        };

        // Squares between king and rook must be empty
        if !occupancy.contains(f_sq) && !occupancy.contains(g_sq) {
            // Transit squares must not be attacked
            let attackers_f = get_attackers(board, f_sq, them);
            let attackers_g = get_attackers(board, g_sq, them);

            if attackers_f.is_empty() && attackers_g.is_empty() {
                moves.push(Move::new_with_flag(king_sq, g_sq, Flag::KingCastle));
            }
        }
    }

    // Queenside Castling
    if board.can_castle_queenside(us) {
        let (b_sq, c_sq, d_sq) = match us {
            Color::White => (Square::new(1), Square::new(2), Square::new(3)), // b1, c1, d1
            Color::Black => (Square::new(57), Square::new(58), Square::new(59)), // b8, c8, d8
        };

        // Squares between king and rook must be empty
        if !occupancy.contains(b_sq) && !occupancy.contains(c_sq) && !occupancy.contains(d_sq) {
            // Only the king's transit squares (c and d) need attack verification.
            // The b-file square is not traversed by the king
            let attackers_c = get_attackers(board, c_sq, them);
            let attackers_d = get_attackers(board, d_sq, them);

            if attackers_c.is_empty() && attackers_d.is_empty() {
                moves.push(Move::new_with_flag(king_sq, c_sq, Flag::QueenCastle));
            }
        }
    }

    moves
}

/// Returns `true` if the side to move's king is currently in check.
pub fn is_in_check(board: &Board) -> bool {
    let us = board.side_to_move;
    let king_sq = board.bitboard(Piece::new(us, Kind::King)).lsb();
    !get_attackers(board, king_sq, us.flip()).is_empty()
}

/// Get a bitboard of the pieces that are attacking the King
fn get_attackers(board: &Board, sq: Square, attacker_col: Color) -> Bitboard {
    let mut attackers = Bitboard::new(0);
    let defender_col = attacker_col.flip();

    // Leaping attacks
    attackers |= attacks::pawn_attacks(sq, defender_col)
        & board.bitboard(Piece::new(attacker_col, Kind::Pawn));

    attackers |=
        attacks::knight_attacks(sq) & board.bitboard(Piece::new(attacker_col, Kind::Knight));

    attackers |= attacks::king_attacks(sq) & board.bitboard(Piece::new(attacker_col, Kind::King));

    let blockers = board.occupancy();

    // Sliding attacks
    let bishops_queens = board.bitboard(Piece::new(attacker_col, Kind::Bishop))
        | board.bitboard(Piece::new(attacker_col, Kind::Queen));
    attackers |= attacks::bishop_attacks(sq, blockers) & bishops_queens;

    let rooks_queens = board.bitboard(Piece::new(attacker_col, Kind::Rook))
        | board.bitboard(Piece::new(attacker_col, Kind::Queen));
    attackers |= attacks::rook_attacks(sq, blockers) & rooks_queens;

    attackers
}

/// Get a bitboard of friendly pieces that are covering the King from a check
fn get_pinned_pieces(board: &Board, king_sq: Square, us: Color) -> Bitboard {
    let mut pinned = Bitboard::new(0);
    let them = us.flip();

    let our_pieces = board.us(us);
    let their_pieces = board.us(them);

    // Orthogonal pins
    let enemy_ortho = board.bitboard(Piece::new(them, Kind::Rook))
        | board.bitboard(Piece::new(them, Kind::Queen));

    let ortho_pinners = attacks::rook_attacks(king_sq, board.us(them)) & enemy_ortho;

    for pinner_sq in ortho_pinners {
        let between = attacks::ray_between(king_sq, pinner_sq) & our_pieces;
        if between.count() == 1 {
            pinned |= between;
        }
    }

    // Diagonal pins
    let enemy_diag = board.bitboard(Piece::new(them, Kind::Bishop))
        | board.bitboard(Piece::new(them, Kind::Queen));

    let diag_pinners = attacks::bishop_attacks(king_sq, their_pieces) & enemy_diag;

    for pinner_sq in diag_pinners {
        let between = attacks::ray_between(king_sq, pinner_sq) & our_pieces;
        if between.count() == 1 {
            pinned |= between;
        }
    }

    pinned
}

fn is_ep_legal(board: &Board, from: Square, ep_sq: Square, king_sq: Square, us: Color) -> bool {
    let them = us.flip();

    // The square of the pawn being captured.
    let captured_pawn_sq = Square::new(match us {
        Color::White => ep_sq.as_usize() - 8,
        Color::Black => ep_sq.as_usize() + 8,
    } as u8);

    // Reject if a knight or another enemy pawn still checks the king after removing the captured pawn.
    if !(attacks::knight_attacks(king_sq) & board.bitboard(Piece::new(them, Kind::Knight)))
        .is_empty()
    {
        return false;
    }

    let remaining_enemy_pawns =
        board.bitboard(Piece::new(them, Kind::Pawn)) & !Bitboard::new(captured_pawn_sq.mask());
    if !(attacks::pawn_attacks(king_sq, us) & remaining_enemy_pawns).is_empty() {
        return false;
    }

    // Create a modified occupancy bitboard reflecting the state after the EP capture.
    let mut modified_occupancy = board.occupancy();
    modified_occupancy.clear_bit(from);
    modified_occupancy.clear_bit(captured_pawn_sq);
    modified_occupancy |= Bitboard::new(ep_sq.mask());

    // Check if the king is attacked orthogonally by a rook or queen.
    let rooks_queens = board.bitboard(Piece::new(them, Kind::Rook))
        | board.bitboard(Piece::new(them, Kind::Queen));

    let orthogonal_attacks = attacks::rook_attacks(king_sq, modified_occupancy) & rooks_queens;

    if !orthogonal_attacks.is_empty() {
        return false;
    }

    // Check diagonal attacks (in case the EP capture opened a diagonal).
    let bishops_queens = board.bitboard(Piece::new(them, Kind::Bishop))
        | board.bitboard(Piece::new(them, Kind::Queen));

    let diagonal_attacks = attacks::bishop_attacks(king_sq, modified_occupancy) & bishops_queens;

    if !diagonal_attacks.is_empty() {
        return false;
    }

    true
}
