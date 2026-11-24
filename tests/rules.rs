mod common;

use std::str::FromStr;

use common::{assert_board_invariants, parse_mv};
use minichess::chess::piece::{Color, Kind};
use minichess::chess::{Move, Piece, Square};
use minichess::engine::search::{
    MATE_THRESHOLD, SearchContext, TranspositionTable, alpha_beta_search,
};
use minichess::move_gen::{gen_all_moves, is_in_check};
use minichess::position::Board;

fn sq(name: &str) -> Square {
    Square::from_str(name).expect("valid square")
}

fn play_sequence(board: &mut Board, moves: &[&str]) -> Vec<Move> {
    let mut played = Vec::with_capacity(moves.len());
    for &uci in moves {
        let resolved = board
            .try_make_move(parse_mv(uci))
            .unwrap_or_else(|e| panic!("failed to play legal move {uci}: {e}"));
        assert_board_invariants(board);
        played.push(resolved);
    }
    played
}

mod fen_parsing_and_validation {
    use super::*;

    #[test]
    fn fen_round_trip_preserves_all_six_fields() {
        let fen = "r3k2r/pppq1ppp/2npbn2/4p3/2B1P3/2NP1N2/PPP2PPP/R1BQK2R b Kq e3 17 42";
        let board = Board::from_fen(fen).expect("valid FEN");

        assert_eq!(board.to_fen(), fen);
        assert_eq!(board.side_to_move, Color::Black);
        assert!(board.can_castle_kingside(Color::White));
        assert!(!board.can_castle_queenside(Color::White));
        assert!(!board.can_castle_kingside(Color::Black));
        assert!(board.can_castle_queenside(Color::Black));
        assert_eq!(board.state.ep_square, Some(sq("e3")));
        assert_eq!(board.state.halfmove_clock, 17);
        assert_eq!(board.fullmove_counter, 42);
        assert_board_invariants(&board);
    }

    #[test]
    fn fen_defaults_omitted_move_counters() {
        let board = Board::from_fen("4k3/8/8/8/8/8/8/4K3 w - -").expect("valid 4-field FEN");

        assert_eq!(board.state.halfmove_clock, 0);
        assert_eq!(board.fullmove_counter, 1);
        assert_eq!(board.to_fen(), "4k3/8/8/8/8/8/8/4K3 w - - 0 1");
    }

    #[test]
    fn fen_rejects_syntactically_invalid_records() {
        let invalid_fens = [
            "",                                    // empty string
            "4k3/8/8/8/8/8/8/4K3",                 // missing side to move
            "4k3/8/8/8/8/8/8/4K3 w",               // missing castling
            "4k3/8/8/8/8/8/8/4K3 w -",             // missing EP square
            "4k3/8/8/8/8/8/8/4K3 w - - 0 1 extra", // trailing garbage
            "4k3/8/8/8/8/8/4K3 w - - 0 1",         // 7 ranks
            "4k3/8/8/8/8/8/8/8/4K3 w - - 0 1",     // 9 ranks
            "4k2/8/8/8/8/8/8/4K3 w - - 0 1",       // rank too short
            "4k4/8/8/8/8/8/8/4K3 w - - 0 1",       // rank too long
            "4k3/8/8/8/8/8/8/4K30 w - - 0 1",      // digit 0
            "4k3/9/8/8/8/8/8/4K3 w - - 0 1",       // digit 9
            "4k3/8/8/8/8/8/8/4K2Z w - - 0 1",      // unknown piece char
            "4k3/8/8/8/8/8/8/4K3 g - - 0 1",       // invalid side to move
            "4k3/8/8/8/8/8/8/4K3 w X - 0 1",       // invalid castling char
            "r3k2r/8/8/8/8/8/8/R3K2R w KK - 0 1",  // duplicate castling char
            "4k3/8/8/8/8/8/8/4K3 w - z9 0 1",      // invalid EP square
            "4k3/8/8/8/8/8/8/4K3 w - - abc 1",     // non-numeric halfmove
            "4k3/8/8/8/8/8/8/4K3 w - - 0 xyz",     // non-numeric fullmove
            "4k3/8/8/8/8/8/8/4K3 w - - 0 0",       // zero fullmove counter
        ];

        for fen in invalid_fens {
            assert!(
                Board::from_fen(fen).is_err(),
                "expected FEN syntax error for: {fen}"
            );
        }
    }

