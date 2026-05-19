use minichess::chess::*;
use minichess::position::*;
use std::str::FromStr;

fn main() {
    let mut board = Board::default();
    println!("{}", board);

    board.make_move(Move::new(
        Square::from_str("e2").unwrap(), Square::from_str("e4").unwrap(), r#move::Flag::Quiet
    ));
    println!("{}", board);

    board.make_move(Move::new(
        Square::from_str("e7").unwrap(), Square::from_str("e5").unwrap(), r#move::Flag::Quiet
    ));
    println!("{}", board);
}
