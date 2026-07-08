use minichess::chess::*;
use minichess::position::*;
use minichess::move_gen::*;
use std::str::FromStr;

fn main() {
    let mut board = Board::default();
    board.make_move(Move::new(
        Square::from_str("e2").unwrap(), Square::from_str("e4").unwrap(), r#move::Flag::Quiet
    ));
    println!("{}", board);


    let mut moves = generator::gen_all_moves(&board);
    for m in moves {
        print!("{} ", m);
    }
}