    #[test]
    fn fen_rejects_illegal_chess_positions() {
        let illegal_fens = [
            "4k3/8/8/8/8/8/8/8 w - - 0 1",        // missing White king
            "8/8/8/8/8/8/8/4K3 w - - 0 1",        // missing Black king
            "4k3/8/8/8/8/8/8/3KK3 w - - 0 1",     // two White kings
            "8/8/8/8/4kK2/8/8/8 w - - 0 1",       // adjacent kings
            "4k3/8/8/8/4R3/8/8/4K3 w - - 0 1",    // opponent king in check
            "4k2P/8/8/8/8/8/8/4K3 w - - 0 1",     // White pawn on rank 8
            "4k3/8/8/8/8/8/8/4K2p w - - 0 1",     // Black pawn on rank 1
            "4k3/8/8/8/8/8/8/4K3 w K - 0 1",      // K right without h1 rook
            "r3k3/8/8/8/8/8/8/R2K4 w Q - 0 1",    // Q right without king on e1
            "4k2r/8/8/8/8/8/8/4K3 b q - 0 1",     // q right without a8 rook
            "4k3/8/8/4p3/8/8/8/4K3 w - e3 0 1",   // EP on rank 3 when White to move
            "4k3/8/8/8/4P3/8/8/4K3 b - e6 0 1",   // EP on rank 6 when Black to move
            "4k3/8/8/8/8/8/8/4K3 w - e6 0 1",     // EP without pushed Black pawn on e5
            "4k3/8/4n3/4p3/8/8/8/4K3 w - e6 0 1", // EP target square occupied
            "4k3/4n3/8/4p3/8/8/8/4K3 w - e6 0 1", // pawn origin square e7 occupied
        ];

        for fen in illegal_fens {
            assert!(
                Board::from_fen(fen).is_err(),
                "expected illegal position rejection for: {fen}"
            );
        }
    }
}

mod move_execution_and_legality {
    use super::*;

    #[test]
    fn quiet_moves_update_turn_halfmove_clock_and_fullmove_counter() {
        let mut board = Board::default();
        let initial_fen = board.to_fen();

        let w_mv = board.try_make_move(parse_mv("g1f3")).unwrap();
        assert_eq!(board.side_to_move, Color::Black);
        assert_eq!(board.state.halfmove_clock, 1);
        assert_eq!(board.fullmove_counter, 1);
        assert_board_invariants(&board);

        let b_mv = board.try_make_move(parse_mv("g8f6")).unwrap();
        assert_eq!(board.side_to_move, Color::White);
        assert_eq!(board.state.halfmove_clock, 2);
        assert_eq!(board.fullmove_counter, 2);
        assert_board_invariants(&board);

        board.try_unmake_move(b_mv).unwrap();
        board.try_unmake_move(w_mv).unwrap();
        assert_eq!(board.to_fen(), initial_fen);
        assert_board_invariants(&board);
    }

    #[test]
    fn pawn_push_resets_halfmove_clock_and_clears_ep_square() {
        let mut board = Board::from_fen("4k3/8/8/3p4/8/8/4P3/4K3 w - d6 12 7").unwrap();
        let initial_fen = board.to_fen();

        let mv = board.try_make_move(parse_mv("e2e3")).unwrap();
        assert_eq!(board.state.halfmove_clock, 0);
        assert_eq!(board.state.ep_square, None);
        assert_board_invariants(&board);

        board.try_unmake_move(mv).unwrap();
        assert_eq!(board.to_fen(), initial_fen);
    }

