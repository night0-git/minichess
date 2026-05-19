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

    #[inline(always)]
    pub const fn rank(self) -> u8 { self.0 / 8 }

    #[inline(always)]
    pub const fn file(self) -> u8 { self.0 % 8 }

    #[inline(always)]
    pub const fn coords(self) -> (u8, u8) {
        (self.rank(), self.file())
    }

    #[inline(always)]
    pub const fn mask(self) -> u64 {
        1u64 << self.0
    }
}

// Conversion from string
impl std::str::FromStr for Square {
    type Err = &'static str;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let b = s.as_bytes();
        if b.len() != 2 { 
            return Err("Invalid algebraic notation length"); 
        }
        
        // Use ASCII arithmetic to map to 0-indexed integers
        let file = b[0].wrapping_sub(b'a');
        let rank = b[1].wrapping_sub(b'1');

        if file < 8 && rank < 8 {
            Ok(Self(rank * 8 + file))
        } else {
            Err("Invalid algebraic notation or coordinates")
        }
    }
}

// Allow for to_string() conversion
impl std::fmt::Display for Square {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", (b'a' + self.file()) as char, (b'1' + self.rank()) as char)
    }
}
