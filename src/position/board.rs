use super::fen;
use super::zobrist::ZOBRIST;
use crate::chess::r#move::Flag;
use crate::chess::piece::{Color, Kind};
use crate::chess::{Bitboard, Move, Piece, Square};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Error {
    #[error("no piece belonging to side to move at source square {0}")]
    NoPieceAtSource(Square),
    #[error("illegal move: {0}")]
    IllegalMove(Move),
    #[error("no state history to unmake")]
    EmptyHistory,
    #[error("no piece at destination square {0} during unmake")]
    NoPieceAtDestination(Square),
    #[error(transparent)]
    Fen(#[from] fen::ParseError),
}

/// Array used to update castling rights upon moves and captures.
/// Index by square (0..64).
/// Bit layout: 0b1111 = Any, 0b0001 = WK, 0b0010 = WQ, 0b0100 = BK, 0b1000 = BQ
pub const CASTLING_MASKS: [u8; 64] = [
    13, 15, 15, 15, 12, 15, 15, 14, // Rank 1: a1(WQ lost), e1(WK+WQ lost), h1(WK lost)
    15, 15, 15, 15, 15, 15, 15, 15, // Rank 2
    15, 15, 15, 15, 15, 15, 15, 15, // Rank 3
    15, 15, 15, 15, 15, 15, 15, 15, // Rank 4
    15, 15, 15, 15, 15, 15, 15, 15, // Rank 5
    15, 15, 15, 15, 15, 15, 15, 15, // Rank 6
    15, 15, 15, 15, 15, 15, 15, 15, // Rank 7
    7, 15, 15, 15, 3, 15, 15, 11, // Rank 8: a8(BQ lost), e8(BK+BQ lost), h8(BK lost)
];

/// Stores irreversible board data, such data that cannot
/// have its previous state be mathematically deduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    pub castling_rights: u8,
    pub ep_square: Option<Square>,
    pub halfmove_clock: u32,
    pub captured_pcs: Option<Piece>,
    pub zobrist_key: u64,
    pub flag: Flag,
}

impl State {
    pub const CASTLE_WK: u8 = 0b0001;
    pub const CASTLE_WQ: u8 = 0b0010;
    pub const CASTLE_BK: u8 = 0b0100;
    pub const CASTLE_BQ: u8 = 0b1000;
}

/// Representation for a chess board/position
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    pub(crate) bboards: [Bitboard; 12],
    pub(crate) pieces: [Option<Piece>; 64],

    pub(crate) color_masks: [Bitboard; 2],
    pub(crate) all_occupancy: Bitboard,

    pub side_to_move: Color,
    pub fullmove_counter: u32,
    pub state: State,

    pub history: Vec<State>,
}

impl Board {
    pub fn from_fen(fen: &str) -> Result<Self, fen::ParseError> {
        fen::parse(fen)
    }

    pub fn to_fen(&self) -> String {
        fen::generate(self)
    }

    /// Create the intial instance of a board
    pub fn empty() -> Self {
        Self {
            bboards: [Bitboard::new(0); 12],
            pieces: [None; 64],
            color_masks: [Bitboard::new(0); 2],
            all_occupancy: Bitboard::new(0),
            side_to_move: Color::White,
            fullmove_counter: 1,
            state: State {
                castling_rights: 0,
                ep_square: None,
                halfmove_clock: 0,
                captured_pcs: None,
                zobrist_key: 0,
                flag: Flag::Quiet,
            },
            history: Vec::with_capacity(256),
        }
    }

    pub fn compute_zobrist_key(&self) -> u64 {
        let mut key = 0;

        // Pieces
        for sq in 0..64 {
            let square = Square::new(sq);
            if let Some(piece) = self.piece_at(square) {
                key ^= ZOBRIST.pieces[piece as usize][sq as usize];
            }
        }

        // Side to move
        if self.side_to_move == Color::Black {
            key ^= ZOBRIST.side_to_move;
        }

        // Castling Rights
        key ^= ZOBRIST.castling[self.state.castling_rights as usize];

        // En Passant
        if let Some(ep) = self.state.ep_square {
            key ^= ZOBRIST.en_passant[ep.file() as usize];
        }

        key
    }

    /// Retrieve any piece at the given square by checking the pieces mailbox.
    #[inline(always)]
    pub fn piece_at(&self, sq: Square) -> Option<Piece> {
        self.pieces[sq.as_usize()]
    }