    #[test]
    fn double_pawn_push_sets_ep_square_for_both_colors() {
        let mut board = Board::default();
        let initial_fen = board.to_fen();

        let w_push = board.try_make_move(parse_mv("e2e4")).unwrap();
        assert_eq!(board.state.ep_square, Some(sq("e3")));
        assert_board_invariants(&board);

        let b_push = board.try_make_move(parse_mv("d7d5")).unwrap();
        assert_eq!(board.state.ep_square, Some(sq("d6")));
        assert_board_invariants(&board);

        board.try_unmake_move(b_push).unwrap();
        assert_eq!(board.state.ep_square, Some(sq("e3")));
        board.try_unmake_move(w_push).unwrap();
        assert_eq!(board.to_fen(), initial_fen);
    }

    #[test]
    fn regular_capture_removes_target_and_resets_halfmove_clock() {
        let mut board = Board::from_fen("4k3/8/3p4/8/4N3/8/8/4K3 w - - 15 10").unwrap();
        let initial_fen = board.to_fen();

        let cap = board.try_make_move(parse_mv("e4d6")).unwrap();
        assert_eq!(board.state.halfmove_clock, 0);
        assert_eq!(board.piece_at(sq("d6")), Some(Piece::WKnight));
        assert_eq!(board.piece_at(sq("e4")), None);
        assert_board_invariants(&board);

        board.try_unmake_move(cap).unwrap();
        assert_eq!(board.to_fen(), initial_fen);
    }

    #[test]
    fn en_passant_capture_removes_adjacent_pawn_for_both_colors() {
        let mut white_board = Board::from_fen("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1").unwrap();
        let white_initial = white_board.to_fen();
        let w_ep = white_board.try_make_move(parse_mv("e5d6")).unwrap();
        assert_eq!(white_board.piece_at(sq("d6")), Some(Piece::WPawn));
        assert_eq!(white_board.piece_at(sq("d5")), None);
        assert_board_invariants(&white_board);
        white_board.try_unmake_move(w_ep).unwrap();
        assert_eq!(white_board.to_fen(), white_initial);

        let mut black_board = Board::from_fen("4k3/8/8/8/3pP3/8/8/4K3 b - e3 0 1").unwrap();
        let black_initial = black_board.to_fen();
        let b_ep = black_board.try_make_move(parse_mv("d4e3")).unwrap();
        assert_eq!(black_board.piece_at(sq("e3")), Some(Piece::BPawn));
        assert_eq!(black_board.piece_at(sq("e4")), None);
        assert_board_invariants(&black_board);
        black_board.try_unmake_move(b_ep).unwrap();
        assert_eq!(black_board.to_fen(), black_initial);
    }

    #[test]
    fn castling_relocates_king_and_rook_for_all_four_sides() {
        let cases = [
            (
                "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1",
                "e1g1",
                "r3k2r/8/8/8/8/8/8/R4RK1 b kq - 1 1",
            ),
            (
                "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1",
                "e1c1",
                "r3k2r/8/8/8/8/8/8/2KR3R b kq - 1 1",
            ),
            (
                "r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1",
                "e8g8",
                "r4rk1/8/8/8/8/8/8/R3K2R w KQ - 1 2",
            ),
            (
                "r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1",
                "e8c8",
                "2kr3r/8/8/8/8/8/8/R3K2R w KQ - 1 2",
            ),
        ];

        for (fen, uci, expected_fen) in cases {
            let mut board = Board::from_fen(fen).unwrap();
            let mv = board.try_make_move(parse_mv(uci)).unwrap();
            assert_eq!(board.to_fen(), expected_fen);
            assert_board_invariants(&board);

            board.try_unmake_move(mv).unwrap();
            assert_eq!(board.to_fen(), fen);
        }
    }

