/* Square conventions:
 * - Rank and file start from 0
 * - 'square' refers to the index of the square in LSF mapping
 * - Notation refers to the standard chess square notation
 * - Coordinates refer to (rank, file)
 */

use std::fmt;

// Custom square conversion error
#[derive(Debug)]
pub enum ConversionError {
    InvalidInput,
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput => write!(f, "Invalid algebraic notation or coordinates"),
        }
    }
}

impl std::error::Error for ConversionError {

}

// Typedef for convenience
type Result<T> = std::result::Result<T, ConversionError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Square(u8);

impl Square {
    #[inline(always)]
    pub const fn new(s: u8) -> Self {
        debug_assert!(s < 64);
        Self(s)
    }

    pub const fn rank(self) -> u8 { self.0 / 8 }
    pub const fn file(self) -> u8 { self.0 % 8 }

    pub const fn from_coords(rank: u8, file: u8) -> Option<Self> {
        if rank < 8 && file < 8 {
            Some(Self(rank * 8 + file))
        } else {
            None
        }
    }
}

// Conversion from string
impl std::str::FromStr for Square {
    type Err = ConversionError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let b = s.as_bytes();
        if b.len() != 2 { return Err(ConversionError::InvalidInput); }
        
        let file = b[0].wrapping_sub(b'a');
        let rank = b[1].wrapping_sub(b'1');

        if file < 8 && rank < 8 {
            Ok(Self(rank * 8 + file))
        } else {
            Err(ConversionError::InvalidInput)
        }
    }
}

impl std::fmt::Display for Square {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", (b'a' + self.file()) as char, (b'1' + self.rank()) as char)
    }
}
