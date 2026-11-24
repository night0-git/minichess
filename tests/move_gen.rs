mod common;

use std::collections::HashSet;
use std::str::FromStr;

use common::{assert_board_invariants, assert_unique_moves, has_uci, uci_set};
use minichess::chess::{Move, Square};
use minichess::move_gen::{gen_all_moves, gen_qsearch_moves};
use minichess::position::Board;

fn sq(name: &str) -> Square {
    Square::from_str(name).expect("valid square")
}

fn moves_from(moves: &[Move], origin: &str) -> Vec<Move> {
    let from = sq(origin);
    moves.iter().copied().filter(|m| m.from() == from).collect()
}

fn uci_set_from(ucis: &[&str]) -> HashSet<String> {
    ucis.iter().map(|s| (*s).to_string()).collect()
}

fn perft(board: &mut Board, depth: usize) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves = gen_all_moves(board);
    if depth == 1 {
        return moves.len() as u64;
    }
    let mut nodes = 0;
    for mv in moves {
        board.make_move(mv);
        nodes += perft(board, depth - 1);
        board.unmake_move(mv);
    }
    nodes
}

fn perft_checked(board: &mut Board, depth: usize) -> u64 {
    assert_board_invariants(board);
    if depth == 0 {
        return 1;
    }
    let moves = gen_all_moves(board);
    assert_unique_moves(&moves);
    if depth == 1 {
        return moves.len() as u64;
    }
    let snapshot = board.clone();
    let mut nodes = 0;
    for mv in moves {
        board.make_move(mv);
        nodes += perft_checked(board, depth - 1);
        board.unmake_move(mv);
        assert_eq!(*board, snapshot, "board drifted after unmaking {mv}");
    }
    nodes
}

mod piece_moves {
    use super::*;

    #[test]
    fn knight_leaps_cover_center_corner_friendly_block_and_enemy_capture() {
        let board = Board::from_fen("7k/8/2p1P3/8/3N4/8/8/N6K w - - 0 1").unwrap();
        let moves = gen_all_moves(&board);
        assert_unique_moves(&moves);

        assert_eq!(
            uci_set(&moves_from(&moves, "d4")),
            uci_set_from(&["d4c6", "d4b5", "d4b3", "d4c2", "d4e2", "d4f3", "d4f5"])
        );
        assert_eq!(
            uci_set(&moves_from(&moves, "a1")),
            uci_set_from(&["a1b3", "a1c2"])
        );
    }

    #[test]
    fn bishop_slides_stop_at_friendly_and_capture_enemy_blockers() {
        let board = Board::from_fen("7k/8/5p2/8/3B4/2P5/8/K7 w - - 0 1").unwrap();
        let moves = gen_all_moves(&board);
        assert_unique_moves(&moves);

        assert_eq!(
            uci_set(&moves_from(&moves, "d4")),
            uci_set_from(&[
                "d4e5", "d4f6", "d4c5", "d4b6", "d4a7", "d4e3", "d4f2", "d4g1"
            ])
        );
    }

    #[test]
    fn rook_slides_stop_at_friendly_and_capture_enemy_blockers() {
        let board = Board::from_fen("7k/8/3p4/8/1P1R4/8/8/K7 w - - 0 1").unwrap();
        let moves = gen_all_moves(&board);
        assert_unique_moves(&moves);

        assert_eq!(
            uci_set(&moves_from(&moves, "d4")),
            uci_set_from(&[
                "d4d5", "d4d6", "d4c4", "d4d3", "d4d2", "d4d1", "d4e4", "d4f4", "d4g4", "d4h4"
            ])
        );
    }

    #[test]
    fn queen_combines_orthogonal_and_diagonal_rays() {
        let board = Board::from_fen("7k/8/3p1p2/8/3Q4/2P5/8/K7 w - - 0 1").unwrap();
        let moves = gen_all_moves(&board);
        assert_unique_moves(&moves);

        let q_ucis = uci_set(&moves_from(&moves, "d4"));
        assert!(q_ucis.contains("d4d6"));
        assert!(q_ucis.contains("d4f6"));
        assert!(!q_ucis.contains("d4d7"));
        assert!(!q_ucis.contains("d4g7"));
        assert!(!q_ucis.contains("d4c3"));
        assert!(q_ucis.contains("d4a4"));
        assert!(q_ucis.contains("d4a7"));
    }

