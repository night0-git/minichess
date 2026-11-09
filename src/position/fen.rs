use super::board::Board;
use crate::chess::Square;
use crate::chess::piece::{Color, Piece};
use std::fmt::Write;

pub const STARTING_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

pub fn parse(fen: &str) -> Result<Board, &'static str> {
    let mut parts = fen.split_whitespace();
    let mut board = Board::empty();

    // Piece placement
    let placement = parts.next().ok_or("Missing piece placement")?;
    let mut rank = 7i8;
    let mut file = 0u8;

    for c in placement.chars() {
        if c == '/' {
            rank -= 1;
            file = 0;
        } else if let Some(digit) = c.to_digit(10) {
            file += digit as u8;
        } else if let Some(piece) = Piece::from_char(c) {
            let sq = Square::from_coords(rank as u8, file);
            board.put_piece_no_zobrist(piece, sq);
            file += 1;
        } else {
            return Err("Invalid piece character in FEN");
        }
    }

    // Active color
    let color = parts.next().ok_or("Missing active color")?;
    board.side_to_move = match color {
        "w" => Color::White,
        "b" => Color::Black,
        _ => return Err("Invalid active color"),
    };

    // Castling rights
    let castling = parts.next().ok_or("Missing castling availability")?;
    board.state.castling_rights = 0;
    if castling != "-" {
        for c in castling.chars() {
            match c {
                'K' => board.state.castling_rights |= 0b0001,
                'Q' => board.state.castling_rights |= 0b0010,
                'k' => board.state.castling_rights |= 0b0100,
                'q' => board.state.castling_rights |= 0b1000,
                _ => return Err("Invalid castling flag"),
            }
        }
    }

    // En Passant target
    let ep = parts.next().ok_or("Missing en passant target")?;
    if ep != "-" {
        board.state.ep_square = Some(ep.parse::<Square>().map_err(|_| "Invalid EP square")?);
    }

    // Halfmove clock
    board.state.halfmove_clock = parts
        .next()
        .unwrap_or("0")
        .parse::<u32>()
        .map_err(|_| "Invalid halfmove clock")?;

    // Fullmove counter
    board.fullmove_counter = parts
        .next()
        .unwrap_or("1")
        .parse::<u32>()
        .map_err(|_| "Invalid fullmove counter")?;

    // Finalize hash
    board.state.zobrist_key = board.compute_zobrist_key();

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