    /// Retrieve a piece at the given square restricted to a specific color.
    #[inline(always)]
    pub fn piece_at_color(&self, sq: Square, color: Color) -> Option<Piece> {
        self.pieces[sq.as_usize()].filter(|p| p.color() == color)
    }

    /// Get the bitboard/occupancy of a piece
    #[inline(always)]
    pub fn bitboard(&self, piece: Piece) -> Bitboard {
        self.bboards[piece as usize]
    }

    #[inline(always)]
    pub fn occupancy(&self) -> Bitboard {
        self.all_occupancy
    }

    /// Get the bitboard/occupancy of a color
    #[inline(always)]
    pub fn us(&self, color: Color) -> Bitboard {
        self.color_masks[color as usize]
    }

    #[inline(always)]
    pub fn can_castle_kingside(&self, us: Color) -> bool {
        if us == Color::White {
            (self.state.castling_rights & State::CASTLE_WK) != 0
        } else {
            (self.state.castling_rights & State::CASTLE_BK) != 0
        }
    }

    #[inline(always)]
    pub fn can_castle_queenside(&self, us: Color) -> bool {
        if us == Color::White {
            (self.state.castling_rights & State::CASTLE_WQ) != 0
        } else {
            (self.state.castling_rights & State::CASTLE_BQ) != 0
        }
    }

    pub fn move_flag(&self, mv: Move) -> Flag {
        let from = mv.from();
        let to = mv.to();
        let us = self.side_to_move;
        let them = us.flip();

        let moving_pcs = self
            .piece_at_color(from, us)
            .expect("Invalid move: no piece at source square");
        let is_capture = self.piece_at_color(to, them).is_some();

        match moving_pcs.kind() {
            Kind::Pawn => {
                let promo_rank = match us {
                    Color::White => 7,
                    Color::Black => 0,
                };
                if to.rank() == promo_rank {
                    if is_capture {
                        Flag::QueenPromoCapture
                    } else {
                        Flag::QueenPromo
                    }
                } else if self.state.ep_square == Some(to) && from.file() != to.file() {
                    Flag::EpCapture
                } else if (from.rank() as i8 - to.rank() as i8).abs() == 2 {
                    Flag::DoublePawnPush
                } else if is_capture {
                    Flag::Capture
                } else {
                    Flag::Quiet
                }
            }
            Kind::King => {
                if (from.rank() == 0 || from.rank() == 7) && from.file() == 4 && to.file() == 6 {
                    Flag::KingCastle
                } else if (from.rank() == 0 || from.rank() == 7)
                    && from.file() == 4
                    && to.file() == 2
                {
                    Flag::QueenCastle
                } else if is_capture {
                    Flag::Capture
                } else {
                    Flag::Quiet
                }
            }
            _ => {
                if is_capture {
                    Flag::Capture
                } else {
                    Flag::Quiet
                }
            }
        }
    }