    #[test]
    fn king_avoids_all_attacking_piece_types_and_only_captures_undefended_pieces() {
        // White king on e4:
        // - d5: attacked by Black king on c6
        // - e5: Black pawn defended by Black pawn on f6 and rook on h5
        // - f5: attacked by Black rook on h5
        // - d4: attacked by Black knight on b3 and Black pawn on e5
        // - f4: friendly White pawn
        // - d3: attacked by Black bishop on a6
        // - e3: attacked by Black queen on c1
        // - f3: undefended Black pawn (only legal king move)
        let board = Board::from_fen("8/8/b1k2p2/4p2r/4KP2/1n3p2/8/2q5 w - - 0 1").unwrap();
        let moves = gen_all_moves(&board);
        assert_unique_moves(&moves);

        assert_eq!(uci_set(&moves_from(&moves, "e4")), uci_set_from(&["e4f3"]));
    }

    #[test]
    fn king_cannot_step_backward_along_checking_slider_xray_rays() {
        let ortho = Board::from_fen("4r2k/8/8/8/4K3/8/8/8 w - - 0 1").unwrap();
        let ortho_k = uci_set(&moves_from(&gen_all_moves(&ortho), "e4"));
        assert_eq!(
            ortho_k,
            uci_set_from(&["e4d5", "e4d4", "e4d3", "e4f5", "e4f4", "e4f3"])
        );

        let diag = Board::from_fen("7k/6b1/8/8/3K4/8/8/8 w - - 0 1").unwrap();
        let diag_k = uci_set(&moves_from(&gen_all_moves(&diag), "d4"));
        assert_eq!(
            diag_k,
            uci_set_from(&["d4c5", "d4d5", "d4c4", "d4e4", "d4d3", "d4e3"])
        );
    }
}

mod pawn_moves {
    use super::*;

    #[test]
    fn single_and_double_pushes_for_both_colors_and_non_start_ranks() {
        let white = Board::from_fen("4k3/8/8/8/8/2P5/4P3/4K3 w - - 0 1").unwrap();
        let w_moves = gen_all_moves(&white);
        assert_eq!(
            uci_set(&moves_from(&w_moves, "e2")),
            uci_set_from(&["e2e3", "e2e4"])
        );
        assert_eq!(
            uci_set(&moves_from(&w_moves, "c3")),
            uci_set_from(&["c3c4"])
        );

        let black = Board::from_fen("4k3/4p3/2p5/8/8/8/8/4K3 b - - 0 1").unwrap();
        let b_moves = gen_all_moves(&black);
        assert_eq!(
            uci_set(&moves_from(&b_moves, "e7")),
            uci_set_from(&["e7e6", "e7e5"])
        );
        assert_eq!(
            uci_set(&moves_from(&b_moves, "c6")),
            uci_set_from(&["c6c5"])
        );
    }

    #[test]
    fn forward_blockers_prevent_single_and_double_pushes() {
        let board = Board::from_fen("4k3/8/8/8/4r3/N1p5/P1P1P3/4K3 w - - 0 1").unwrap();
        let moves = gen_all_moves(&board);

        assert!(moves_from(&moves, "a2").is_empty());
        assert!(moves_from(&moves, "c2").is_empty());
        assert_eq!(uci_set(&moves_from(&moves, "e2")), uci_set_from(&["e2e3"]));
    }

