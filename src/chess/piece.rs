#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    White = 0,
    Black = 1,
}

impl Color {
    pub const ALL: [Self; 2] = [Self::White, Self::Black];

    #[inline(always)]
    pub fn flip(self) -> Self {
        unsafe { std::mem::transmute((self as u8) ^ 1) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Pawn = 0,
    Knight = 1,
    Bishop = 2,
    Rook = 3,
    Queen = 4,
    King = 5,
}

impl Kind {
    pub const ALL: [Self; 6] = [
        Self::Pawn,
        Self::Knight,
        Self::Bishop,
        Self::Rook,
        Self::Queen,
        Self::King,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Piece {
    WPawn = 0,
    WKnight = 1,
    WBishop = 2,
    WRook = 3,
    WQueen = 4,
    WKing = 5,
    BPawn = 6,
    BKnight = 7,
    BBishop = 8,
    BRook = 9,
    BQueen = 10,
    BKing = 11,
}

impl std::fmt::Display for Piece {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_char())
    }
}

impl Piece {
    pub const ALL: [Self; 12] = [
        Self::WPawn,
        Self::WKnight,
        Self::WBishop,
        Self::WRook,
        Self::WQueen,
        Self::WKing,
        Self::BPawn,
        Self::BKnight,
        Self::BBishop,
        Self::BRook,
        Self::BQueen,
        Self::BKing,
    ];

    #[inline(always)]
    pub const fn new(color: Color, kind: Kind) -> Self {
        let index = (color as u8) * 6 + (kind as u8);
        unsafe { std::mem::transmute(index) }
    }

    #[inline(always)]
    pub const fn from_u8(value: u8) -> Self {
        unsafe { std::mem::transmute(value) }
    }

    #[inline(always)]
    pub const fn color(self) -> Color {
        if (self as u8) < 6 {
            Color::White
        } else {
            Color::Black
        }
    }

    #[inline(always)]
    pub const fn kind(self) -> Kind {
        unsafe { std::mem::transmute((self as u8) % 6) }
    }

    /// Construct Piece from a char (eg., P for white pawn, p for black pawn)
    pub const fn from_char(c: char) -> Option<Self> {
        match c as u8 {
            b'P' => Some(Self::WPawn),
            b'N' => Some(Self::WKnight),
            b'B' => Some(Self::WBishop),
            b'R' => Some(Self::WRook),
            b'Q' => Some(Self::WQueen),
            b'K' => Some(Self::WKing),
            b'p' => Some(Self::BPawn),
            b'n' => Some(Self::BKnight),
            b'b' => Some(Self::BBishop),
            b'r' => Some(Self::BRook),
            b'q' => Some(Self::BQueen),
            b'k' => Some(Self::BKing),
            _ => None,
        }
    }

    /// Construct a char from a Piece (eg., P for white pawn, p for black pawn)
    pub const fn to_char(self) -> char {
        const CHARS: [u8; 12] = *b"PNBRQKpnbrqk";
        CHARS[self as usize] as char
    }
}
