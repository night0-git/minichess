use std::fmt::Write;
use std::num::ParseIntError;
use thiserror::Error;

use super::board::{Board, State};
use crate::chess::piece::{Color, Kind, Piece};
use crate::chess::{Bitboard, Square, square};

pub const STARTING_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseError {
    #[error("missing FEN field: {0}")]
    MissingField(&'static str),
    #[error("unexpected extra FEN fields")]
    ExtraFields,
    #[error("invalid piece placement: expected 8 ranks, found {0}")]
    InvalidRankCount(usize),
    #[error("invalid piece placement: rank {rank} has {files} squares, expected 8")]
    InvalidRankLength { rank: usize, files: usize },
    #[error("invalid piece character '{0}' in piece placement")]
    InvalidPieceChar(char),
    #[error("invalid active color '{0}': expected 'w' or 'b'")]
    InvalidActiveColor(String),
    #[error("invalid castling flag '{0}'")]
    InvalidCastlingFlag(char),
    #[error("duplicate castling flag '{0}'")]
    DuplicateCastlingFlag(char),
    #[error("castling rights do not match king and rook placement")]
    InvalidCastlingRights,
    #[error("invalid en passant square: {0}")]
    InvalidEpSquare(#[from] square::ParseError),
    #[error("en passant square {0} is inconsistent with board state")]
    InvalidEpPosition(Square),
    #[error("invalid halfmove clock: {0}")]
    InvalidHalfmoveClock(#[source] ParseIntError),
    #[error("invalid fullmove counter: {0}")]
    InvalidFullmoveCounter(#[source] ParseIntError),
    #[error("fullmove counter must be at least 1")]
    ZeroFullmoveCounter,
    #[error("invalid {0:?} king count: expected 1, found {1}")]
    InvalidKingCount(Color, u32),
    #[error("pawns cannot be placed on rank 1 or rank 8")]
    PawnOnBackRank,
    #[error("side not to move cannot be in check")]
    OpponentInCheck,
}

pub type Error = ParseError;

pub fn parse(fen: &str) -> Result<Board, ParseError> {
    let mut parts = fen.split_whitespace();
    let mut board = Board::empty();

    // Piece placement.
    let placement = parts
        .next()
        .ok_or(ParseError::MissingField("piece placement"))?;
    let ranks: Vec<&str> = placement.split('/').collect();
    if ranks.len() != 8 {
        return Err(ParseError::InvalidRankCount(ranks.len()));
    }

    for (rank_idx, rank_str) in ranks.iter().enumerate() {
        let rank = (7 - rank_idx) as u8;
        let mut file = 0usize;

        for c in rank_str.chars() {
            if let Some(digit) = c.to_digit(10)
                && (1..=8).contains(&digit)
            {
                file += digit as usize;
            } else if let Some(piece) = Piece::from_char(c) {
                if file >= 8 {
                    return Err(ParseError::InvalidRankLength {
                        rank: (rank + 1) as usize,
                        files: file + 1,
                    });
                }
                let sq = Square::from_coords(rank, file as u8);
                board.put_piece_no_zobrist(piece, sq);
                file += 1;
            } else {
                return Err(ParseError::InvalidPieceChar(c));
            }
        }

        if file != 8 {
            return Err(ParseError::InvalidRankLength {
                rank: (rank + 1) as usize,
                files: file,
            });
        }
    }

    // Active color.
    let color = parts
        .next()
        .ok_or(ParseError::MissingField("active color"))?;
    board.side_to_move = match color {
        "w" => Color::White,
        "b" => Color::Black,
        _ => return Err(ParseError::InvalidActiveColor(color.to_string())),
    };

    // Castling rights.
    let castling = parts
        .next()
        .ok_or(ParseError::MissingField("castling availability"))?;
    board.state.castling_rights = 0;
    if castling != "-" {
        for c in castling.chars() {
            let bit = match c {
                'K' => State::CASTLE_WK,
                'Q' => State::CASTLE_WQ,
                'k' => State::CASTLE_BK,
                'q' => State::CASTLE_BQ,
                _ => return Err(ParseError::InvalidCastlingFlag(c)),
            };
            if (board.state.castling_rights & bit) != 0 {
                return Err(ParseError::DuplicateCastlingFlag(c));
            }
            board.state.castling_rights |= bit;
        }
    }

    // En passant target.
    let ep = parts
        .next()
        .ok_or(ParseError::MissingField("en passant target"))?;
    if ep != "-" {
        board.state.ep_square = Some(ep.parse::<Square>()?);
    }

    // Halfmove clock.
    board.state.halfmove_clock = parts
        .next()
        .unwrap_or("0")
        .parse::<u32>()
        .map_err(ParseError::InvalidHalfmoveClock)?;

    // Fullmove counter.
    board.fullmove_counter = parts
        .next()
        .unwrap_or("1")
        .parse::<u32>()
        .map_err(ParseError::InvalidFullmoveCounter)?;

    if parts.next().is_some() {
        return Err(ParseError::ExtraFields);
    }

    if board.fullmove_counter == 0 {
        return Err(ParseError::ZeroFullmoveCounter);
    }

    // Validate king counts (exactly one per color).
    for color in Color::ALL {
        let count = board.bitboard(Piece::new(color, Kind::King)).count();
        if count != 1 {
            return Err(ParseError::InvalidKingCount(color, count));
        }
    }

    // Pawns cannot exist on the first or eighth rank.
    let pawns = board.bitboard(Piece::WPawn) | board.bitboard(Piece::BPawn);
    if (pawns.as_u64() & (Bitboard::RANK_1 | Bitboard::RANK_8)) != 0 {
        return Err(ParseError::PawnOnBackRank);
    }

    // Castling rights require the corresponding king and rook on their starting squares.
    let cr = board.state.castling_rights;
    let valid_castling = ((cr & State::CASTLE_WK) == 0
        || (board.piece_at(Square::new(4)) == Some(Piece::WKing)
            && board.piece_at(Square::new(7)) == Some(Piece::WRook)))
        && ((cr & State::CASTLE_WQ) == 0
            || (board.piece_at(Square::new(4)) == Some(Piece::WKing)
                && board.piece_at(Square::new(0)) == Some(Piece::WRook)))
        && ((cr & State::CASTLE_BK) == 0
            || (board.piece_at(Square::new(60)) == Some(Piece::BKing)
                && board.piece_at(Square::new(63)) == Some(Piece::BRook)))
        && ((cr & State::CASTLE_BQ) == 0
            || (board.piece_at(Square::new(60)) == Some(Piece::BKing)
                && board.piece_at(Square::new(56)) == Some(Piece::BRook)));
    if !valid_castling {
        return Err(ParseError::InvalidCastlingRights);
    }

    // En passant target must match an empty skipped square behind an opponent pawn.
    if let Some(ep_sq) = board.state.ep_square {
        let expected_rank = match board.side_to_move {
            Color::White => 5,
            Color::Black => 2,
        };
        if ep_sq.rank() != expected_rank {
            return Err(ParseError::InvalidEpPosition(ep_sq));
        }
        let (pushed_sq, origin_sq, expected_pawn) = match board.side_to_move {
            Color::White => (
                Square::new(ep_sq.as_u8() - 8),
                Square::new(ep_sq.as_u8() + 8),
                Piece::BPawn,
            ),
            Color::Black => (
                Square::new(ep_sq.as_u8() + 8),
                Square::new(ep_sq.as_u8() - 8),
                Piece::WPawn,
            ),
        };
        if board.piece_at(ep_sq).is_some()
            || board.piece_at(origin_sq).is_some()
            || board.piece_at(pushed_sq) != Some(expected_pawn)
        {
            return Err(ParseError::InvalidEpPosition(ep_sq));
        }
    }

    // Side not to move cannot already be in check (also prevents adjacent kings).
    let opp_king_sq = board
        .bitboard(Piece::new(board.side_to_move.flip(), Kind::King))
        .lsb();
    if crate::move_gen::generator::is_square_attacked(&board, opp_king_sq, board.side_to_move) {
        return Err(ParseError::OpponentInCheck);
    }

    // Finalize hashes.
    board.state.ep_hashed = crate::move_gen::generator::has_legal_ep(&board);
    board.state.zobrist_key = board.compute_zobrist_key();
    board.state.pawn_key = board.compute_pawn_key();

    Ok(board)
}

pub fn generate(board: &Board) -> String {
    // Pre-allocate with average FEN length
    let mut fen = String::with_capacity(90);

    // Piece placement (rank 8 down to rank 1)
    for rank in (0..8).rev() {
        let mut empty_count = 0;

        for file in 0..8 {
            let sq = Square::from_coords(rank, file);

            if let Some(piece) = board.piece_at(sq) {
                if empty_count > 0 {
                    // char::from_digit securely converts 1..8 to '1'..'8'
                    fen.push(std::char::from_digit(empty_count as u32, 10).unwrap());
                    empty_count = 0;
                }
                fen.push(piece.to_char());
            } else {
                empty_count += 1;
            }
        }

        // Flush remaining empty squares at the end of the rank
        if empty_count > 0 {
            fen.push(std::char::from_digit(empty_count as u32, 10).unwrap());
        }

        // Add rank separator, except for the last rank
        if rank > 0 {
            fen.push('/');
        }
    }

    // Side to move
    fen.push(' ');
    fen.push(match board.side_to_move {
        Color::White => 'w',
        Color::Black => 'b',
    });

    // Castling availability
    fen.push(' ');
    if board.state.castling_rights == 0 {
        fen.push('-');
    } else {
        if board.state.castling_rights & 0b0001 != 0 {
            fen.push('K');
        }
        if board.state.castling_rights & 0b0010 != 0 {
            fen.push('Q');
        }
        if board.state.castling_rights & 0b0100 != 0 {
            fen.push('k');
        }
        if board.state.castling_rights & 0b1000 != 0 {
            fen.push('q');
        }
    }

    // En passant target
    fen.push(' ');
    if let Some(sq) = board.state.ep_square {
        write!(&mut fen, "{}", sq).unwrap();
    } else {
        fen.push('-');
    }

    // Halfmove clock and fullmove counter
    write!(
        &mut fen,
        " {} {}",
        board.state.halfmove_clock, board.fullmove_counter
    )
    .unwrap();

    fen
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_regenerates_starting_fen() {
        let board = parse(STARTING_FEN).unwrap();
        assert_eq!(board.to_fen(), STARTING_FEN);
    }

    #[test]
    fn rejects_malformed_fen_records() {
        assert_eq!(parse(""), Err(ParseError::MissingField("piece placement")));
        assert_eq!(
            parse("8/8/8/8/8/8/8/8"),
            Err(ParseError::MissingField("active color"))
        );
        assert_eq!(
            parse("8/8/8/8/8/8/8/8 x - - 0 1"),
            Err(ParseError::InvalidActiveColor("x".to_string()))
        );
        assert_eq!(
            parse("8/8/8/8/8/8/8/8 w X - 0 1"),
            Err(ParseError::InvalidCastlingFlag('X'))
        );
        assert!(matches!(
            parse("8/8/8/8/8/8/8/8 w - invalid_ep 0 1"),
            Err(ParseError::InvalidEpSquare(_))
        ));
        assert!(matches!(
            parse("8/8/8/8/8/8/8/8 w - - not_a_num 1"),
            Err(ParseError::InvalidHalfmoveClock(_))
        ));
        assert!(matches!(
            parse("8/8/8/8/8/8/8/8 w - - 0 not_a_num"),
            Err(ParseError::InvalidFullmoveCounter(_))
        ));
        assert_eq!(
            parse("8/8/8/8/8/8/8/7X w - - 0 1"),
            Err(ParseError::InvalidPieceChar('X'))
        );
    }
}
