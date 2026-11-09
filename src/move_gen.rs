pub mod attacks;
pub mod generator;

pub use generator::gen_all_moves;
pub use generator::gen_qsearch_moves;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess::{Move, Square};
    use crate::position::Board;
    use std::str::FromStr;

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

    mod starting_position {
        use super::*;

        #[test]
        fn generates_exactly_20_moves() {
            let board = Board::default();
            let moves = gen_all_moves(&board);
            assert_eq!(moves.len(), 20);
        }

        #[test]
        fn contains_all_pawn_pushes() {
            let board = Board::default();
            let moves = gen_all_moves(&board);
            let expected_pawns = [
                "a2a3", "a2a4", "b2b3", "b2b4", "c2c3", "c2c4", "d2d3", "d2d4", "e2e3", "e2e4",
                "f2f3", "f2f4", "g2g3", "g2g4", "h2h3", "h2h4",
            ];
            for mv_str in &expected_pawns {
                assert!(moves.contains(&Move::from_str(mv_str).unwrap()));
            }
        }

        #[test]
        fn contains_all_knight_leaps() {
            let board = Board::default();
            let moves = gen_all_moves(&board);
            let expected_knights = ["b1a3", "b1c3", "g1f3", "g1h3"];
            for mv_str in &expected_knights {
                assert!(moves.contains(&Move::from_str(mv_str).unwrap()));
            }
        }

        #[test]
        fn quiescence_search_has_zero_moves() {
            let board = Board::default();
            let q_moves = gen_qsearch_moves(&board);
            assert_eq!(q_moves.len(), 0);
        }

        #[test]
        fn perft_depth_1_is_20() {
            let mut board = Board::default();
            assert_eq!(perft(&mut board, 1), 20);
        }

        #[test]
        fn perft_depth_2_is_400() {
            let mut board = Board::default();
            assert_eq!(perft(&mut board, 2), 400);
        }
    }

    mod castling {
        use super::*;

        #[test]
        fn both_kingside_and_queenside_available_on_open_board() {
            let board = Board::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert!(moves.contains(&Move::from_str("e1g1").unwrap()));
            assert!(moves.contains(&Move::from_str("e1c1").unwrap()));
        }

        #[test]
        fn king_in_check_cannot_castle() {
            let board = Board::from_fen("r3k2r/8/8/8/4r3/8/8/R3K2R w KQkq - 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert!(!moves.contains(&Move::from_str("e1g1").unwrap()));
            assert!(!moves.contains(&Move::from_str("e1c1").unwrap()));
        }

        #[test]
        fn transit_square_d1_attacked_blocks_queenside_castling() {
            let board = Board::from_fen("r3k2r/8/8/8/3r4/8/8/R3K2R w KQkq - 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert!(!moves.contains(&Move::from_str("e1c1").unwrap()));
            assert!(moves.contains(&Move::from_str("e1g1").unwrap()));
        }

        #[test]
        fn transit_square_f1_attacked_blocks_kingside_castling() {
            let board = Board::from_fen("r3k2r/8/8/8/5r2/8/8/R3K2R w KQkq - 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert!(!moves.contains(&Move::from_str("e1g1").unwrap()));
            assert!(moves.contains(&Move::from_str("e1c1").unwrap()));
        }

        #[test]
        fn pieces_between_king_and_rook_block_castling() {
            let board = Board::from_fen("r3k2r/8/8/8/8/8/8/R1B1K2R w KQkq - 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert!(!moves.contains(&Move::from_str("e1c1").unwrap()));
            assert!(moves.contains(&Move::from_str("e1g1").unwrap()));
        }

        #[test]
        fn enemy_attack_on_b1_allows_queenside_castling() {
            let board = Board::from_fen("r3k2r/8/8/8/1r6/8/8/R3K2R w KQkq - 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert!(moves.contains(&Move::from_str("e1c1").unwrap()));
            assert!(moves.contains(&Move::from_str("e1g1").unwrap()));
        }
    }

    mod check_evasion {
        use super::*;

        #[test]
        fn king_can_flee_from_single_slider_check() {
            let board = Board::from_fen("8/8/8/8/4r3/8/2B5/3NK2k w - - 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert!(moves.contains(&Move::from_str("e1d2").unwrap()));
            assert!(moves.contains(&Move::from_str("e1f2").unwrap()));
            assert!(moves.contains(&Move::from_str("e1f1").unwrap()));
            assert!(!moves.contains(&Move::from_str("e1e2").unwrap()));
        }

        #[test]
        fn knight_can_block_single_slider_check() {
            let board = Board::from_fen("8/8/8/8/4r3/8/2B5/3NK2k w - - 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert!(moves.contains(&Move::from_str("d1e3").unwrap()));
            assert!(!moves.contains(&Move::from_str("d1c3").unwrap()));
            assert!(!moves.contains(&Move::from_str("d1f2").unwrap()));
        }

        #[test]
        fn piece_can_capture_checking_slider() {
            let board = Board::from_fen("8/8/8/8/4r3/8/2B5/3NK2k w - - 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert!(moves.contains(&Move::from_str("c2e4").unwrap()));
            assert!(!moves.contains(&Move::from_str("c2b3").unwrap()));
            assert!(!moves.contains(&Move::from_str("c2d3").unwrap()));
        }

        #[test]
        fn double_check_permits_only_king_moves() {
            let board = Board::from_fen("4r3/8/8/Q7/8/3n4/8/R3K2k w - - 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert!(!moves.is_empty());
            for mv in &moves {
                assert_eq!(mv.from(), Square::from_str("e1").unwrap());
            }
        }

        #[test]
        fn double_check_disallows_capturing_checker_with_non_king() {
            let board = Board::from_fen("4r3/8/8/Q7/8/3n4/8/R3K2k w - - 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert!(!moves.contains(&Move::from_str("a5d3").unwrap()));
        }

        #[test]
        fn double_check_disallows_blocking_with_non_king() {
            let board = Board::from_fen("4r3/8/8/Q7/8/3n4/8/R3K2k w - - 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert!(!moves.contains(&Move::from_str("a1e1").unwrap()));
        }
    }

    mod absolute_pins {
        use super::*;

        #[test]
        fn rook_pinned_along_file_can_move_along_file() {
            let board = Board::from_fen("4r3/8/8/8/4R3/8/8/4K2k w - - 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert!(moves.contains(&Move::from_str("e4e2").unwrap()));
            assert!(moves.contains(&Move::from_str("e4e3").unwrap()));
            assert!(moves.contains(&Move::from_str("e4e5").unwrap()));
            assert!(moves.contains(&Move::from_str("e4e6").unwrap()));
            assert!(moves.contains(&Move::from_str("e4e7").unwrap()));
        }

        #[test]
        fn rook_pinned_along_file_can_capture_pinner() {
            let board = Board::from_fen("4r3/8/8/8/4R3/8/8/4K2k w - - 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert!(moves.contains(&Move::from_str("e4e8").unwrap()));
        }

        #[test]
        fn rook_pinned_along_file_cannot_move_laterally() {
            let board = Board::from_fen("4r3/8/8/8/4R3/8/8/4K2k w - - 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert!(!moves.contains(&Move::from_str("e4d4").unwrap()));
            assert!(!moves.contains(&Move::from_str("e4c4").unwrap()));
            assert!(!moves.contains(&Move::from_str("e4f4").unwrap()));
        }

        #[test]
        fn knight_pinned_to_king_has_no_legal_moves() {
            let board = Board::from_fen("4r3/8/8/8/4N3/8/8/4K2k w - - 0 1").unwrap();
            let moves = gen_all_moves(&board);
            for mv in &moves {
                assert_ne!(mv.from(), Square::from_str("e4").unwrap());
            }
        }

        #[test]
        fn bishop_pinned_along_diagonal_can_move_along_diagonal() {
            let board = Board::from_fen("7b/8/8/8/3B4/8/8/K6k w - - 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert!(moves.contains(&Move::from_str("d4c3").unwrap()));
            assert!(moves.contains(&Move::from_str("d4b2").unwrap()));
            assert!(moves.contains(&Move::from_str("d4e5").unwrap()));
            assert!(moves.contains(&Move::from_str("d4f6").unwrap()));
            assert!(moves.contains(&Move::from_str("d4g7").unwrap()));
        }

        #[test]
        fn bishop_pinned_along_diagonal_can_capture_pinner() {
            let board = Board::from_fen("7b/8/8/8/3B4/8/8/K6k w - - 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert!(moves.contains(&Move::from_str("d4h8").unwrap()));
        }

        #[test]
        fn bishop_pinned_along_diagonal_cannot_step_off_diagonal() {
            let board = Board::from_fen("7b/8/8/8/3B4/8/8/K6k w - - 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert!(!moves.contains(&Move::from_str("d4c5").unwrap()));
            assert!(!moves.contains(&Move::from_str("d4e3").unwrap()));
        }
    }

    mod en_passant {
        use super::*;

        #[test]
        fn standard_en_passant_capture_is_generated() {
            let board = Board::from_fen("8/8/8/3Pp3/8/8/8/4K2k w - e6 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert!(moves.contains(&Move::from_str("d5e6").unwrap()));
        }

        #[test]
        fn horizontal_pin_discovery_check_prevents_en_passant() {
            let board = Board::from_fen("8/8/8/K1Pp3r/8/8/8/7k w - d6 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert!(!moves.contains(&Move::from_str("c5d6").unwrap()));
        }

        #[test]
        fn en_passant_resolves_check_by_capturing_checker() {
            let board = Board::from_fen("8/8/3K4/3Pp3/8/8/8/7k w - e6 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert!(moves.contains(&Move::from_str("d5e6").unwrap()));
        }
    }

    mod terminal_states {
        use super::*;

        #[test]
        fn checkmate_fool_mate_generates_zero_moves() {
            let board =
                Board::from_fen("rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3")
                    .unwrap();
            let moves = gen_all_moves(&board);
            assert_eq!(moves.len(), 0);
        }

        #[test]
        fn stalemate_generates_zero_moves() {
            let board = Board::from_fen("k7/2Q5/2K5/8/8/8/8/8 b - - 0 1").unwrap();
            let moves = gen_all_moves(&board);
            assert_eq!(moves.len(), 0);
        }
    }

    mod quiescence_search {
        use super::*;

        #[test]
        fn generates_strictly_captures_when_not_in_check() {
            let board = Board::from_fen("8/8/8/3p4/4P3/8/8/4K2k w - - 0 1").unwrap();
            let q_moves = gen_qsearch_moves(&board);
            assert_eq!(q_moves.len(), 1);
            assert_eq!(q_moves[0], Move::from_str("e4d5").unwrap());
        }

        #[test]
        fn filters_out_quiet_moves_when_not_in_check() {
            let board = Board::from_fen("8/8/8/3p4/4P3/8/8/4K2k w - - 0 1").unwrap();
            let q_moves = gen_qsearch_moves(&board);
            assert!(!q_moves.contains(&Move::from_str("e4e5").unwrap()));
            assert!(!q_moves.contains(&Move::from_str("e1e2").unwrap()));
        }

        #[test]
        fn generates_all_check_evasions_when_in_check() {
            let board = Board::from_fen("4r3/8/8/8/8/8/8/4K2k w - - 0 1").unwrap();
            let all_moves = gen_all_moves(&board);
            let q_evasions = gen_qsearch_moves(&board);
            assert_eq!(q_evasions.len(), all_moves.len());
            for mv in &all_moves {
                assert!(q_evasions.contains(mv));
            }
        }
    }

    mod perft_benchmarks {
        use super::*;

        #[test]
        fn position_3_depth_1_nodes() {
            let mut board = Board::from_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1").unwrap();
            assert_eq!(perft(&mut board, 1), 14);
        }

        #[test]
        fn position_3_depth_2_nodes() {
            let mut board = Board::from_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1").unwrap();
            assert_eq!(perft(&mut board, 2), 191);
        }

        #[test]
        fn kiwipete_depth_1_nodes() {
            let mut board = Board::from_fen(
                "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            )
            .unwrap();
            assert_eq!(perft(&mut board, 1), 48);
        }
    }
}
