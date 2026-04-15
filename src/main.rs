mod board;
mod bitboard;
use crate::board::Board;
use crate::bitboard::{Bitboard, EMPTY};

fn main() {
    let mut board = Board::new();
    board.setup_start_position();
    board.print();
}