    #[test]
    fn promotion_push_and_capture_replace_pawn_for_all_piece_kinds() {
        let kinds = [
            ('q', Kind::Queen),
            ('r', Kind::Rook),
            ('b', Kind::Bishop),
            ('n', Kind::Knight),
        ];

        let mut white_board = Board::from_fen("3r4/4P3/8/8/8/8/8/k6K w - - 5 1").unwrap();
        let w_initial = white_board.to_fen();
        for (ch, kind) in kinds {
            let push = white_board
                .try_make_move(parse_mv(&format!("e7e8{ch}")))
                .unwrap();
            assert_eq!(
                white_board.piece_at(sq("e8")),
                Some(Piece::new(Color::White, kind))
            );
            assert_eq!(white_board.state.halfmove_clock, 0);
            assert_board_invariants(&white_board);
            white_board.try_unmake_move(push).unwrap();
            assert_eq!(white_board.to_fen(), w_initial);

            let cap = white_board
                .try_make_move(parse_mv(&format!("e7d8{ch}")))
                .unwrap();
            assert_eq!(
                white_board.piece_at(sq("d8")),
                Some(Piece::new(Color::White, kind))
            );
            assert_board_invariants(&white_board);
            white_board.try_unmake_move(cap).unwrap();
            assert_eq!(white_board.to_fen(), w_initial);
        }

        let mut black_board = Board::from_fen("k6K/8/8/8/8/8/4p3/3R4 b - - 3 1").unwrap();
        let b_initial = black_board.to_fen();
        let b_promo = black_board.try_make_move(parse_mv("e2d1n")).unwrap();
        assert_eq!(black_board.piece_at(sq("d1")), Some(Piece::BKnight));
        assert_board_invariants(&black_board);
        black_board.try_unmake_move(b_promo).unwrap();
        assert_eq!(black_board.to_fen(), b_initial);
    }

    #[test]
    fn rejects_all_illegal_move_and_unmake_attempts() {
        let illegal_cases = [
            // Empty source square.
            (
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                "e4e5",
            ),
            // Moving opponent piece.
            (
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                "e7e5",
            ),
            // Illegal knight geometry.
            ("4k3/8/8/3p4/4N3/8/8/4K3 w - - 0 1", "e4d5"),
            // Capturing own piece.
            ("4k3/8/8/8/8/8/4P3/4K3 w - - 0 1", "e1e2"),
            // Moving king into check.
            ("4r2k/8/8/8/8/8/8/3K4 w - - 0 1", "d1e1"),
            // Ignoring check.
            ("4r2k/8/8/8/8/8/P7/4K3 w - - 0 1", "a2a3"),
            // Moving pinned piece off pin ray.
            ("4r2k/8/8/8/4R3/8/8/4K3 w - - 0 1", "e4d4"),
            // Castling without castling rights.
            ("r3k2r/8/8/8/8/8/8/R3K2R w - - 0 1", "e1g1"),
            // Castling through an attacked square.
            ("r3k2r/8/8/8/5r2/8/8/R3K2R w KQkq - 0 1", "e1g1"),
            // Missing promotion piece on back rank (White and Black).
            ("8/4P3/8/8/8/8/8/k6K w - - 0 1", "e7e8"),
            ("k6K/8/8/8/8/8/4p3/8 b - - 0 1", "e2e1"),
            // Unexpected promotion suffix on non-promotion moves.
            (
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                "e2e4q",
            ),
            (
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                "g1f3q",
            ),
        ];

        for (fen, uci) in illegal_cases {
            let mut board = Board::from_fen(fen).unwrap();
            assert!(
                board.try_make_move(parse_mv(uci)).is_err(),
                "expected illegal move {uci} to be rejected in {fen}"
            );
            assert_eq!(board.to_fen(), fen);
        }

        // Unmake errors: empty history and mismatched destination.
        let mut board = Board::default();
        assert!(board.try_unmake_move(parse_mv("e2e4")).is_err());

        let made = board.try_make_move(parse_mv("e2e4")).unwrap();
        assert!(board.try_unmake_move(parse_mv("a2a4")).is_err());
        assert!(board.try_unmake_move(made).is_ok());
    }
}