    #[test]
    fn diagonal_captures_work_for_both_colors_without_edge_wrap() {
        let white = Board::from_fen("4k3/8/8/p1p1p2p/P2P3P/8/8/4K3 w - - 0 1").unwrap();
        let w_moves = gen_all_moves(&white);
        assert!(moves_from(&w_moves, "a4").is_empty());
        assert!(moves_from(&w_moves, "h4").is_empty());
        assert_eq!(
            uci_set(&moves_from(&w_moves, "d4")),
            uci_set_from(&["d4c5", "d4d5", "d4e5"])
        );

        let black = Board::from_fen("4k3/8/8/p2p3p/P1P1P2P/8/8/4K3 b - - 0 1").unwrap();
        let b_moves = gen_all_moves(&black);
        assert!(moves_from(&b_moves, "a5").is_empty());
        assert!(moves_from(&b_moves, "h5").is_empty());
        assert_eq!(
            uci_set(&moves_from(&b_moves, "d5")),
            uci_set_from(&["d5c4", "d5d4", "d5e4"])
        );
    }

    #[test]
    fn promotions_generate_all_four_pieces_for_quiet_and_capture_both_colors() {
        let white = Board::from_fen("3r4/4P3/8/8/8/8/8/k6K w - - 0 1").unwrap();
        assert_eq!(
            uci_set(&moves_from(&gen_all_moves(&white), "e7")),
            uci_set_from(&[
                "e7e8q", "e7e8r", "e7e8b", "e7e8n", "e7d8q", "e7d8r", "e7d8b", "e7d8n"
            ])
        );

        let black = Board::from_fen("k6K/8/8/8/8/8/4p3/3R4 b - - 0 1").unwrap();
        assert_eq!(
            uci_set(&moves_from(&gen_all_moves(&black), "e2")),
            uci_set_from(&[
                "e2e1q", "e2e1r", "e2e1b", "e2e1n", "e2d1q", "e2d1r", "e2d1b", "e2d1n"
            ])
        );
    }
}

mod en_passant {
    use super::*;

    #[test]
    fn en_passant_generated_from_both_adjacent_files_for_both_colors() {
        let white = Board::from_fen("4k3/8/8/2PpP2P/8/8/8/4K3 w - d6 0 1").unwrap();
        let w_moves = gen_all_moves(&white);
        assert!(has_uci(&w_moves, "c5d6"));
        assert!(has_uci(&w_moves, "e5d6"));
        assert!(!has_uci(&w_moves, "h5d6"));

        let black = Board::from_fen("4k3/8/8/8/2pPp2p/8/8/4K3 b - d3 0 1").unwrap();
        let b_moves = gen_all_moves(&black);
        assert!(has_uci(&b_moves, "c4d3"));
        assert!(has_uci(&b_moves, "e4d3"));
        assert!(!has_uci(&b_moves, "h4d3"));
    }

    #[test]
    fn en_passant_rejected_on_horizontal_rank_pin_for_both_colors() {
        let white = Board::from_fen("7k/8/8/K1Pp3r/8/8/8/8 w - d6 0 1").unwrap();
        assert!(!has_uci(&gen_all_moves(&white), "c5d6"));

        let black = Board::from_fen("8/8/8/8/k1pP3R/8/8/7K b - d3 0 1").unwrap();
        assert!(!has_uci(&gen_all_moves(&black), "c4d3"));
    }

    #[test]
    fn en_passant_rejected_when_diagonal_or_vertical_pin_exposed() {
        // Removing White pawn on d5 opens the a8-f3 diagonal to Black bishop on a8.
        let diag_moving = Board::from_fen("b6k/8/8/3Pp3/8/5K2/8/8 w - e6 0 1").unwrap();
        assert!(!has_uci(&gen_all_moves(&diag_moving), "d5e6"));

        // Removing captured Black pawn on e5 opens the b8-g3 diagonal to Black bishop on b8.
        let diag_captured = Board::from_fen("1b5k/8/8/3Pp3/8/6K1/8/8 w - e6 0 1").unwrap();
        assert!(!has_uci(&gen_all_moves(&diag_captured), "d5e6"));

        // Moving d5 off the d-file exposes king on d1 to Black rook on d8.
        let vertical = Board::from_fen("3r3k/8/8/3Pp3/8/8/8/3K4 w - e6 0 1").unwrap();
        assert!(!has_uci(&gen_all_moves(&vertical), "d5e6"));
    }

    #[test]
    fn en_passant_allowed_when_staying_on_diagonal_pin_ray() {
        let board = Board::from_fen("6bk/8/8/3Pp3/8/8/K7/8 w - e6 0 1").unwrap();
        assert!(has_uci(&gen_all_moves(&board), "d5e6"));
    }

