use minichess::chess::*;
use minichess::move_gen::*;
use minichess::position::*;
use std::str::FromStr;

fn main() {
    let mut board = Board::default();
    board.make_move(Move::from_str("e2e4").unwrap());
    println!("{}", board);

    let moves = generator::gen_all_moves(&board);
    for m in moves {
        print!("{} ", m);
    }
}