mod castling_rights {
    use super::*;

    #[test]
    fn king_move_revokes_both_castling_rights_for_moving_side_only() {
        let mut w_board = Board::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
        w_board.try_make_move(parse_mv("e1e2")).unwrap();
        assert!(!w_board.can_castle_kingside(Color::White));
        assert!(!w_board.can_castle_queenside(Color::White));
        assert!(w_board.can_castle_kingside(Color::Black));
        assert!(w_board.can_castle_queenside(Color::Black));

        let mut b_board = Board::from_fen("r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1").unwrap();
        b_board.try_make_move(parse_mv("e8e7")).unwrap();
        assert!(b_board.can_castle_kingside(Color::White));
        assert!(b_board.can_castle_queenside(Color::White));
        assert!(!b_board.can_castle_kingside(Color::Black));
        assert!(!b_board.can_castle_queenside(Color::Black));
    }

    #[test]
    fn rook_move_revokes_only_its_own_side_castling_right() {
        let cases = [
            ("w", "h1h2", "Qkq"),
            ("w", "a1a2", "Kkq"),
            ("b", "h8h7", "KQq"),
            ("b", "a8a7", "KQk"),
        ];

        for (side, uci, expected_rights) in cases {
            let fen = format!("r3k2r/8/8/8/8/8/8/R3K2R {side} KQkq - 0 1");
            let mut board = Board::from_fen(&fen).unwrap();
            board.try_make_move(parse_mv(uci)).unwrap();
            let actual_rights = board
                .to_fen()
                .split_whitespace()
                .nth(2)
                .unwrap()
                .to_string();
            assert_eq!(actual_rights, expected_rights, "after rook move {uci}");
        }
    }

    #[test]
    fn capturing_unmoved_rook_revokes_opponent_castling_right() {
        let cases = [
            ("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", "h1h8", "Qq"),
            ("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", "a1a8", "Kk"),
            ("r3k2r/8/5B2/8/8/8/8/R3K2R w KQkq - 0 1", "f6h8", "KQq"),
            ("r3k2r/8/8/8/8/8/1b6/R3K2R b KQkq - 0 1", "b2a1", "Kkq"),
        ];

        for (fen, uci, expected_rights) in cases {
            let mut board = Board::from_fen(fen).unwrap();
            board.try_make_move(parse_mv(uci)).unwrap();
            let actual_rights = board
                .to_fen()
                .split_whitespace()
                .nth(2)
                .unwrap()
                .to_string();
            assert_eq!(actual_rights, expected_rights, "after capture {uci}");
        }
    }
}

mod terminal_conditions {
    use super::*;

    #[test]
    fn is_in_check_detects_every_attacking_piece_kind_and_respects_blockers() {
        let checked_fens = [
            "4k3/8/8/8/8/3p4/4K3/8 w - - 0 1", // black pawn
            "4k3/8/8/8/8/5n2/8/4K3 w - - 0 1", // black knight
            "4k3/8/8/8/1b6/8/8/4K3 w - - 0 1", // black bishop
            "4k3/8/8/8/4r3/8/8/4K3 w - - 0 1", // black rook
            "4k3/8/8/8/7q/8/8/4K3 w - - 0 1",  // black queen
            "8/4k3/3P4/8/8/8/8/4K3 b - - 0 1", // white pawn
            "4k3/8/3N4/8/8/8/8/4K3 b - - 0 1", // white knight
        ];
        for fen in checked_fens {
            let board = Board::from_fen(fen).unwrap();
            assert!(is_in_check(&board), "expected check in {fen}");
        }

        let shielded = Board::from_fen("4r2k/8/8/8/1b6/2P5/4P3/4K3 w - - 0 1").unwrap();
        assert!(!is_in_check(&shielded));
    }

