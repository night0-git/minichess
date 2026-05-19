#[derive(Debug)]
pub struct Zobrist {
    pub pieces: [[u64; 64]; 12],
    pub castling: [u64; 16],
    pub en_passant: [u64; 8], // Indexed by file (0..7)
    pub side_to_move: u64,
}

struct PRNG { seed: u64 }

impl PRNG {
    const fn new(seed: u64) -> Self { Self { seed } }
    
    const fn rand(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
}

const fn init_zobrist() -> Zobrist {
    let mut prng = PRNG::new(0x98f10715e2bf2f05); // Arbitrary seed
    
    let mut z = Zobrist {
        pieces: [[0; 64]; 12],
        castling: [0; 16],
        en_passant: [0; 8],
        side_to_move: prng.rand(),
    };

    let mut p = 0;
    while p < 12 {
        let mut s = 0;
        while s < 64 {
            z.pieces[p][s] = prng.rand();
            s += 1;
        }
        p += 1;
    }

    let mut c = 0;
    while c < 16 {
        z.castling[c] = prng.rand();
        c += 1;
    }

    let mut f = 0;
    while f < 8 {
        z.en_passant[f] = prng.rand();
        f += 1;
    }

    z
}

pub static ZOBRIST: Zobrist = init_zobrist();
