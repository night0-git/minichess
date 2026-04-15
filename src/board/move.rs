use super::piece::Piece;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move(u32);

impl std::fmt::Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.from(), self.to())
    }
}

impl Move {
    pub const fn new(from: u8, to: u8, flag: Flag) -> Self {
        Self(((flag as u32) << 12) | ((to as u32) << 6) | (from as u32))
    }

    pub const fn from(self) -> u8 {
        (self.0 & 0x3F) as u8
    }

    pub const fn to(self) -> u8 {
        ((self.0 >> 6) & 0x3F) as u8
    }

    pub fn set_score(&mut self, score: u16) {
        self.0 = (self.0 & 0xFFFF) | ((score as u32) << 16);
    }

    pub const fn score(self) -> u16 {
        (self.0 >> 16) as u16
    }
}

// Each flag is a 4-bit code with each bit represents a different meaning:
// | promotion | capture | special 1 | special 0 |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)] // Force the enum to be 1 byte
pub enum Flag {
    Quiet = 0,
    DoublePawnPush = 1,
    KingCastle = 2,
    QueenCastle = 3,
    Capture = 4,
    EpCapture = 5,
    // 6 and 7 are reserved
    KnightPromo = 8,
    BishopPromo = 9,
    RookPromo = 10,
    QueenPromo = 11,
    KnightPromoCapture = 12,
    BishopPromoCapture = 13,
    RookPromoCapture = 14,
    QueenPromoCapture = 15,
}
