use super::piece::Kind;
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

    pub const fn new_with_flag(from: Square, to: Square, flag: Flag) -> Self {
        Self(((flag as u32) << 12) | (to.as_u32() << 6) | from.as_u32())
    }

    pub const fn new_promo(from: Square, to: Square, promo: Kind) -> Self {
        let flag = match Flag::promo(promo, false) {
            Some(flag) => flag,
            None => Flag::QueenPromo,
        };
        Self::new_with_flag(from, to, flag)
    }

    pub const fn new_promo_capture(from: Square, to: Square, promo: Kind) -> Self {
        let flag = match Flag::promo(promo, true) {
            Some(flag) => flag,
            None => Flag::QueenPromoCapture,
        };
        Self::new_with_flag(from, to, flag)
    }

    pub const fn from(self) -> Square {
        Square::new((self.0 & 0x3F) as u8)
    }

    pub const fn to(self) -> Square {
        Square::new(((self.0 >> 6) & 0x3F) as u8)
    }

    pub const fn flag(self) -> Flag {
        Flag::from_u8(((self.0 >> 12) & 0x0F) as u8)
    }

    pub const fn with_flag(self, flag: Flag) -> Self {
        Self((self.0 & !0xF000) | (((flag as u32) & 0x0F) << 12))
    }

    pub const fn promotion(self) -> Option<Kind> {
        match self.flag() {
            Flag::KnightPromo | Flag::KnightPromoCapture => Some(Kind::Knight),
            Flag::BishopPromo | Flag::BishopPromoCapture => Some(Kind::Bishop),
            Flag::RookPromo | Flag::RookPromoCapture => Some(Kind::Rook),
            Flag::QueenPromo | Flag::QueenPromoCapture => Some(Kind::Queen),
            _ => None,
        }
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
            let promo_flag = match promo_char.to_ascii_lowercase() {
                'q' => Flag::QueenPromo,
                'r' => Flag::RookPromo,
                'b' => Flag::BishopPromo,
                'n' => Flag::KnightPromo,
                _ => return Err(ParseError::InvalidPromotion(promo_char)),
            };
            return Ok(Self::new_with_flag(from, to, promo_flag));
        }

        Ok(Self::new(from, to))
    }
}

impl std::fmt::Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.promotion() {
            Some(Kind::Queen) => write!(f, "{}{}q", self.from(), self.to()),
            Some(Kind::Rook) => write!(f, "{}{}r", self.from(), self.to()),
            Some(Kind::Bishop) => write!(f, "{}{}b", self.from(), self.to()),
            Some(Kind::Knight) => write!(f, "{}{}n", self.from(), self.to()),
            _ => write!(f, "{}{}", self.from(), self.to()),
        }
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

impl Flag {
    #[inline(always)]
    pub const fn is_capture(self) -> bool {
        (self as u8 & 0b0100) != 0
    }

    #[inline(always)]
    pub const fn is_promotion(self) -> bool {
        (self as u8 & 0b1000) != 0
    }

    /// Returns the promotion flag for `kind` (`Knight`/`Bishop`/`Rook`/`Queen`),
    /// or `None` for non-promotable piece kinds.
    pub const fn promo(kind: Kind, is_capture: bool) -> Option<Self> {
        match (kind, is_capture) {
            (Kind::Knight, false) => Some(Self::KnightPromo),
            (Kind::Knight, true) => Some(Self::KnightPromoCapture),
            (Kind::Bishop, false) => Some(Self::BishopPromo),
            (Kind::Bishop, true) => Some(Self::BishopPromoCapture),
            (Kind::Rook, false) => Some(Self::RookPromo),
            (Kind::Rook, true) => Some(Self::RookPromoCapture),
            (Kind::Queen, false) => Some(Self::QueenPromo),
            (Kind::Queen, true) => Some(Self::QueenPromoCapture),
            _ => None,
        }
    }

    pub const fn from_u8(val: u8) -> Self {
        match val & 0x0F {
            0 => Flag::Quiet,
            1 => Flag::DoublePawnPush,
            2 => Flag::KingCastle,
            3 => Flag::QueenCastle,
            4 => Flag::Capture,
            5 => Flag::EpCapture,
            6 => Flag::Reserved6,
            7 => Flag::Reserved7,
            8 => Flag::KnightPromo,
            9 => Flag::BishopPromo,
            10 => Flag::RookPromo,
            11 => Flag::QueenPromo,
            12 => Flag::KnightPromoCapture,
            13 => Flag::BishopPromoCapture,
            14 => Flag::RookPromoCapture,
            _ => Flag::QueenPromoCapture,
        }
    }
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

    #[test]
    fn parses_and_formats_promotion_moves() {
        let mv_q = Move::from_str("e7e8q").unwrap();
        assert_eq!(mv_q.promotion(), Some(Kind::Queen));
        assert_eq!(mv_q.to_string(), "e7e8q");

        let mv_n = Move::from_str("e2e1n").unwrap();
        assert_eq!(mv_n.promotion(), Some(Kind::Knight));
        assert_eq!(mv_n.to_string(), "e2e1n");

        let mv_r = Move::from_str("a7a8r").unwrap();
        assert_eq!(mv_r.promotion(), Some(Kind::Rook));
        assert_eq!(mv_r.to_string(), "a7a8r");

        let mv_b = Move::from_str("h2h1b").unwrap();
        assert_eq!(mv_b.promotion(), Some(Kind::Bishop));
        assert_eq!(mv_b.to_string(), "h2h1b");
    }

    #[test]
    fn flag_helpers_classify_captures_and_promotions() {
        assert!(!Flag::Quiet.is_capture());
        assert!(!Flag::Quiet.is_promotion());
        assert!(!Flag::DoublePawnPush.is_capture());
        assert!(!Flag::KingCastle.is_capture());
        assert!(Flag::Capture.is_capture());
        assert!(Flag::EpCapture.is_capture());
        assert!(Flag::QueenPromo.is_promotion());
        assert!(!Flag::QueenPromo.is_capture());
        assert!(Flag::KnightPromoCapture.is_promotion());
        assert!(Flag::KnightPromoCapture.is_capture());

        assert_eq!(Flag::promo(Kind::Queen, false), Some(Flag::QueenPromo));
        assert_eq!(Flag::promo(Kind::Rook, true), Some(Flag::RookPromoCapture));
        assert_eq!(Flag::promo(Kind::Pawn, false), None);

        let promo_cap = Move::new_promo_capture(
            Square::from_str("e7").unwrap(),
            Square::from_str("d8").unwrap(),
            Kind::Knight,
        );
        assert_eq!(promo_cap.flag(), Flag::KnightPromoCapture);
        assert_eq!(promo_cap.promotion(), Some(Kind::Knight));
    }
}
