use crate::chess::{self, Color, Piece};
use crate::chess::Bitboard;

#[derive(Clone, Copy, Debug)]
pub struct Position {
    bboards: [Bitboard; 12],
    side_to_move: Color,
    castling_rights: [bool; 4],
    enpassant_sq: Option<u32>,
    // Resets to 0 every capture or pawn move
    halfmove_clock: u32,
    // Starts at 1 and increments every black move
    fullmove_counter: u32
}

impl Position {
    pub fn from_fen(fen: &str) -> fen::Result<Self> {
        fen::parse(fen)
    }

    pub fn to_fen(&self) -> String {
        // Form a board to push as Piece Placement to the FEN
        let board = ".".repeat(64);
        for col in Color::ALL {
            for pcs in Piece::ALL {
                let idx = Self::get_idx(col, pcs);
                let mut bb = self.bboards[idx];

                while bb != 0 {
                    let sq = bb.pop_ls1b();
                    fen[sq] = piece::to_char(col, pcs);
                }
            }
        }

        let mut fen = "";
        // Append Piece Placement
        for rank in (0..8).rev() {
            let start = i * rank;
            let end = start + 8;
            let pieces = &board[start..end];

            let mut emp_sq = 0;
            for pcs in pieces {
                if pcs != '.' {
                    if emp_sq == 0 {
                        fen.push(emp_sq as char);
                        emp_sq = 0;
                    }
                    fen.push(pcs);
                } else {
                    emp_sq += 1;
                }
            }
        }
        // Separator between ranks
        if rank != 7 { fen.push(" /") };
        // Append Side To Move
        fen.push(if self.side_to_move == Color::White
                { " w" } else { " b" });
        let castle = String::new();
        for i in 0..4 {
            if self.castling_rights[i] {
                castle.push(match i {
                    0 => 'K',
                    1 => 'Q',
                    2 => 'k',
                    3 => 'q'
                });
            }
        }
        // Append Castling Rights
        fen.push(if castle != "" { ' ' + castle } else { ' -' });
        // Append Enpassant Square
        fen.push(' ' + square::notations_from_idx(enpassant_sq));
        // Append Halfmove Clock
        fen.push(' ' + self.halfmove_clock.to_string());
        // Append Fullmove Clock
        fen.push(' ' + self.fullmove_counter.to_string());
        fen
    }

    pub fn make_move(&mut self, mv: Move) -> bool {

    }

    pub fn unmake_move(&must self) -> bool {

    }
}

// Helpers
impl Position {
    pub fn initial() -> Self {
        let initial_pos = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        Self::from_fen(initial_pos)
    }

    // Map Color, Type to index
    pub fn get_idx(color: Color, piece: PieceType) -> usize {
        (color as usize * 6) + (piece as usize)
    }
}