    #[test]
    fn en_passant_resolves_direct_pawn_check_but_rejects_unresolved_checks() {
        // Double-pushed Black pawn on e5 directly checks White king on d6.
        let resolves = Board::from_fen("7k/8/3K4/3Pp3/8/8/8/8 w - e6 0 1").unwrap();
        assert!(has_uci(&gen_all_moves(&resolves), "d5e6"));

        // Unresolved rook check along rank 7.
        let rook_check = Board::from_fen("8/r5K1/8/3Pp3/8/8/8/7k w - e6 0 1").unwrap();
        assert!(!has_uci(&gen_all_moves(&rook_check), "d5e6"));

        // Unresolved knight check on king e1.
        let knight_check = Board::from_fen("7k/8/8/3Pp3/8/5n2/8/4K3 w - e6 0 1").unwrap();
        assert!(!has_uci(&gen_all_moves(&knight_check), "d5e6"));

        // Unresolved check by a different Black pawn on d2.
        let other_pawn_check = Board::from_fen("7k/8/8/3Pp3/8/8/3p4/4K3 w - e6 0 1").unwrap();
        assert!(!has_uci(&gen_all_moves(&other_pawn_check), "d5e6"));
    }
}

mod castling {
    use super::*;

    #[test]
    fn castling_generated_for_both_sides_of_white_and_black() {
        let white = Board::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
        let w_moves = gen_all_moves(&white);
        assert!(has_uci(&w_moves, "e1g1"));
        assert!(has_uci(&w_moves, "e1c1"));

        let black = Board::from_fen("r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1").unwrap();
        let b_moves = gen_all_moves(&black);
        assert!(has_uci(&b_moves, "e8g8"));
        assert!(has_uci(&b_moves, "e8c8"));
    }

    #[test]
    fn castling_disallowed_when_rights_unset() {
        let white_q_only = Board::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w Q - 0 1").unwrap();
        let w_moves = gen_all_moves(&white_q_only);
        assert!(!has_uci(&w_moves, "e1g1"));
        assert!(has_uci(&w_moves, "e1c1"));

        let black_k_only = Board::from_fen("r3k2r/8/8/8/8/8/8/R3K2R b k - 0 1").unwrap();
        let b_moves = gen_all_moves(&black_k_only);
        assert!(has_uci(&b_moves, "e8g8"));
        assert!(!has_uci(&b_moves, "e8c8"));
    }

    #[test]
    fn castling_disallowed_while_in_check() {
        let white = Board::from_fen("r3k2r/8/8/8/4r3/8/8/R3K2R w KQkq - 0 1").unwrap();
        let w_moves = gen_all_moves(&white);
        assert!(!has_uci(&w_moves, "e1g1"));
        assert!(!has_uci(&w_moves, "e1c1"));

        let black = Board::from_fen("r3k2r/8/8/8/4R3/8/8/R3K2R b KQkq - 0 1").unwrap();
        let b_moves = gen_all_moves(&black);
        assert!(!has_uci(&b_moves, "e8g8"));
        assert!(!has_uci(&b_moves, "e8c8"));
    }

    #[test]
    fn castling_disallowed_when_any_between_square_is_occupied() {
        let cases = [
            ("r3k2r/8/8/8/8/8/8/R3KB1R w KQkq - 0 1", "e1g1", "e1c1"),
            ("r3k2r/8/8/8/8/8/8/R3K1NR w KQkq - 0 1", "e1g1", "e1c1"),
            ("r3k2r/8/8/8/8/8/8/RN2K2R w KQkq - 0 1", "e1c1", "e1g1"),
            ("r3k2r/8/8/8/8/8/8/R1B1K2R w KQkq - 0 1", "e1c1", "e1g1"),
            ("r3k2r/8/8/8/8/8/8/R2QK2R w KQkq - 0 1", "e1c1", "e1g1"),
            ("r3kb1r/8/8/8/8/8/8/R3K2R b KQkq - 0 1", "e8g8", "e8c8"),
            ("r3k1nr/8/8/8/8/8/8/R3K2R b KQkq - 0 1", "e8g8", "e8c8"),
            ("rn2k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1", "e8c8", "e8g8"),
            ("r1b1k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1", "e8c8", "e8g8"),
            ("r2qk2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1", "e8c8", "e8g8"),
        ];

        for (fen, blocked, allowed) in cases {
            let board = Board::from_fen(fen).unwrap();
            let moves = gen_all_moves(&board);
            assert!(
                !has_uci(&moves, blocked),
                "{blocked} should be blocked in {fen}"
            );
            assert!(
                has_uci(&moves, allowed),
                "{allowed} should remain legal in {fen}"
            );
        }
    }