    #[allow(clippy::collapsible_if)]
    pub fn make_move(&mut self, mv: Move) {
        self.history.push(self.state);

        let us = self.side_to_move;
        let them = us.flip();

        let from = mv.from();
        let to = mv.to();
        let flag = self.move_flag(mv);

        let moving_pcs = self
            .piece_at_color(from, us)
            .expect("Invalid move: no piece at source square");
        // Remove moving piece from source square
        self.remove_piece(moving_pcs, from);

        // Handle capture
        let mut captured_pcs = None;
        if flag as u8 >= Flag::Capture as u8 && flag != Flag::EpCapture {
            if let Some(captured) = self.piece_at_color(to, them) {
                self.remove_piece(captured, to);
                captured_pcs = Some(captured);
            }
        }

        match flag {
            Flag::Quiet | Flag::Capture | Flag::DoublePawnPush => {
                self.put_piece(moving_pcs, to);
            }
            Flag::KingCastle => {
                self.put_piece(moving_pcs, to);
                let (r_from, r_to) = match us {
                    Color::White => (Square::new(7), Square::new(5)), // h1 to f1
                    Color::Black => (Square::new(63), Square::new(61)), // h8 to f8
                };
                let rook = Piece::new(us, Kind::Rook);
                self.remove_piece(rook, r_from);
                self.put_piece(rook, r_to);
            }
            Flag::QueenCastle => {
                self.put_piece(moving_pcs, to);
                let (r_from, r_to) = match us {
                    Color::White => (Square::new(0), Square::new(3)), // a1 to d1
                    Color::Black => (Square::new(56), Square::new(59)), // a8 to d8
                };
                let rook = Piece::new(us, Kind::Rook);
                self.remove_piece(rook, r_from);
                self.put_piece(rook, r_to);
            }
            Flag::EpCapture => {
                self.put_piece(moving_pcs, to);
                let cap_sq = Square::new(match us {
                    Color::White => to.as_u8() - 8,
                    Color::Black => to.as_u8() + 8,
                });
                let captured_pawn = Piece::new(them, Kind::Pawn);
                self.remove_piece(captured_pawn, cap_sq);
                captured_pcs = Some(captured_pawn);
            }
            // Promotions
            _ => {
                let promo_kind = match flag {
                    Flag::KnightPromo | Flag::KnightPromoCapture => Kind::Knight,
                    Flag::BishopPromo | Flag::BishopPromoCapture => Kind::Bishop,
                    Flag::RookPromo | Flag::RookPromoCapture => Kind::Rook,
                    Flag::QueenPromo | Flag::QueenPromoCapture => Kind::Queen,
                    _ => unreachable!(),
                };
                let promo_piece = Piece::new(us, promo_kind);
                self.put_piece(promo_piece, to);
            }
        }

        self.update_state(mv, flag, moving_pcs, captured_pcs);
    }

    pub fn unmake_move(&mut self, mv: Move) {
        // Revert turn and counters
        let us = self.side_to_move.flip();
        self.side_to_move = us;
        if us == Color::Black {
            self.fullmove_counter -= 1;
        }

        let from = mv.from();
        let to = mv.to();
        let flag = self.state.flag;

        // Identify and remove the moving piece from destination
        let moving_pcs = self
            .piece_at_color(to, us)
            .expect("Invalid unmake: no piece at destination square");
        self.remove_piece(moving_pcs, to);

        // Restore the piece to its source square
        if flag as u8 >= Flag::KnightPromo as u8 {
            let pawn = Piece::new(us, Kind::Pawn);
            self.put_piece(pawn, from);
        } else {
            self.put_piece(moving_pcs, from);
        }

        // Restore captured pieces
        if let Some(captured) = self.state.captured_pcs {
            if flag == Flag::EpCapture {
                let cap_sq = Square::new(match us {
                    Color::White => to.as_u8() - 8,
                    Color::Black => to.as_u8() + 8,
                });
                self.put_piece(captured, cap_sq);
            } else {
                self.put_piece(captured, to);
            }
        }

        // Revert castling rook placements
        match flag {
            Flag::KingCastle => {
                let (r_from, r_to) = match us {
                    Color::White => (Square::new(7), Square::new(5)),
                    Color::Black => (Square::new(63), Square::new(61)),
                };
                let rook = Piece::new(us, Kind::Rook);
                self.remove_piece(rook, r_to);
                self.put_piece(rook, r_from);
            }
            Flag::QueenCastle => {
                let (r_from, r_to) = match us {
                    Color::White => (Square::new(0), Square::new(3)),
                    Color::Black => (Square::new(56), Square::new(59)),
                };
                let rook = Piece::new(us, Kind::Rook);
                self.remove_piece(rook, r_to);
                self.put_piece(rook, r_from);
            }
            _ => {}
        }

        // Restore irreversible state
        self.state = self.history.pop().expect("No state history to unmake");
    }

    /// Attempt to make a move, verifying that there is a piece belonging to the side to move
    /// at the source square. Returns an error instead of panicking if invalid.
    pub fn try_make_move(&mut self, mv: Move) -> Result<(), Error> {
        let us = self.side_to_move;
        let from = mv.from();
        if self.piece_at_color(from, us).is_none() {
            return Err(Error::NoPieceAtSource(from));
        }
        self.make_move(mv);
        Ok(())
    }

    /// Attempt to unmake a move, verifying that history is non-empty and that there is a piece
    /// at the destination square. Returns an error instead of panicking if invalid.
    pub fn try_unmake_move(&mut self, mv: Move) -> Result<(), Error> {
        if self.history.is_empty() {
            return Err(Error::EmptyHistory);
        }
        let us = self.side_to_move.flip();
        let to = mv.to();
        if self.piece_at_color(to, us).is_none() {
            return Err(Error::NoPieceAtDestination(to));
        }
        self.unmake_move(mv);
        Ok(())
    }

