use crate::board::{Board, Move};
use crate::piece::{Color, Piece};

// Produce pseudo-legal/legal moves

pub fn gen_moves(board: &Board) -> Vec<Move>


pub fn gen_pseudo_moves(board: &Board) -> Vec<Move>