    #[test]
    fn castling_disallowed_when_transit_or_destination_square_attacked() {
        let cases = [
            ("r3k2r/8/8/8/5r2/8/8/R3K2R w KQkq - 0 1", "e1g1", "e1c1"),
            ("r3k2r/8/8/8/6r1/8/8/R3K2R w KQkq - 0 1", "e1g1", "e1c1"),
            ("r3k2r/8/8/8/3r4/8/8/R3K2R w KQkq - 0 1", "e1c1", "e1g1"),
            ("r3k2r/8/8/8/2r5/8/8/R3K2R w KQkq - 0 1", "e1c1", "e1g1"),
            ("r3k2r/8/8/8/5R2/8/8/R3K2R b KQkq - 0 1", "e8g8", "e8c8"),
            ("r3k2r/8/8/8/6R1/8/8/R3K2R b KQkq - 0 1", "e8g8", "e8c8"),
            ("r3k2r/8/8/8/3R4/8/8/R3K2R b KQkq - 0 1", "e8c8", "e8g8"),
            ("r3k2r/8/8/8/2R5/8/8/R3K2R b KQkq - 0 1", "e8c8", "e8g8"),
        ];

        for (fen, blocked, allowed) in cases {
            let board = Board::from_fen(fen).unwrap();
            let moves = gen_all_moves(&board);
            assert!(
                !has_uci(&moves, blocked),
                "{blocked} should be blocked in {fen}"
            );
            assert!(
                has_uci(&moves, allowed),
                "{allowed} should remain legal in {fen}"
            );
        }
    }

    #[test]
    fn castling_allowed_when_b_file_or_rook_squares_are_attacked() {
        let white = Board::from_fen("4k3/8/8/8/rr5r/8/8/R3K2R w KQ - 0 1").unwrap();
        let w_moves = gen_all_moves(&white);
        assert!(has_uci(&w_moves, "e1c1"));
        assert!(has_uci(&w_moves, "e1g1"));

        let black = Board::from_fen("r3k2r/8/8/8/RR5R/8/8/4K3 b kq - 0 1").unwrap();
        let b_moves = gen_all_moves(&black);
        assert!(has_uci(&b_moves, "e8c8"));
        assert!(has_uci(&b_moves, "e8g8"));
    }
}

mod pins {
    use super::*;

    #[test]
    fn orthogonal_pin_restricts_rook_queen_pawn_and_immobilizes_bishop_knight() {
        let rook_pin = Board::from_fen("4r2k/8/8/8/4R3/8/8/4K3 w - - 0 1").unwrap();
        assert_eq!(
            uci_set(&moves_from(&gen_all_moves(&rook_pin), "e4")),
            uci_set_from(&["e4e2", "e4e3", "e4e5", "e4e6", "e4e7", "e4e8"])
        );

        let rank_pin = Board::from_fen("7k/8/8/8/8/8/8/r2QK3 w - - 0 1").unwrap();
        assert_eq!(
            uci_set(&moves_from(&gen_all_moves(&rank_pin), "d1")),
            uci_set_from(&["d1c1", "d1b1", "d1a1"])
        );

        let pawn_pin = Board::from_fen("4r2k/8/8/8/8/3p1p2/4P3/4K3 w - - 0 1").unwrap();
        assert_eq!(
            uci_set(&moves_from(&gen_all_moves(&pawn_pin), "e2")),
            uci_set_from(&["e2e3", "e2e4"])
        );

        let bishop_pin = Board::from_fen("4r2k/8/8/8/4B3/8/8/4K3 w - - 0 1").unwrap();
        assert!(moves_from(&gen_all_moves(&bishop_pin), "e4").is_empty());

        let knight_pin = Board::from_fen("4r2k/8/8/8/4N3/8/8/4K3 w - - 0 1").unwrap();
        assert!(moves_from(&gen_all_moves(&knight_pin), "e4").is_empty());
    }