    #[inline(always)]
    fn put_piece(&mut self, piece: Piece, sq: Square) {
        // Set bit and update masks
        self.bboards[piece as usize].set_bit(sq);
        self.color_masks[piece.color() as usize].set_bit(sq);
        self.all_occupancy.set_bit(sq);
        self.pieces[sq.as_usize()] = Some(piece);

        // XOR in the piece
        self.state.zobrist_key ^= ZOBRIST.pieces[piece as usize][sq.as_usize()];
    }

    #[inline(always)]
    fn remove_piece(&mut self, piece: Piece, sq: Square) {
        // Clear bit and update masks
        self.bboards[piece as usize].clear_bit(sq);
        self.color_masks[piece.color() as usize].clear_bit(sq);
        self.all_occupancy.clear_bit(sq);
        self.pieces[sq.as_usize()] = None;

        // XOR out the piece
        self.state.zobrist_key ^= ZOBRIST.pieces[piece as usize][sq.as_usize()];
    }

    /// Places a piece on the board without updating the Zobrist hash.
    /// Used exclusively for position initialization.
    #[inline(always)]
    pub(crate) fn put_piece_no_zobrist(&mut self, piece: Piece, sq: Square) {
        let p_idx = piece as usize;
        let c_idx = piece.color() as usize;

        self.bboards[p_idx].set_bit(sq);
        self.color_masks[c_idx].set_bit(sq);
        self.all_occupancy.set_bit(sq);
        self.pieces[sq.as_usize()] = Some(piece);
    }

    #[inline]
    fn update_state(
        &mut self,
        mv: Move,
        flag: Flag,
        moving_pcs: Piece,
        captured_pcs: Option<Piece>,
    ) {
        self.state.captured_pcs = captured_pcs;
        self.state.flag = flag;

        // Update zobrist key: XOR out old state
        self.state.zobrist_key ^= ZOBRIST.castling[self.state.castling_rights as usize];
        if let Some(ep) = self.state.ep_square {
            self.state.zobrist_key ^= ZOBRIST.en_passant[ep.file() as usize];
        }

        // Update en passant target square
        if flag == Flag::DoublePawnPush {
            self.state.ep_square = Some(Square::new(match moving_pcs.color() {
                Color::White => mv.to().as_u8() - 8,
                Color::Black => mv.to().as_u8() + 8,
            }));
        } else {
            self.state.ep_square = None;
        }

        // Halfmove clock reset on capture or pawn move
        if captured_pcs.is_some() || moving_pcs.kind() == Kind::Pawn {
            self.state.halfmove_clock = 0;
        } else {
            self.state.halfmove_clock += 1;
        }

        // Castling rights update
        self.state.castling_rights &=
            CASTLING_MASKS[mv.from().as_usize()] & CASTLING_MASKS[mv.to().as_usize()];

        // XOR in new state
        self.state.zobrist_key ^= ZOBRIST.castling[self.state.castling_rights as usize];
        if let Some(ep) = self.state.ep_square {
            self.state.zobrist_key ^= ZOBRIST.en_passant[ep.file() as usize];
        }
        self.state.zobrist_key ^= ZOBRIST.side_to_move;

        // Turn update
        if self.side_to_move == Color::Black {
            self.fullmove_counter += 1;
        }
        self.side_to_move = self.side_to_move.flip();
    }

    /// Returns a pretty-formatted ASCII representation of the board.
    pub fn pretty(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        writeln!(&mut s, "  +-----------------+").unwrap();
        for rank in (0..8).rev() {
            write!(&mut s, "{} | ", rank + 1).unwrap();
            for file in 0..8 {
                let sq = Square::from_coords(rank, file);
                if let Some(piece) = self.piece_at(sq) {
                    write!(&mut s, "{} ", piece.to_char()).unwrap();
                } else {
                    write!(&mut s, ". ").unwrap();
                }
            }
            writeln!(&mut s, "|").unwrap();
        }
        writeln!(&mut s, "  +-----------------+").unwrap();
        writeln!(&mut s, "    a b c d e f g h").unwrap();

        writeln!(&mut s, "FEN: {}", self.to_fen()).unwrap();
        writeln!(&mut s, "Key: {:016X}", self.state.zobrist_key).unwrap();
        s
    }

