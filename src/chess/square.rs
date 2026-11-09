use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ParseError {
    #[error("invalid algebraic notation length: expected 2 characters, found {0}")]
    InvalidLength(usize),
    #[error("invalid coordinates: file '{file}' and rank '{rank}' out of bounds")]
    InvalidCoordinates { file: char, rank: char },
    #[error("square index out of bounds: {0} (must be 0..64)")]
    IndexOutOfBounds(u8),
}

pub type Error = ParseError;

/* Square conventions:
 * - Rank and file start from 0
 * - 'square' refers to the index of the square in LSF mapping
 * - Notation refers to the standard chess square notation
 * - Coordinates refer to (rank, file)
 */

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Square(u8);

impl Square {
    #[inline(always)]
    pub const fn new(s: u8) -> Self {
        debug_assert!(s < 64);
        Self(s)
    }

    #[inline(always)]
    pub const fn as_u8(self) -> u8 {
        self.0
    }

    #[inline(always)]
    pub const fn as_u32(self) -> u32 {
        self.0 as u32
    }

    #[inline(always)]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }

    #[inline(always)]
    pub const fn from_coords(rank: u8, file: u8) -> Self {
        debug_assert!(rank < 8 && file < 8);
        Self(rank * 8 + file)
    }

    pub fn try_from_coords(rank: u8, file: u8) -> Result<Self, ParseError> {
        if rank < 8 && file < 8 {
            Ok(Self(rank * 8 + file))
        } else {
            Err(ParseError::InvalidCoordinates {
                file: (b'a' + file) as char,
                rank: (b'1' + rank) as char,
            })
        }
    }

    #[inline(always)]
    pub const fn rank(self) -> u8 {
        self.0 / 8
    }

    #[inline(always)]
    pub const fn file(self) -> u8 {
        self.0 % 8
    }

    #[inline(always)]
    pub const fn coords(self) -> (u8, u8) {
        (self.rank(), self.file())
    }

    #[inline(always)]
    pub const fn mask(self) -> u64 {
        1u64 << self.0
    }
}

impl TryFrom<u8> for Square {
    type Error = ParseError;

    fn try_from(s: u8) -> Result<Self, Self::Error> {
        if s < 64 {
            Ok(Self(s))
        } else {
            Err(ParseError::IndexOutOfBounds(s))
        }
    }
}

// Conversion from string
impl std::str::FromStr for Square {
    type Err = ParseError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let b = s.as_bytes();
        if b.len() != 2 {
            return Err(ParseError::InvalidLength(s.len()));
        }

        // Use ASCII arithmetic to map to 0-indexed integers
        let file = b[0].wrapping_sub(b'a');
        let rank = b[1].wrapping_sub(b'1');

        if file < 8 && rank < 8 {
            Ok(Self(rank * 8 + file))
        } else {
            Err(ParseError::InvalidCoordinates {
                file: b[0] as char,
                rank: b[1] as char,
            })
        }
    }
}

impl std::fmt::Display for Square {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}",
            (b'a' + self.file()) as char,
            (b'1' + self.rank()) as char
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_square_from_str() {
        assert_eq!(Square::from_str("a1"), Ok(Square::new(0)));
        assert_eq!(Square::from_str("h8"), Ok(Square::new(63)));
        assert_eq!(Square::from_str("e4"), Ok(Square::new(28)));
    }

    #[test]
    fn test_square_from_str_errors() {
        assert_eq!(Square::from_str("e"), Err(ParseError::InvalidLength(1)));
        assert_eq!(Square::from_str("e44"), Err(ParseError::InvalidLength(3)));
        assert_eq!(
            Square::from_str("z9"),
            Err(ParseError::InvalidCoordinates {
                file: 'z',
                rank: '9'
            })
        );
    }

    #[test]
    fn test_square_try_from() {
        assert_eq!(Square::try_from(0), Ok(Square::new(0)));
        assert_eq!(Square::try_from(63), Ok(Square::new(63)));
        assert_eq!(Square::try_from(64), Err(ParseError::IndexOutOfBounds(64)));
    }
}
