/*
    Little-Endian Rank-File Mapping (LERF)
    8 | 56 57 58 59 60 61 62 63
    7 | 48 49 50 51 52 53 54 55
    6 | 40 41 42 43 44 45 46 47
    5 | 32 33 34 35 36 37 38 39
    4 | 24 25 26 27 28 29 30 31
    3 | 16 17 18 19 20 21 22 23
    2 |  8  9 10 11 12 13 14 15
    1 |  0  1  2  3  4  5  6  7
    ---------------------------
         a  b  c  d  e  f  g  h
*/

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    North = 8,
    South = -8,
    East = 1,
    West = -1,
    NorthEast = 9,
    NorthWest = 7,
    SouthEast = -7,
    SouthWest = -9,
}

impl Direction {
    /// Returns all possible move directions for iteration.
    pub const ALL: [Direction; 8] = [
        Direction::North, Direction::South, Direction::East, Direction::West,
        Direction::NorthEast, Direction::NorthWest, Direction::SouthEast, Direction::SouthWest,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bitboard(pub u64);

impl Bitboard {
    pub const EMPTY: Self = Self(0u64);
    pub const ALL: Self = Self(1u64);

    pub const FILE_A: u64     = 0x0101010101010101;
    pub const FILE_H: u64     = 0x8080808080808080;
    pub const RANK_1: u64     = 0x00000000000000FF;
    pub const RANK_8: u64     = 0xFF00000000000000;
    pub const DIAG_A1_H8: u64 = 0x8040201008040201;
    pub const DIAG_H1_A8: u64 = 0x0102040810204080;
    pub const LIGHT_SQRS: u64 = 0x55AA55AA55AA55AA;
    pub const DARK_SQRS: u64  = 0xAA55AA55AA55AA55;
}

impl Bitboard {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn is_empty(self) -> bool {
        self.0.count_ones() == 0
    }

    pub const fn is_any(self) -> bool {
        self.0.count_ones() != 0
    }

    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    pub const fn contains(self, square: u8) -> bool {
        (self.0 & (1 << square)) != 0
    }

    pub const fn with_bit(self, square: u8) -> Self {
        Self(self.0 | (1 << square))
    }

    pub const fn without_bit(self, square: u8) -> Self {
        Self(self.0 & !(1 << square))
    }

    pub const fn toggled_bit(self, square: u8) -> Self {
        Self(self.0 ^ (1 << square))
    }

    pub const fn cleared_ls1b(self) -> Self {
        Self(self.0 & self.0.wrapping_sub(1))
    }
    
    pub fn set_bit(&mut self, square: u8) {
        self.0 |= 1 << square;
    }

    pub fn remove_bit(&mut self, square: u8) {
        self.0 &= !(1 << square);
    }

    pub fn toggle_bit(&mut self, square: u8) {
        self.0 ^= (1 << square);
    }

    pub fn pop_ls1b(&mut self) -> u8 {
        let ls1b = self.0.trailing_zeros();
        self.0 &= !(1 << ls1b);
        ls1b as u8
    }

    pub const fn ls1b(self) -> u8 {
        self.0.trailing_zeros() as u8
    }

    pub const fn ms1b(self) -> u8 {
        63 - self.0.leading_zeros() as u8
    }

    pub const fn shift(self, direction: Direction) -> Self {
        let d = direction as i8;
        if d > 0 {
            Self(self.0 << d as u32)
        } else {
            Self(self.0 >> (d.abs() as u32))
        }
    }

    pub const fn rotate_180(self) -> Self {
        // Use rust built-in method instead of doing
        // manual delta swaps
        Self(self.0.reverse_bits())
    }

    pub const fn mirror_vertical(self) -> Self {
        // Rust also has a built-in method for this
        Self(self.0.swap_bytes())
    }

    pub const fn mirror_horizontal(self) -> Self {
        let mut x = self.0;
        let k1 = 0x5555555555555555;
        let k2 = 0x3333333333333333;
        let k4 = 0x0F0F0F0F0F0F0F0F;
        x = ((x >> 1) & k1) | ((x & k1) << 1);
        x = ((x >> 2) & k2) | ((x & k2) << 2);
        x = ((x >> 4) & k4) | ((x & k4) << 4);
        Self(x)
    }
}

impl std::ops::BitAnd for Bitboard {
    type Output = Self;

    #[inline(always)]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl std::ops::BitOr for Bitboard {
    type Output = Self;

    #[inline(always)]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitXor for Bitboard {
    type Output = Self;

    #[inline(always)]
    fn bitxor(self, rhs: Self) -> Self::Output {
        Self(self.0 ^ rhs.0)
    }
}

impl std::ops::Not for Bitboard {
    type Output = Self;

    #[inline(always)]
    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

impl std::ops::BitAndAssign for Bitboard {
    #[inline(always)]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}
impl std::ops::BitOrAssign for Bitboard {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs;
    }
}
impl std::ops::BitXorAssign for Bitboard {
    #[inline(always)]
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs;
    }
}

impl Iterator for Bitboard {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0 == 0 {
            None
        } else {
            // Find ls1b
            let index = self.0.trailing_zeros() as u8;
            
            // Clear the ls1b
            self.0 &= self.0.wrapping_sub(1);
            
            Some(index)
        }
    }
}

impl std::fmt::Display for Bitboard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "  +-----------------+")?;
        for rank in (0..8).rev() {
            write!(f, "{} | ", rank + 1)?;
            for file in 0..8 {
                let square = rank * 8 + file;
                if self.contains(square) {
                    write!(f, "X ")?; // Bit is set
                } else {
                    write!(f, ". ")?; // Bit is empty
                }
            }
            writeln!(f, "|")?;
        }
        writeln!(f, "  +-----------------+")?;
        writeln!(f, "    a b c d e f g h")
    }
}



