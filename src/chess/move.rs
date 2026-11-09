use super::square::{self, Square};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ParseError {
    #[error("invalid algebraic move length: expected at least 4 characters, found {0}")]
    InvalidLength(usize),
    #[error("invalid square in move: {0}")]
    Square(#[from] square::ParseError),
    #[error("invalid promotion piece: '{0}' (expected 'q', 'r', 'b', or 'n')")]
    InvalidPromotion(char),
}

pub type Error = ParseError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move(u32);

impl Move {
    pub const fn new(from: Square, to: Square) -> Self {
        Self((to.as_u32() << 6) | from.as_u32())
    }

    pub const fn from(self) -> Square {
        Square::new((self.0 & 0x3F) as u8)
    }

    pub const fn to(self) -> Square {
        Square::new(((self.0 >> 6) & 0x3F) as u8)
    }

    pub fn set_score(&mut self, score: u16) {
        self.0 = (self.0 & 0xFFFF) | ((score as u32) << 16);
    }

    pub const fn score(self) -> u16 {
        (self.0 >> 16) as u16
    }
}

impl std::str::FromStr for Move {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() < 4 {
            return Err(ParseError::InvalidLength(s.len()));
        }
        let from = Square::from_str(&s[0..2])?;
        let to = Square::from_str(&s[2..4])?;

        if s.len() >= 5 {
            let promo_char = s.as_bytes()[4] as char;
            match promo_char.to_ascii_lowercase() {
                'q' | 'r' | 'b' | 'n' => {}
                _ => return Err(ParseError::InvalidPromotion(promo_char)),
            }
        }

        Ok(Self::new(from, to))
    }
}

impl std::fmt::Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.from(), self.to())
    }
}

// Each flag is a 4-bit code with each bit represents a different meaning:
/* | promotion | capture | special 1 | special 0 | */
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)] // Force the enum to be 1 byte
pub enum Flag {
    Quiet = 0,
    DoublePawnPush = 1,
    KingCastle = 2,
    QueenCastle = 3,
    Capture = 4,
    EpCapture = 5,
    Reserved6 = 6,
    Reserved7 = 7,
    KnightPromo = 8,
    BishopPromo = 9,
    RookPromo = 10,
    QueenPromo = 11,
    KnightPromoCapture = 12,
    BishopPromoCapture = 13,
    RookPromoCapture = 14,
    QueenPromoCapture = 15,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn parses_valid_uci_move() {
        let mv = Move::from_str("e2e4").unwrap();
        assert_eq!(mv.from(), Square::from_str("e2").unwrap());
        assert_eq!(mv.to(), Square::from_str("e4").unwrap());
        assert_eq!(mv.to_string(), "e2e4");
    }

    #[test]
    fn rejects_invalid_uci_notation() {
        assert_eq!(Move::from_str("e2"), Err(ParseError::InvalidLength(2)));
        assert_eq!(
            Move::from_str("z9e4"),
            Err(ParseError::Square(square::ParseError::InvalidCoordinates {
                file: 'z',
                rank: '9'
            }))
        );
        assert_eq!(
            Move::from_str("e7e8x"),
            Err(ParseError::InvalidPromotion('x'))
        );
    }
}