    #[test]
    fn checkmate_has_check_and_zero_legal_moves() {
        let mates = [
            "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3",
            "6rk/5Npp/8/8/8/8/8/4K3 b - - 0 1",
        ];
        for fen in mates {
            let board = Board::from_fen(fen).unwrap();
            assert!(is_in_check(&board), "expected check in checkmate {fen}");
            assert!(gen_all_moves(&board).is_empty());
        }
    }

    #[test]
    fn stalemate_has_no_check_and_zero_legal_moves() {
        let board = Board::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").unwrap();
        assert!(!is_in_check(&board));
        assert!(gen_all_moves(&board).is_empty());
    }
}

mod draw_rules {
    use super::*;

    #[test]
    fn fifty_move_draw_triggers_at_100_halfmoves_not_99() {
        let mut board = Board::from_fen("4k3/8/8/8/8/8/8/4K2R w - - 99 60").unwrap();
        assert!(!board.is_fifty_move_draw());

        board.try_make_move(parse_mv("h1h2")).unwrap();
        assert_eq!(board.state.halfmove_clock, 100);
        assert!(board.is_fifty_move_draw());

        board.try_make_move(parse_mv("e8d8")).unwrap();
        assert_eq!(board.state.halfmove_clock, 101);
        assert!(board.is_fifty_move_draw());
    }

    #[test]
    fn pawn_move_and_capture_at_ply_99_reset_fifty_move_clock() {
        let mut pawn_board = Board::from_fen("4k3/8/8/8/8/8/4P3/4K3 w - - 99 60").unwrap();
        pawn_board.try_make_move(parse_mv("e2e3")).unwrap();
        assert_eq!(pawn_board.state.halfmove_clock, 0);
        assert!(!pawn_board.is_fifty_move_draw());

        let mut cap_board = Board::from_fen("4k3/8/8/8/8/8/7n/4K2R w - - 99 60").unwrap();
        cap_board.try_make_move(parse_mv("h1h2")).unwrap();
        assert_eq!(cap_board.state.halfmove_clock, 0);
        assert!(!cap_board.is_fifty_move_draw());
    }

    #[test]
    fn checkmate_on_fiftieth_move_overrides_fifty_move_draw_while_non_mate_check_draws() {
        let mut tt = TranspositionTable::new(1);

        let mut mated = Board::from_fen("6k1/5ppp/8/8/8/8/8/3R2K1 w - - 99 60").unwrap();
        mated.try_make_move(parse_mv("d1d8")).unwrap();
        assert!(mated.is_fifty_move_draw());
        assert!(is_in_check(&mated));
        assert!(gen_all_moves(&mated).is_empty());

        let mut ctx = SearchContext::new(None, None, &mut tt);
        let mate_score = alpha_beta_search(&mut mated, -32_000, 32_000, 2, 1, &mut ctx);
        assert!(
            mate_score <= -MATE_THRESHOLD,
            "checkmate on move 50 must override 50-move draw, got {mate_score}"
        );

        let mut drawn_check = Board::from_fen("6k1/8/8/8/8/8/8/3R2K1 w - - 99 60").unwrap();
        drawn_check.try_make_move(parse_mv("d1d8")).unwrap();
        assert!(drawn_check.is_fifty_move_draw());
        assert!(is_in_check(&drawn_check));
        assert!(!gen_all_moves(&drawn_check).is_empty());

        let mut ctx2 = SearchContext::new(None, None, &mut tt);
        let draw_score = alpha_beta_search(&mut drawn_check, -32_000, 32_000, 2, 1, &mut ctx2);
        assert_eq!(draw_score, 0);
    }