    #[test]
    fn diagonal_pin_restricts_bishop_pawn_and_immobilizes_rook() {
        let bishop_pin = Board::from_fen("7b/8/8/8/3B4/8/8/K6k w - - 0 1").unwrap();
        assert_eq!(
            uci_set(&moves_from(&gen_all_moves(&bishop_pin), "d4")),
            uci_set_from(&["d4b2", "d4c3", "d4e5", "d4f6", "d4g7", "d4h8"])
        );

        let pawn_pin = Board::from_fen("6b1/5P2/8/8/8/8/K6k/8 w - - 0 1").unwrap();
        assert_eq!(
            uci_set(&moves_from(&gen_all_moves(&pawn_pin), "f7")),
            uci_set_from(&["f7g8q", "f7g8r", "f7g8b", "f7g8n"])
        );

        let rook_pin = Board::from_fen("7b/8/8/8/3R4/8/8/K6k w - - 0 1").unwrap();
        assert!(moves_from(&gen_all_moves(&rook_pin), "d4").is_empty());
    }

    #[test]
    fn two_friendly_pieces_between_king_and_slider_are_not_pinned() {
        let board = Board::from_fen("4r2k/8/8/8/4N3/8/4R3/4K3 w - - 0 1").unwrap();
        let moves = gen_all_moves(&board);

        assert!(!moves_from(&moves, "e4").is_empty());
        assert!(has_uci(&moves, "e2d2"));
        assert!(has_uci(&moves, "e2f2"));
    }

    #[test]
    fn enemy_piece_shielding_enemy_slider_does_not_pin_friendly_piece() {
        let board = Board::from_fen("4r2k/8/4p3/8/4N3/8/8/4K3 w - - 0 1").unwrap();
        assert_eq!(moves_from(&gen_all_moves(&board), "e4").len(), 8);
    }
}

mod check_evasions {
    use super::*;

    #[test]
    fn single_slider_check_allows_flee_capture_and_double_pawn_push_block() {
        let board = Board::from_fen("7k/8/5N2/7q/8/8/4K1P1/8 w - - 0 1").unwrap();
        let moves = gen_all_moves(&board);
        assert_unique_moves(&moves);

        assert!(has_uci(&moves, "g2g4"));
        assert!(!has_uci(&moves, "g2g3"));
        assert!(has_uci(&moves, "f6h5"));
        assert!(!has_uci(&moves, "f6e4"));
        assert!(has_uci(&moves, "e2d2"));
        assert!(!has_uci(&moves, "e2f3"));
    }

    #[test]
    fn single_leaper_check_forbids_blocking_and_allows_only_capture_or_king_move() {
        let board = Board::from_fen("7k/8/8/8/8/2R2n2/8/B3K3 w - - 0 1").unwrap();
        let moves = gen_all_moves(&board);

        assert_eq!(uci_set(&moves_from(&moves, "c3")), uci_set_from(&["c3f3"]));
        assert!(moves_from(&moves, "a1").is_empty());
        assert!(!moves_from(&moves, "e1").is_empty());
    }

    #[test]
    fn pinned_piece_cannot_capture_or_block_unrelated_check() {
        let board = Board::from_fen("4r2k/8/8/8/8/4Rn2/8/4K3 w - - 0 1").unwrap();
        let moves = gen_all_moves(&board);

        assert!(moves_from(&moves, "e3").is_empty());
    }

