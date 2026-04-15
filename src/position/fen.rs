use std::fmt;

#[derive(Debug)]
enum FenErr {
    InvalidFormat,
    InvalidPiece,
}

type Result<T> = std::result::Result<T, FenErr>;

impl fmt::Display for FenErr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FenErr::InvalidFormat => write!(f, "Invalid format!"),
            FenErr::InvalidPiece => write!(f, "Invalid piece!"),
        }
    }
}

impl std::error::Error for FenErr {}

pub fn parse(fen: &str) -> Result<Position> {
    let mut parts = fen.split_whitespace();

    let mut next = || parts.next().ok_or(FenErr::InvalidFormat);

    let bboards = Self::parse_bboards(next()?)?;
    let side_to_move = Self::parse_side(next()?)?;
    let castling_rights = Self::parse_castling(next()?)?;
    let enpassant_eq = Self::parse_enpassant(next()?)?;
    let halfmove_clock = Self::parse_halfmove(next()?)?;
    let fullmove_counter = Self::parse_fullmove(next()?)?;

    Ok(Position {
        bboards,
        side_to_move,
        castling_rights,
        enpassant_eq,
        halfmove_clock,
        fullmove_counter
    })
}

fn parse_bboards(fen_part: &str) -> Result<[Bitboard; 12]> {
    let mut bboards = [Self::EMPTY; 64];
    let mut rank = 7;
    let mut file = 0;

    for &c in fen_part.as_bytes() {
        match c {
            b'/' => {
                if file != 8 { return Err(FenErr::InvalidFormat); }
                rank -= 1;
                file = 0;
            }
            b'1'..=b'8' => {
                file += (c - b'0') as usize;
            }
            _ => {
                let (col, pcs) = piece::from_char(c).ok_or(FenErr::InvalidPiece)?;
                let pcs_idx = Self::get_idx(col, pcs);

                let sq = square::sq_idx(rank, file);
                bboards[pcs_idx] |= 1u64 << sq;

                file += 1;
            }
        }
    }
    Ok(bboards)
}

fn parse_side(fen_part: &str) -> Result<Color> {
    match fen_part {
        "w" => Ok(Color::White),
        "b" => Ok(Color::Black),
        _ => Err(FenErr::InvalidFormat),
    }
}

fn parse_castling(fen_part: &str) -> Result<[bool; 4]> {
    let mut castling_rights = [false; 4];

    for &c in fen_part.as_bytes() {
        match c {
            b'K' => { castling_rights[0] = true; }
            b'Q' => { castling_rights[1] = true; }
            b'k' => { castling_rights[2] = true; }
            b'q' => { castling_rights[3] = true; }
            b'-' => { castling_rights = [false; 4]; break; }
            _ => { return Err(FenErr::InvalidFormat); }
        }
    }
    Ok(castling_rights)
}

fn parse_enpassant(fen_part: &str) -> Result<u32> {

}

fn parse_halfmove(fen_part: &str) -> Result<u32> {

}

fn parse_fullmove(fen_part: &str) -> Result<u32> {

}