    #[test]
    fn twofold_and_threefold_repetition_track_occurrences() {
        let from_midgame_fen = Board::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 40 30").unwrap();
        assert!(!from_midgame_fen.is_twofold_repetition());

        let mut board = Board::default();
        let cycle = ["g1f3", "g8f6", "f3g1", "f6g8"];

        assert!(!board.is_twofold_repetition());
        assert!(!board.is_threefold_repetition());

        play_sequence(&mut board, &cycle);
        assert!(board.is_twofold_repetition());
        assert!(!board.is_threefold_repetition());

        play_sequence(&mut board, &cycle);
        assert!(board.is_twofold_repetition());
        assert!(board.is_threefold_repetition());
    }

    #[test]
    fn irreversible_move_prevents_repetition_across_clock_reset() {
        let mut board = Board::default();
        play_sequence(&mut board, &["a2a3", "a7a6"]);
        assert!(!board.is_twofold_repetition());

        play_sequence(&mut board, &["g1f3", "g8f6"]);
        assert!(!board.is_twofold_repetition());

        play_sequence(&mut board, &["f3g1", "f6g8"]);
        assert!(board.is_twofold_repetition());
        assert!(!board.is_threefold_repetition());
    }

    #[test]
    fn repetition_distinguishes_castling_rights_change() {
        let mut board = Board::from_fen("1n2k2r/8/8/8/8/8/8/4K2R w Kk - 0 1").unwrap();
        // First cycle loses White's kingside castling right, so it is a new position.
        play_sequence(&mut board, &["h1g1", "b8c6", "g1h1", "c6b8"]);
        assert!(!board.can_castle_kingside(Color::White));
        assert!(board.can_castle_kingside(Color::Black));
        assert!(!board.is_twofold_repetition());

        // Second cycle repeats the post-revocation position with identical rights.
        play_sequence(&mut board, &["h1g1", "b8c6", "g1h1", "c6b8"]);
        assert!(board.is_twofold_repetition());
    }

    #[test]
    fn repetition_distinguishes_legal_en_passant_availability() {
        // After e2e4, Black pawn on d4 can legally capture en passant on e3.
        let mut board = Board::from_fen("4k3/8/8/8/3p4/8/4P3/4K3 w - - 0 1").unwrap();
        play_sequence(&mut board, &["e2e4"]);
        assert!(
            gen_all_moves(&board)
                .iter()
                .any(|m| m.to_string() == "d4e3")
        );

        // Returning to the same squares without EP rights must not count as a repetition.
        play_sequence(&mut board, &["e8d8", "e1d1", "d8e8", "d1e1"]);
        assert!(!board.is_twofold_repetition());

        // Repeating again (both without EP rights) is a repetition.
        play_sequence(&mut board, &["e8d8", "e1d1", "d8e8", "d1e1"]);
        assert!(board.is_twofold_repetition());
    }

    #[test]
    fn repetition_ignores_pseudo_and_pinned_en_passant() {
        // Pseudo EP: no Black pawn adjacent to e4, so returning to the same layout is a repetition.
        let mut pseudo = Board::from_fen("4k3/8/8/3p4/8/8/4P3/4K3 w - - 0 1").unwrap();
        play_sequence(&mut pseudo, &["e2e4", "e8d8", "e1d1", "d8e8", "d1e1"]);
        assert!(pseudo.is_twofold_repetition());

        // Pinned EP: Black pawn on d4 is horizontally pinned by rook a4 to king h4, so d4e3 is illegal.
        let mut pinned = Board::from_fen("8/8/8/8/R2p3k/8/4P3/4K3 w - - 0 1").unwrap();
        play_sequence(&mut pinned, &["e2e4"]);
        assert!(
            !gen_all_moves(&pinned)
                .iter()
                .any(|m| m.to_string() == "d4e3")
        );
        play_sequence(&mut pinned, &["h4g4", "e1d1", "g4h4", "d1e1"]);
        assert!(pinned.is_twofold_repetition());
    }

