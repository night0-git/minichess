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
    #[error("no piece at destination square {0}")]
    NoPieceAtDestination(Square),
    #[error("pawn move to back rank missing promotion piece: {0}")]
    MissingPromotion(Move),
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
    pub(crate) ep_hashed: bool,
    pub halfmove_clock: u32,
    pub captured_pcs: Option<Piece>,
    pub zobrist_key: u64,
    pub pawn_key: u64,
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
    pub const FIFTY_MOVE_RULE_HALFMOVES: u32 = 100;
    const TWOFOLD_PRIOR_OCCURRENCES: usize = 1;
    const THREEFOLD_PRIOR_OCCURRENCES: usize = 2;

    pub fn from_fen(fen: &str) -> Result<Self, fen::ParseError> {
        fen::parse(fen)
    }

    pub fn to_fen(&self) -> String {
        fen::generate(self)
    }

    /// Returns `true` if the halfmove clock has reached the 50-move draw limit (100 plies).
    #[inline]
    pub const fn is_fifty_move_draw(&self) -> bool {
        self.state.halfmove_clock >= Self::FIFTY_MOVE_RULE_HALFMOVES
    }

    /// Returns `true` if neither side has sufficient material to deliver checkmate.
    #[inline]
    pub const fn is_insufficient_material(&self) -> bool {
        let heavy_or_pawns = self.bboards[Piece::WPawn as usize].as_u64()
            | self.bboards[Piece::BPawn as usize].as_u64()
            | self.bboards[Piece::WRook as usize].as_u64()
            | self.bboards[Piece::BRook as usize].as_u64()
            | self.bboards[Piece::WQueen as usize].as_u64()
            | self.bboards[Piece::BQueen as usize].as_u64();

        if heavy_or_pawns != 0 {
            return false;
        }

        let knights = self.bboards[Piece::WKnight as usize].as_u64()
            | self.bboards[Piece::BKnight as usize].as_u64();
        let bishops = self.bboards[Piece::WBishop as usize].as_u64()
            | self.bboards[Piece::BBishop as usize].as_u64();

        if knights == 0 {
            // K vs K, K+B vs K, or bishops on a single square color cannot checkmate.
            (bishops & Bitboard::LIGHT_SQRS) == 0 || (bishops & Bitboard::DARK_SQRS) == 0
        } else {
            // Only K+N vs K is insufficient; K+N+N, K+N vs K+N, and K+B vs K+N can helpmate.
            bishops == 0 && knights.count_ones() <= 1
        }
    }

    /// Returns `true` if the current position has occurred at least once before in `self.history`.
    #[inline]
    pub fn is_twofold_repetition(&self) -> bool {
        self.is_repetition(Self::TWOFOLD_PRIOR_OCCURRENCES)
    }

    /// Returns `true` if the current position has occurred at least twice before in `self.history`.
    #[inline]
    pub fn is_threefold_repetition(&self) -> bool {
        self.is_repetition(Self::THREEFOLD_PRIOR_OCCURRENCES)
    }

    /// Returns `true` if the current position has occurred at least `threshold`
    /// times earlier in `self.history` (e.g. `1` for a 2-fold repetition, `2` for a 3-fold).
    pub fn is_repetition(&self, threshold: usize) -> bool {
        let limit = (self.state.halfmove_clock as usize).min(self.history.len());
        if limit < 2 {
            return false;
        }

        let key = self.state.zobrist_key;
        let len = self.history.len();
        let mut count = 0;
        let mut dist = 2;
        while dist <= limit {
            if self.history[len - dist].zobrist_key == key {
                count += 1;
                if count >= threshold {
                    return true;
                }
            }
            dist += 2;
        }
        false
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
                ep_hashed: false,
                halfmove_clock: 0,
                captured_pcs: None,
                zobrist_key: 0,
                pawn_key: 0,
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

        // En passant: hash only when a legal en passant capture exists.
        if let Some(ep) = self.state.ep_square
            && crate::move_gen::generator::has_legal_ep(self)
        {
            key ^= ZOBRIST.en_passant[ep.file() as usize];
        }

        key
    }

    /// Compute the Zobrist hash of all White and Black pawns on the board.
    pub fn compute_pawn_key(&self) -> u64 {
        let mut key = 0;

        for pawn in [Piece::WPawn, Piece::BPawn] {
            for sq in self.bitboard(pawn) {
                key ^= ZOBRIST.pieces[pawn as usize][sq.as_usize()];
            }
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

    /// Deduce flag for a move on the current board.
    pub fn move_flag(&self, mv: Move) -> Result<Flag, Error> {
        let from = mv.from();
        let to = mv.to();
        let us = self.side_to_move;
        let them = us.flip();

        let moving_pcs = self
            .piece_at_color(from, us)
            .ok_or(Error::NoPieceAtSource(from))?;
        let is_capture = self.piece_at_color(to, them).is_some();

        match moving_pcs.kind() {
            Kind::Pawn => {
                let promo_rank = match us {
                    Color::White => 7,
                    Color::Black => 0,
                };
                if to.rank() == promo_rank {
                    let promo = mv.promotion().ok_or(Error::MissingPromotion(mv))?;
                    Flag::promo(promo, is_capture).ok_or(Error::IllegalMove(mv))
                } else if self.state.ep_square == Some(to) && from.file() != to.file() {
                    Ok(Flag::EpCapture)
                } else if (from.rank() as i8 - to.rank() as i8).abs() == 2 {
                    Ok(Flag::DoublePawnPush)
                } else if is_capture {
                    Ok(Flag::Capture)
                } else {
                    Ok(Flag::Quiet)
                }
            }
            Kind::King => {
                if (from.rank() == 0 || from.rank() == 7) && from.file() == 4 && to.file() == 6 {
                    Ok(Flag::KingCastle)
                } else if (from.rank() == 0 || from.rank() == 7)
                    && from.file() == 4
                    && to.file() == 2
                {
                    Ok(Flag::QueenCastle)
                } else if is_capture {
                    Ok(Flag::Capture)
                } else {
                    Ok(Flag::Quiet)
                }
            }
            _ => {
                if is_capture {
                    Ok(Flag::Capture)
                } else {
                    Ok(Flag::Quiet)
                }
            }
        }
    }

    /// Make a fully flagged move on the board without validating legality, can panic.
    #[allow(clippy::collapsible_if)]
    pub fn make_move(&mut self, mv: Move) {
        debug_assert_eq!(
            self.move_flag(mv),
            Ok(mv.flag()),
            "make_move requires a move with matching flag: {mv}"
        );
        self.history.push(self.state);

        let us = self.side_to_move;
        let them = us.flip();

        let from = mv.from();
        let to = mv.to();
        let flag = mv.flag();

        let moving_pcs = self
            .piece_at_color(from, us)
            .expect("Invalid move: no piece at source square");
        // Remove moving piece from source square
        self.remove_piece(moving_pcs, from);

        // Handle capture
        let mut captured_pcs = None;
        if flag.is_capture() && flag != Flag::EpCapture {
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
                let promo_kind = mv.promotion().expect("Invalid promotion flag");
                let promo_piece = Piece::new(us, promo_kind);
                self.put_piece(promo_piece, to);
            }
        }

        self.update_state(mv, flag, moving_pcs, captured_pcs);
    }

    /// Unmake a flagged move from board history, can panic.
    pub fn unmake_move(&mut self, mv: Move) {
        let prev_state = self.history.pop().expect("No state history to unmake");

        // Revert turn and counters
        let us = self.side_to_move.flip();
        self.side_to_move = us;
        if us == Color::Black {
            self.fullmove_counter -= 1;
        }

        let from = mv.from();
        let to = mv.to();
        let flag = mv.flag();

        // Identify and remove the moving piece from destination
        let moving_pcs = self
            .piece_at_color(to, us)
            .expect("Invalid unmake: no piece at destination square");
        self.remove_piece(moving_pcs, to);

        // Restore the piece to its source square
        if flag.is_promotion() {
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
        self.state = prev_state;
    }

    /// Resolve the move flag from the current board and make the move,
    /// returning the resolved (flagged) move or an error instead of panicking.
    pub fn try_make_move(&mut self, mv: Move) -> Result<Move, Error> {
        let flag = self.move_flag(mv)?;
        let resolved = mv.with_flag(flag);
        self.make_move(resolved);
        Ok(resolved)
    }

    /// Attempt to unmake a flagged move, returning an error instead of panicking if invalid.
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
        let piece_key = ZOBRIST.pieces[piece as usize][sq.as_usize()];
        self.state.zobrist_key ^= piece_key;
        if piece.kind() == Kind::Pawn {
            self.state.pawn_key ^= piece_key;
        }
    }

    #[inline(always)]
    fn remove_piece(&mut self, piece: Piece, sq: Square) {
        // Clear bit and update masks
        self.bboards[piece as usize].clear_bit(sq);
        self.color_masks[piece.color() as usize].clear_bit(sq);
        self.all_occupancy.clear_bit(sq);
        self.pieces[sq.as_usize()] = None;

        // XOR out the piece
        let piece_key = ZOBRIST.pieces[piece as usize][sq.as_usize()];
        self.state.zobrist_key ^= piece_key;
        if piece.kind() == Kind::Pawn {
            self.state.pawn_key ^= piece_key;
        }
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

        // Update zobrist key: XOR out old state.
        self.state.zobrist_key ^= ZOBRIST.castling[self.state.castling_rights as usize];
        if self.state.ep_hashed
            && let Some(ep) = self.state.ep_square
        {
            self.state.zobrist_key ^= ZOBRIST.en_passant[ep.file() as usize];
        }

        // Update en passant target square.
        if flag == Flag::DoublePawnPush {
            self.state.ep_square = Some(Square::new(match moving_pcs.color() {
                Color::White => mv.to().as_u8() - 8,
                Color::Black => mv.to().as_u8() + 8,
            }));
        } else {
            self.state.ep_square = None;
        }
        self.state.ep_hashed = false;

        // Halfmove clock reset on capture or pawn move.
        if captured_pcs.is_some() || moving_pcs.kind() == Kind::Pawn {
            self.state.halfmove_clock = 0;
        } else {
            self.state.halfmove_clock += 1;
        }

        // Castling rights update.
        self.state.castling_rights &=
            CASTLING_MASKS[mv.from().as_usize()] & CASTLING_MASKS[mv.to().as_usize()];

        // Flip side to move before checking legal en passant for the next player.
        if self.side_to_move == Color::Black {
            self.fullmove_counter += 1;
        }
        self.side_to_move = self.side_to_move.flip();

        // XOR in new state.
        self.state.zobrist_key ^= ZOBRIST.castling[self.state.castling_rights as usize];
        if let Some(ep) = self.state.ep_square
            && crate::move_gen::generator::has_legal_ep(self)
        {
            self.state.zobrist_key ^= ZOBRIST.en_passant[ep.file() as usize];
            self.state.ep_hashed = true;
        }
        self.state.zobrist_key ^= ZOBRIST.side_to_move;
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

        let mv = Move::from_str("e2e4")
            .unwrap()
            .with_flag(Flag::DoublePawnPush);
        board.make_move(mv);
        assert_eq!(board.state.ep_square, Some(Square::from_str("e3").unwrap()));

        board.unmake_move(mv);
        assert_eq!(board.to_fen(), initial_fen);
        assert_eq!(board.state.zobrist_key, initial_key);
    }

    #[test]
    fn makes_and_unmakes_regular_capture() {
        // e4, d5, exd5
        let mut board = Board::default();
        board
            .try_make_move(Move::from_str("e2e4").unwrap())
            .unwrap();
        board
            .try_make_move(Move::from_str("d7d5").unwrap())
            .unwrap();
        let fen_before_cap = board.to_fen();
        let key_before_cap = board.state.zobrist_key;

        let cap_mv = Move::from_str("e4d5").unwrap().with_flag(Flag::Capture);
        board.make_move(cap_mv);
        assert_eq!(board.state.captured_pcs, Some(Piece::BPawn));

        board.unmake_move(cap_mv);
        assert_eq!(board.to_fen(), fen_before_cap);
        assert_eq!(board.state.zobrist_key, key_before_cap);
    }

    #[test]
    fn makes_and_unmakes_castling() {
        // 1. e4 e5 2. Nf3 Nc6 3. Bc4 Bc5 4. O-O
        let mut board = Board::default();
        for m in &["e2e4", "e7e5", "g1f3", "b8c6", "f1c4", "f8c5"] {
            board.try_make_move(Move::from_str(m).unwrap()).unwrap();
        }
        let fen_before_castle = board.to_fen();
        let key_before_castle = board.state.zobrist_key;

        let castle_mv = Move::from_str("e1g1").unwrap().with_flag(Flag::KingCastle);
        board.make_move(castle_mv);
        assert_eq!(
            board.piece_at(Square::from_str("f1").unwrap()),
            Some(Piece::WRook)
        );

        board.unmake_move(castle_mv);
        assert_eq!(board.to_fen(), fen_before_castle);
        assert_eq!(board.state.zobrist_key, key_before_castle);
    }

    #[test]
    fn makes_and_unmakes_en_passant_capture() {
        // 1. e4 a6 2. e5 d5 3. exd6
        let mut board = Board::default();
        for m in &["e2e4", "a7a6", "e4e5", "d7d5"] {
            board.try_make_move(Move::from_str(m).unwrap()).unwrap();
        }
        let fen_before_ep = board.to_fen();
        let key_before_ep = board.state.zobrist_key;

        let ep_mv = Move::from_str("e5d6").unwrap().with_flag(Flag::EpCapture);
        board.make_move(ep_mv);
        assert_eq!(board.piece_at(Square::from_str("d5").unwrap()), None);

        board.unmake_move(ep_mv);
        assert_eq!(board.to_fen(), fen_before_ep);
        assert_eq!(board.state.zobrist_key, key_before_ep);
    }

    #[test]
    fn makes_and_unmakes_promotion_capture() {
        let mut board = Board::from_fen("3r4/4P3/8/8/8/8/8/k6K w - - 0 1").unwrap();
        let initial_fen = board.to_fen();
        let initial_key = board.state.zobrist_key;

        let mv = Move::from_str("e7d8q")
            .unwrap()
            .with_flag(Flag::QueenPromoCapture);
        board.make_move(mv);
        assert_eq!(
            board.piece_at(Square::from_str("d8").unwrap()),
            Some(Piece::WQueen)
        );
        assert_eq!(board.state.captured_pcs, Some(Piece::BRook));

        board.unmake_move(mv);
        assert_eq!(board.to_fen(), initial_fen);
        assert_eq!(board.state.zobrist_key, initial_key);
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

        // Missing promotion on pawn push to back rank reports error
        let mut promo_board = Board::from_fen("8/4P3/8/8/8/8/8/k6K w - - 0 1").unwrap();
        let unpromoted_mv = Move::from_str("e7e8").unwrap();
        assert_eq!(
            promo_board.move_flag(unpromoted_mv),
            Err(Error::MissingPromotion(unpromoted_mv))
        );
        assert_eq!(
            promo_board.try_make_move(unpromoted_mv),
            Err(Error::MissingPromotion(unpromoted_mv))
        );

        // Cannot unmake when history is empty
        assert_eq!(
            board.try_unmake_move(Move::from_str("e2e4").unwrap()),
            Err(Error::EmptyHistory)
        );

        // Make a valid move and verify resolved flag is returned
        let raw_mv = Move::from_str("e2e4").unwrap();
        let valid_mv = board.try_make_move(raw_mv).unwrap();
        assert_eq!(valid_mv.flag(), Flag::DoublePawnPush);

        // Now try unmake with mismatched move (no white piece on a8)
        let wrong_unmake = Move::from_str("a7a8").unwrap();
        assert_eq!(
            board.try_unmake_move(wrong_unmake),
            Err(Error::NoPieceAtDestination(Square::from_str("a8").unwrap()))
        );

        // Correct unmake works
        assert!(board.try_unmake_move(valid_mv).is_ok());
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "make_move requires a move with matching flag")]
    fn make_move_unpromoted_pawn_to_back_rank_panics_in_debug() {
        let mut promo_board = Board::from_fen("8/4P3/8/8/8/8/8/k6K w - - 0 1").unwrap();
        let unpromoted_mv = Move::from_str("e7e8").unwrap();
        promo_board.make_move(unpromoted_mv);
    }

    #[test]
    fn detects_twofold_and_threefold_repetitions() {
        let mut board = Board::default();
        let cycle = ["g1f3", "g8f6", "f3g1", "f6g8"];

        assert!(!board.is_repetition(1));
        for mv_str in cycle {
            board
                .try_make_move(Move::from_str(mv_str).unwrap())
                .unwrap();
        }
        assert!(board.is_repetition(1));
        assert!(!board.is_repetition(2));

        for mv_str in cycle {
            board
                .try_make_move(Move::from_str(mv_str).unwrap())
                .unwrap();
        }
        assert!(board.is_repetition(2));
    }

    #[test]
    fn pawn_key_matches_recomputed_across_moves_and_unmake() {
        // Test pawn pushes, non-pawn moves, regular capture, en passant, castling, and promotion.
        let mut board = Board::default();
        assert_eq!(board.state.pawn_key, board.compute_pawn_key());

        // Non-pawn move keeps pawn_key identical.
        let key_before_knight = board.state.pawn_key;
        let knight_mv = board
            .try_make_move(Move::from_str("g1f3").unwrap())
            .unwrap();
        assert_eq!(board.state.pawn_key, key_before_knight);
        assert_eq!(board.state.pawn_key, board.compute_pawn_key());
        board.unmake_move(knight_mv);
        assert_eq!(board.state.pawn_key, key_before_knight);

        // Sequence with double push, capture, en passant, and castling setup.
        let mut played = Vec::new();
        for mv_str in [
            "e2e4", "a7a6", "e4e5", "d7d5", "e5d6", "b8c6", "g1f3", "c8e6", "f1c4", "d8d7", "e1g1",
        ] {
            let mv = board
                .try_make_move(Move::from_str(mv_str).unwrap())
                .unwrap();
            assert_eq!(
                board.state.pawn_key,
                board.compute_pawn_key(),
                "pawn_key mismatch after {mv_str}"
            );
            played.push(mv);
        }

        while let Some(mv) = played.pop() {
            board.unmake_move(mv);
            assert_eq!(board.state.pawn_key, board.compute_pawn_key());
        }

        // Promotion capture removes pawn from pawn_key.
        let mut promo_board = Board::from_fen("3r4/4P3/8/8/8/8/8/k6K w - - 0 1").unwrap();
        assert_eq!(promo_board.state.pawn_key, promo_board.compute_pawn_key());
        assert_ne!(promo_board.state.pawn_key, 0);

        let promo_mv = promo_board
            .try_make_move(Move::from_str("e7d8q").unwrap())
            .unwrap();
        assert_eq!(promo_board.state.pawn_key, 0);
        assert_eq!(promo_board.state.pawn_key, promo_board.compute_pawn_key());

        promo_board.unmake_move(promo_mv);
        assert_eq!(promo_board.state.pawn_key, promo_board.compute_pawn_key());
    }

    #[test]
    fn detects_insufficient_material() {
        let insufficient_fens = [
            "4k3/8/8/8/8/8/8/4K3 w - - 0 1",     // K vs K
            "4k3/8/8/8/8/8/8/4K1N1 w - - 0 1",   // K+N vs K
            "4k1n1/8/8/8/8/8/8/4K3 w - - 0 1",   // K vs K+N
            "4k3/8/8/8/8/8/8/4KB2 w - - 0 1",    // K+B vs K (light bishop)
            "4k3/8/8/8/8/8/8/2B1K3 w - - 0 1",   // K+B vs K (dark bishop)
            "2b1k3/8/8/8/8/8/8/4KB2 w - - 0 1",  // K+B vs K+B (both light)
            "4kb2/8/8/8/8/8/8/2B1K3 w - - 0 1",  // K+B vs K+B (both dark)
            "4k3/8/8/8/5B2/8/8/2B1K3 w - - 0 1", // K+B+B vs K (same color)
        ];
        for fen in insufficient_fens {
            let board = Board::from_fen(fen).unwrap();
            assert!(
                board.is_insufficient_material(),
                "Expected insufficient material for {fen}"
            );
        }

        let sufficient_fens = [
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1",   // K+P vs K
            "4k3/8/8/8/8/8/8/4K2R w - - 0 1",    // K+R vs K
            "4k3/8/8/8/8/8/8/4K2Q w - - 0 1",    // K+Q vs K
            "2b1k3/8/8/8/8/8/8/2B1K3 w - - 0 1", // K+B vs K+B (opposite colors)
            "4k3/8/8/8/8/8/8/2B1KB2 w - - 0 1",  // K+B+B vs K (bishop pair)
            "4k3/8/8/8/8/8/8/1N2K1N1 w - - 0 1", // K+N+N vs K
            "4k1n1/8/8/8/8/8/8/4K1N1 w - - 0 1", // K+N vs K+N
            "4k3/8/8/8/8/8/8/4KBN1 w - - 0 1",   // K+B+N vs K
            "4kb2/8/8/8/8/8/8/4K1N1 w - - 0 1",  // K+N vs K+B
        ];
        for fen in sufficient_fens {
            let board = Board::from_fen(fen).unwrap();
            assert!(
                !board.is_insufficient_material(),
                "Expected sufficient material for {fen}"
            );
        }
    }
}
