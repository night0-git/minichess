use crate::chess::{Bitboard, Piece, Move, Square};
use crate::chess::piece::{Color, Kind};
use crate::chess::r#move::Flag;
use super::zobrist::ZOBRIST;
use super::fen;

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
     7, 15, 15, 15,  3, 15, 15, 11, // Rank 8: a8(BQ lost), e8(BK+BQ lost), h8(BK lost)
];

/// Stores irreversible board data, such data that cannot
/// have its previous state be mathematically deduced.
#[derive(Debug, Clone, Copy)]
pub struct State {
    pub castling_rights: u8,
    pub ep_square: Option<Square>,
    pub halfmove_clock: u32,
    pub captured_pcs: Option<Piece>,
    pub zobrist_key: u64,
}

#[derive(Debug, Clone)]
pub struct Board {
    pub bboards: [Bitboard; 12],
    pub pieces: [Option<Piece>; 64],

    pub color_masks: [Bitboard; 2], 
    pub all_occupancy: Bitboard,

    pub side_to_move: Color,
    pub fullmove_counter: u32,
    pub state: State,

    pub history: Vec<State>,
}

impl Board {
    pub fn from_fen(fen: &str) -> Result<Self, &'static str> {
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

    /// Retrieve any piece at the given square by checking all 12 bitboards.
    #[inline(always)]
    pub fn piece_at(&self, sq: Square) -> Option<Piece> {
        self.pieces[sq.as_usize()]
    }

    /// Retrieve a piece at the given square restricted to a specific color.
    #[inline(always)]
    pub fn piece_at_color(&self, sq: Square, color: Color) -> Option<Piece> {
        self.pieces[sq.as_usize()].filter(|p| p.color() == color)
    }

    pub fn make_move(&mut self, mv: Move) {
        self.history.push(self.state);

        let us = self.side_to_move;
        let them = us.flip();

        let from = mv.from();
        let to = mv.to();
        let flag = mv.flag();

        let moving_pcs = self.piece_at_color(from, us)
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
                    Color::White => (Square::new(7), Square::new(5)),   // h1 to f1
                    Color::Black => (Square::new(63), Square::new(61)), // h8 to f8
                };
                let rook = Piece::new(us, Kind::Rook);
                self.remove_piece(rook, r_from);
                self.put_piece(rook, r_to);
            }
            Flag::QueenCastle => {
                self.put_piece(moving_pcs, to);
                let (r_from, r_to) = match us {
                    Color::White => (Square::new(0), Square::new(3)),   // a1 to d1
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
                    Flag::RookPromo   | Flag::RookPromoCapture   => Kind::Rook,
                    Flag::QueenPromo  | Flag::QueenPromoCapture  => Kind::Queen,
                    _ => unreachable!(),
                };
                let promo_piece = Piece::new(us, promo_kind);
                self.put_piece(promo_piece, to);
            }
        }

        self.update_state(mv, moving_pcs, captured_pcs);
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
        let flag = mv.flag();

        // Identify and remove the moving piece from destination
        let moving_pcs = self.piece_at_color(to, us)
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
    fn update_state(&mut self, mv: Move, moving_pcs: Piece, captured_pcs: Option<Piece>) {
        self.state.captured_pcs = captured_pcs;

        // Update zobrist key: XOR out old state
        self.state.zobrist_key ^= ZOBRIST.castling[self.state.castling_rights as usize];
        if let Some(ep) = self.state.ep_square {
            self.state.zobrist_key ^= ZOBRIST.en_passant[ep.file() as usize];
        }

        // Update en passant target square
        if mv.flag() == Flag::DoublePawnPush {
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
        self.state.castling_rights &= CASTLING_MASKS[mv.from().as_usize()]
            & CASTLING_MASKS[mv.to().as_usize()];

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
}

impl Default for Board {
    fn default() -> Self {
        Self::from_fen(fen::STARTING_FEN)
            .expect("STARTING_FEN is invalid")
    }
}

impl std::fmt::Display for Board {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "  +-----------------+")?;
        for rank in (0..8).rev() {
            write!(f, "{} | ", rank + 1)?;
            for file in 0..8 {
                let sq = Square::from_coords(rank, file);
                if let Some(piece) = self.piece_at(sq) {
                    write!(f, "{} ", piece.to_char())?;
                } else {
                    write!(f, ". ")?;
                }
            }
            writeln!(f, "|")?;
        }
        writeln!(f, "  +-----------------+")?;
        writeln!(f, "    a b c d e f g h")?;
        
        writeln!(f, "FEN: {}", self.to_fen())?;
        writeln!(f, "Key: {:016X}", self.state.zobrist_key)
    }
}