    #[test]
    fn repetition_distinguishes_side_to_move() {
        let mut board = Board::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        // White triangulates to reach identical squares with Black to move.
        play_sequence(&mut board, &["e1d1", "e8d8", "d1d2", "d8e8", "d2e1"]);
        assert_eq!(board.side_to_move, Color::Black);
        assert!(!board.is_twofold_repetition());
    }

    #[test]
    fn insufficient_material_covers_all_fide_dead_positions() {
        let draw_fens = [
            "4k3/8/8/8/8/8/8/4K3 w - - 0 1",     // K vs K
            "4k3/8/8/8/8/8/8/4KB2 w - - 0 1",    // K+B vs K (light)
            "4k3/8/8/8/8/8/8/2B1K3 w - - 0 1",   // K+B vs K (dark)
            "2b1k3/8/8/8/8/8/8/4KB2 w - - 0 1",  // K+B vs K+B (both light)
            "4kb2/8/8/8/8/8/8/2B1K3 w - - 0 1",  // K+B vs K+B (both dark)
            "4k3/8/8/8/5B2/8/8/2B1K3 w - - 0 1", // K+B+B vs K (same color)
            "4k3/8/8/8/8/8/8/4K1N1 w - - 0 1",   // K+N vs K
            "4k1n1/8/8/8/8/8/8/4K3 w - - 0 1",   // K vs K+N
        ];

        for fen in draw_fens {
            let board = Board::from_fen(fen).unwrap();
            assert!(
                board.is_insufficient_material(),
                "expected insufficient material for {fen}"
            );
        }
    }

    #[test]
    fn insufficient_material_excludes_all_checkmate_possible_positions() {
        let sufficient_fens = [
            "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1",   // pawn
            "4k3/8/8/8/8/8/8/4K2R w - - 0 1",    // rook
            "4k3/8/8/8/8/8/8/4K2Q w - - 0 1",    // queen
            "2b1k3/8/8/8/8/8/8/2B1K3 w - - 0 1", // opposite-colored bishops across sides
            "4k3/8/8/8/8/8/8/2B1KB2 w - - 0 1",  // bishop pair on one side
            "4k3/8/8/8/8/8/8/1N2K1N1 w - - 0 1", // two knights on one side
            "4k1n1/8/8/8/8/8/8/4K1N1 w - - 0 1", // knight vs knight
            "4k3/8/8/8/8/8/8/4KBN1 w - - 0 1",   // bishop + knight on one side
            "4kb2/8/8/8/8/8/8/4K1N1 w - - 0 1",  // knight vs bishop
        ];

        for fen in sufficient_fens {
            let board = Board::from_fen(fen).unwrap();
            assert!(
                !board.is_insufficient_material(),
                "expected sufficient material for {fen}"
            );
        }
    }
}

mod transpositions {
    use super::*;

    #[test]
    fn transpositions_with_pseudo_ep_produce_identical_position_keys() {
        let mut line_a = Board::default();
        play_sequence(&mut line_a, &["e2e4", "e7e5", "g1f3", "b8c6"]);

        let mut line_b = Board::default();
        play_sequence(&mut line_b, &["g1f3", "b8c6", "e2e4", "e7e5"]);

        assert_eq!(line_a.state.zobrist_key, line_b.state.zobrist_key);
    }

    #[test]
    fn complex_game_unwinds_to_exact_initial_state() {
        let mut board = Board::default();
        let initial = board.clone();

        let mut played = play_sequence(
            &mut board,
            &[
                "e2e4", "a7a6", "e4e5", "d7d5", "e5d6", "b8c6", "g1f3", "c8e6", "f1c4", "d8d7",
                "e1g1", "e8c8", "d6c7",
            ],
        );

        while let Some(mv) = played.pop() {
            board.try_unmake_move(mv).unwrap();
            assert_board_invariants(&board);
        }

        assert_eq!(board, initial);
    }
}
