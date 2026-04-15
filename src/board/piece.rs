#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    White = 0,
    Black = 1
}

impl Color {
    pub const ALL: [Self; 2] = [Self::White, Self::Black];
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceType {
    Pawn = 0,
    Knight = 1,
    Bishop = 2,
    Rook = 3,
    Queen = 4,
    King = 5
}

impl PieceType {
    pub const ALL: [Self; 6] = [
        Self::Pawn, Self::Knight, Self::Bishop,
        Self::Rook, Self::Queen, Self::King
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    color: Color,
    kind: PieceType
}

impl std::fmt::Display for Piece {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_char())
    }
}

impl Piece {
    pub const fn new(color: Color, kind: PieceType) -> Self {
        Self { color, kind }
    }

    // Intentionally not set as const for future proof
    pub fn from_char(c: char) -> Option<Self> {
        if !c.is_ascii() { return None; }

        match c as u8 {
            b'P' => Some(Self { color: Color::White, kind: PieceType::Pawn }),
            b'N' => Some(Self { color: Color::White, kind: PieceType::Knight }),
            b'B' => Some(Self { color: Color::White, kind: PieceType::Bishop }),
            b'R' => Some(Self { color: Color::White, kind: PieceType::Rook }),
            b'Q' => Some(Self { color: Color::White, kind: PieceType::Queen }),
            b'K' => Some(Self { color: Color::White, kind: PieceType::King }),
            b'p' => Some(Self { color: Color::Black, kind: PieceType::Pawn }),
            b'n' => Some(Self { color: Color::Black, kind: PieceType::Knight }),
            b'b' => Some(Self { color: Color::Black, kind: PieceType::Bishop }),
            b'r' => Some(Self { color: Color::Black, kind: PieceType::Rook }),
            b'q' => Some(Self { color: Color::Black, kind: PieceType::Queen }),
            b'k' => Some(Self { color: Color::Black, kind: PieceType::King }),
            _    => None,
        }
    }

    // Get chess piece notation
    pub const fn to_char(self) -> char {
        let mut b = match self.kind {
            PieceType::Pawn   => b'p',
            PieceType::Knight => b'n',
            PieceType::Bishop => b'b',
            PieceType::Rook   => b'r',
            PieceType::Queen  => b'q',
            PieceType::King   => b'k',
        };

        // If White, clear the 5th bit to make it uppercase
        match self.color {
            Color::White => b &= !0x20,
            Color::Black => (),
        }

        b as char
    }
}