    /// Prints the pretty-formatted ASCII representation of the board to standard output.
    pub fn pretty_print(&self) {
        print!("{}", self.pretty());
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::from_fen(fen::STARTING_FEN).expect("STARTING_FEN is invalid")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn makes_and_unmakes_double_pawn_push() {
        let mut board = Board::default();
        let initial_fen = board.to_fen();
        let initial_key = board.state.zobrist_key;

        let mv = Move::from_str("e2e4").unwrap();
        board.make_move(mv);
        assert_eq!(board.state.flag, Flag::DoublePawnPush);
        assert_eq!(board.state.ep_square, Some(Square::from_str("e3").unwrap()));

        board.unmake_move(mv);
        assert_eq!(board.to_fen(), initial_fen);
        assert_eq!(board.state.zobrist_key, initial_key);
    }

    #[test]
    fn makes_and_unmakes_regular_capture() {
        // e4, d5, exd5
        let mut board = Board::default();
        board.make_move(Move::from_str("e2e4").unwrap());
        board.make_move(Move::from_str("d7d5").unwrap());
        let fen_before_cap = board.to_fen();
        let key_before_cap = board.state.zobrist_key;

        let cap_mv = Move::from_str("e4d5").unwrap();
        board.make_move(cap_mv);
        assert_eq!(board.state.flag, Flag::Capture);

        board.unmake_move(cap_mv);
        assert_eq!(board.to_fen(), fen_before_cap);
        assert_eq!(board.state.zobrist_key, key_before_cap);
    }

    #[test]
    fn makes_and_unmakes_castling() {
        // 1. e4 e5 2. Nf3 Nc6 3. Bc4 Bc5 4. O-O
        let mut board = Board::default();
        for m in &["e2e4", "e7e5", "g1f3", "b8c6", "f1c4", "f8c5"] {
            board.make_move(Move::from_str(m).unwrap());
        }
        let fen_before_castle = board.to_fen();
        let key_before_castle = board.state.zobrist_key;

        let castle_mv = Move::from_str("e1g1").unwrap();
        board.make_move(castle_mv);
        assert_eq!(board.state.flag, Flag::KingCastle);

        board.unmake_move(castle_mv);
        assert_eq!(board.to_fen(), fen_before_castle);
        assert_eq!(board.state.zobrist_key, key_before_castle);
    }

    #[test]
    fn makes_and_unmakes_en_passant_capture() {
        // 1. e4 a6 2. e5 d5 3. exd6
        let mut board = Board::default();
        for m in &["e2e4", "a7a6", "e4e5", "d7d5"] {
            board.make_move(Move::from_str(m).unwrap());
        }
        let fen_before_ep = board.to_fen();
        let key_before_ep = board.state.zobrist_key;

        let ep_mv = Move::from_str("e5d6").unwrap();
        board.make_move(ep_mv);
        assert_eq!(board.state.flag, Flag::EpCapture);

        board.unmake_move(ep_mv);
        assert_eq!(board.to_fen(), fen_before_ep);
        assert_eq!(board.state.zobrist_key, key_before_ep);
    }

    #[test]
    fn try_make_and_unmake_report_errors() {
        let mut board = Board::default();

        // White moves first, e7 has a black pawn, not white
        let invalid_white_mv = Move::from_str("e7e5").unwrap();
        assert_eq!(
            board.try_make_move(invalid_white_mv),
            Err(Error::NoPieceAtSource(Square::from_str("e7").unwrap()))
        );

        // Cannot unmake when history is empty
        assert_eq!(
            board.try_unmake_move(Move::from_str("e2e4").unwrap()),
            Err(Error::EmptyHistory)
        );

        // Make a valid move
        let valid_mv = Move::from_str("e2e4").unwrap();
        assert!(board.try_make_move(valid_mv).is_ok());

        // Now try unmake with mismatched move (no white piece on a8)
        let wrong_unmake = Move::from_str("a7a8").unwrap();
        assert_eq!(
            board.try_unmake_move(wrong_unmake),
            Err(Error::NoPieceAtDestination(Square::from_str("a8").unwrap()))
        );

        // Correct unmake works
        assert!(board.try_unmake_move(valid_mv).is_ok());
    }
}