    #[test]
    fn double_check_permits_only_king_evasions_even_when_ep_square_exists() {
        let board = Board::from_fen("5b1k/8/3K4/3Pp3/8/8/8/R7 w - e6 0 1").unwrap();
        let moves = gen_all_moves(&board);
        assert_unique_moves(&moves);

        assert!(!moves.is_empty());
        assert!(!has_uci(&moves, "d5e6"));
        assert!(moves.iter().all(|m| m.from() == sq("d6")));
    }
}

mod qsearch {
    use super::*;

    #[test]
    fn qsearch_when_not_in_check_generates_only_captures_ep_and_promotions() {
        let board = Board::from_fen("1n2r1k1/P7/8/3pP3/8/2N5/3nB3/R3K2R w KQ d6 0 1").unwrap();
        let q_moves = gen_qsearch_moves(&board);
        assert_unique_moves(&q_moves);

        let q_ucis = uci_set(&q_moves);
        let expected = uci_set_from(&[
            "c3d5", "e5d6", "e1d2", "a7a8q", "a7a8r", "a7a8b", "a7a8n", "a7b8q", "a7b8r", "a7b8b",
            "a7b8n",
        ]);
        assert_eq!(q_ucis, expected);

        let black = Board::from_fen("4k3/8/8/8/8/8/4p3/K7 b - - 0 1").unwrap();
        assert_eq!(
            uci_set(&gen_qsearch_moves(&black)),
            uci_set_from(&["e2e1q", "e2e1r", "e2e1b", "e2e1n"])
        );
    }

    #[test]
    fn qsearch_when_in_check_matches_full_legal_evasion_set() {
        let board = Board::from_fen("4r2k/8/8/8/8/8/2B5/3NK3 w - - 0 1").unwrap();
        let all = gen_all_moves(&board);
        let q_evasions = gen_qsearch_moves(&board);

        assert_unique_moves(&q_evasions);
        assert_eq!(uci_set(&q_evasions), uci_set(&all));
        assert!(has_uci(&q_evasions, "d1e3"));
        assert!(has_uci(&q_evasions, "e1f1"));
    }
}

mod perft_suite {
    use super::*;

    #[test]
    fn cpw_position_1_startpos() {
        let mut board = Board::default();
        assert_eq!(perft(&mut board, 1), 20);
        assert_eq!(perft(&mut board, 2), 400);
        assert_eq!(perft(&mut board, 3), 8_902);
        assert_eq!(perft(&mut board, 4), 197_281);
    }

    #[test]
    fn cpw_position_2_kiwipete_with_full_invariant_checks() {
        let mut board =
            Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1")
                .unwrap();
        assert_eq!(perft_checked(&mut board, 2), 2_039);
        assert_eq!(perft(&mut board, 3), 97_862);
    }

    #[test]
    fn cpw_position_3_endgame_ep_and_pins() {
        let mut board = Board::from_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1").unwrap();
        assert_eq!(perft_checked(&mut board, 3), 2_812);
        assert_eq!(perft(&mut board, 4), 43_238);
    }

    #[test]
    fn cpw_position_4_promotions_and_mirrored_black_to_move() {
        let mut white =
            Board::from_fen("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1")
                .unwrap();
        assert_eq!(perft_checked(&mut white, 2), 264);
        assert_eq!(perft(&mut white, 3), 9_467);

        let mut black =
            Board::from_fen("r2q1rk1/pP1p2pp/Q4n2/bbp1p3/Np6/1B3NBn/pPPP1PPP/R3K2R b KQ - 0 1")
                .unwrap();
        assert_eq!(perft_checked(&mut black, 2), 264);
        assert_eq!(perft(&mut black, 3), 9_467);
    }

    #[test]
    fn cpw_position_5_discovered_checks_and_castling() {
        let mut board =
            Board::from_fen("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8").unwrap();
        assert_eq!(perft_checked(&mut board, 2), 1_486);
        assert_eq!(perft(&mut board, 3), 62_379);
    }

    #[test]
    fn cpw_position_6_middlegame() {
        let mut board = Board::from_fen(
            "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        )
        .unwrap();
        assert_eq!(perft(&mut board, 1), 46);
        assert_eq!(perft(&mut board, 2), 2_079);
        assert_eq!(perft(&mut board, 3), 89_890);
    }
}
