//! Compile-time generator for magic bitboards and move-masking geometry.
//!
//! Writes the following static arrays to `OUT_DIR` for `include!()` integration:
//! * `bishop_magics.rs` / `rook_magics.rs`: Hashing parameters (`Magic` structs).
//! * `bishop_attacks.rs` / `rook_attacks.rs`: Precomputed attack bitboards.
//! * `ray_between.rs`: Squares strictly between two aligned squares.
//! * `line_through.rs`: Unobstructed rays extending through two aligned squares.

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

const BISHOP_TABLE_SIZE: usize = 5248;
const ROOK_TABLE_SIZE: usize = 102400;

#[derive(Clone, Copy)]
struct Magic {
    mask: u64,
    magic: u64,
    shift: u8,
    offset: usize,
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir);

    // Generate magic tables
    let (b_magics, b_attacks) = generate_bishop_state();
    let (r_magics, r_attacks) = generate_rook_state();

    write_table(out_path.join("bishop_magics.rs"), &b_magics, format_magic);
    write_table(
        out_path.join("bishop_attacks.rs"),
        &b_attacks,
        format_attack,
    );
    write_table(out_path.join("rook_magics.rs"), &r_magics, format_magic);
    write_table(out_path.join("rook_attacks.rs"), &r_attacks, format_attack);

    // Generate geometry tables
    let (ray_between, line_through) = generate_geometry_tables();

    write_geometry_table(out_path.join("ray_between.rs"), &ray_between);
    write_geometry_table(out_path.join("line_through.rs"), &line_through);
}

fn write_table<T, F>(path: std::path::PathBuf, data: &[T], formatter: F)
where
    F: Fn(&T) -> String,
{
    let mut f = File::create(path).unwrap();
    writeln!(f, "[").unwrap();
    for item in data {
        writeln!(f, "    {},", formatter(item)).unwrap();
    }
    writeln!(f, "]").unwrap();
}

fn format_magic(m: &Magic) -> String {
    format!(
        "Magic {{ mask: {}, magic: {}, shift: {}, offset: {} }}",
        m.mask, m.magic, m.shift, m.offset
    )
}

fn format_attack(a: &u64) -> String {
    format!("Bitboard::new({})", a)
}

fn generate_bishop_state() -> ([Magic; 64], Vec<u64>) {
    let mut magics = [Magic {
        mask: 0,
        magic: 0,
        shift: 0,
        offset: 0,
    }; 64];
    let mut attacks = vec![0; BISHOP_TABLE_SIZE];
    let mut offset = 0;

    for sq in 0..64 {
        let mask = get_bishop_mask(sq as u8);
        let magic = BISHOP_MAGICS[sq];
        let active_bits = BISHOP_SHIFTS[sq];
        let shift = 64 - active_bits;

        magics[sq] = Magic {
            mask,
            magic,
            shift,
            offset,
        };

        let permutations = generate_blocker_permutations(mask);
        for perm in permutations {
            let hash = (perm.wrapping_mul(magic)) >> shift;
            let index = offset + hash as usize;
            attacks[index] = calculate_bishop_attacks(sq as u8, perm);
        }
        offset += 1 << active_bits;
    }
    (magics, attacks)
}

fn generate_rook_state() -> ([Magic; 64], Vec<u64>) {
    let mut magics = [Magic {
        mask: 0,
        magic: 0,
        shift: 0,
        offset: 0,
    }; 64];
    let mut attacks = vec![0; ROOK_TABLE_SIZE];
    let mut offset = 0;

    for sq in 0..64 {
        let mask = get_rook_mask(sq as u8);
        let magic = ROOK_MAGICS[sq];
        let active_bits = ROOK_SHIFTS[sq];
        let shift = 64 - active_bits;

        magics[sq] = Magic {
            mask,
            magic,
            shift,
            offset,
        };

        let permutations = generate_blocker_permutations(mask);
        for perm in permutations {
            let hash = (perm.wrapping_mul(magic)) >> shift;
            let index = offset + hash as usize;
            attacks[index] = calculate_rook_attacks(sq as u8, perm);
        }
        offset += 1 << active_bits;
    }
    (magics, attacks)
}

fn generate_blocker_permutations(mask: u64) -> Vec<u64> {
    let mut subsets = Vec::new();
    let mut subset: u64 = 0;
    loop {
        subsets.push(subset);
        subset = subset.wrapping_sub(mask) & mask;
        if subset == 0 {
            break;
        }
    }
    subsets
}

fn get_bishop_mask(sq: u8) -> u64 {
    let rank = sq as i8 / 8;
    let file = sq as i8 % 8;
    let mut mask = 0u64;
    let directions = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

    for &(dr, df) in &directions {
        let (mut r, mut f) = (rank + dr, file + df);
        while r > 0 && r < 7 && f > 0 && f < 7 {
            mask |= 1u64 << (r * 8 + f);
            r += dr;
            f += df;
        }
    }
    mask
}

fn calculate_bishop_attacks(sq: u8, blockers: u64) -> u64 {
    let rank = sq as i8 / 8;
    let file = sq as i8 % 8;
    let mut attacks = 0u64;
    let directions = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

    for &(dr, df) in &directions {
        let (mut r, mut f) = (rank + dr, file + df);
        while (0..8).contains(&r) && (0..8).contains(&f) {
            let bit = 1u64 << (r * 8 + f);
            attacks |= bit;
            if (blockers & bit) != 0 {
                break;
            }
            r += dr;
            f += df;
        }
    }
    attacks
}

fn get_rook_mask(sq: u8) -> u64 {
    let rank = sq as i8 / 8;
    let file = sq as i8 % 8;
    let mut mask = 0u64;

    for r in (rank + 1)..7 {
        mask |= 1u64 << (r * 8 + file);
    }
    for r in (1..rank).rev() {
        mask |= 1u64 << (r * 8 + file);
    }
    for f in (file + 1)..7 {
        mask |= 1u64 << (rank * 8 + f);
    }
    for f in (1..file).rev() {
        mask |= 1u64 << (rank * 8 + f);
    }

    mask
}

fn calculate_rook_attacks(sq: u8, blockers: u64) -> u64 {
    let rank = sq as i8 / 8;
    let file = sq as i8 % 8;
    let mut attacks = 0u64;

    for r in (rank + 1)..8 {
        let bit = 1u64 << (r * 8 + file);
        attacks |= bit;
        if (blockers & bit) != 0 {
            break;
        }
    }
    for r in (0..rank).rev() {
        let bit = 1u64 << (r * 8 + file);
        attacks |= bit;
        if (blockers & bit) != 0 {
            break;
        }
    }
    for f in (file + 1)..8 {
        let bit = 1u64 << (rank * 8 + f);
        attacks |= bit;
        if (blockers & bit) != 0 {
            break;
        }
    }
    for f in (0..file).rev() {
        let bit = 1u64 << (rank * 8 + f);
        attacks |= bit;
        if (blockers & bit) != 0 {
            break;
        }
    }
    attacks
}

pub const BISHOP_SHIFTS: [u8; 64] = [
    6, 5, 5, 5, 5, 5, 5, 6, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 7, 7, 7, 7, 5, 5, 5, 5, 7, 9, 9, 7, 5, 5,
    5, 5, 7, 9, 9, 7, 5, 5, 5, 5, 7, 7, 7, 7, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 6, 5, 5, 5, 5, 5, 5, 6,
];

pub const BISHOP_MAGICS: [u64; 64] = [
    9368648609924554880,
    9009475591934976,
    4504776450605056,
    1130334595844096,
    1725202480235520,
    288516396277699584,
    613618303369805920,
    10168455467108368,
    9046920051966080,
    36031066926022914,
    1152925941509587232,
    9301886096196101,
    290536121828773904,
    5260205533369993472,
    7512287909098426400,
    153141218749450240,
    9241386469758076456,
    5352528174448640064,
    2310346668982272096,
    1154049638051909890,
    282645627930625,
    2306405976892514304,
    11534281888680707074,
    72339630111982113,
    8149474640617539202,
    2459884588819024896,
    11675583734899409218,
    1196543596102144,
    5774635144585216,
    145242600416216065,
    2522607328671633440,
    145278609400071184,
    5101802674455216,
    650979603259904,
    9511646410653040801,
    1153493285013424640,
    18016048314974752,
    4688397299729694976,
    9226754220791842050,
    4611969694574863363,
    145532532652773378,
    5265289125480634376,
    288239448330604544,
    2395019802642432,
    14555704381721968898,
    2324459974457168384,
    23652833739932677,
    282583111844497,
    4629880776036450560,
    5188716322066279440,
    146367151686549765,
    1153170821083299856,
    2315697107408912522,
    2342448293961403408,
    2309255902098161920,
    469501395595331584,
    4615626809856761874,
    576601773662552642,
    621501155230386208,
    13835058055890469376,
    3748138521932726784,
    9223517207018883457,
    9237736128969216257,
    1127068154855556,
];

pub const ROOK_SHIFTS: [u8; 64] = [
    12, 11, 11, 11, 11, 11, 11, 12, 11, 10, 10, 10, 10, 10, 10, 11, 11, 10, 10, 10, 10, 10, 10, 11,
    11, 10, 10, 10, 10, 10, 10, 11, 11, 10, 10, 10, 10, 10, 10, 11, 11, 10, 10, 10, 10, 10, 10, 11,
    11, 10, 10, 10, 10, 10, 10, 11, 12, 11, 11, 11, 11, 11, 11, 12,
];

pub const ROOK_MAGICS: [u64; 64] = [
    612498416294952992,
    2377936612260610304,
    36037730568766080,
    72075188908654856,
    144119655536003584,
    5836666216720237568,
    9403535813175676288,
    1765412295174865024,
    3476919663777054752,
    288300746238222339,
    9288811671472386,
    146648600474026240,
    3799946587537536,
    704237264700928,
    10133167915730964,
    2305983769267405952,
    9223634270415749248,
    10344480540467205,
    9376496898355021824,
    2323998695235782656,
    9241527722809755650,
    189159985010188292,
    2310421375767019786,
    4647717014536733827,
    5585659813035147264,
    1442911135872321664,
    140814801969667,
    1188959108457300100,
    288815318485696640,
    758869733499076736,
    234750139167147013,
    2305924931420225604,
    9403727128727390345,
    9223970239903959360,
    309094713112139074,
    38290492990967808,
    3461016597114651648,
    181289678366835712,
    4927518981226496513,
    1155212901905072225,
    36099167912755202,
    9024792514543648,
    4611826894462124048,
    291045264466247688,
    83880127713378308,
    1688867174481936,
    563516973121544,
    9227888831703941123,
    703691741225216,
    45203259517829248,
    693563138976596032,
    4038638777286134272,
    865817582546978176,
    13835621555058516608,
    11541041685463296,
    288511853443695360,
    283749161902275,
    176489098445378,
    2306124759338845321,
    720584805193941061,
    4977040710267061250,
    10097633331715778562,
    325666550235288577,
    1100057149646,
];

fn generate_geometry_tables() -> ([[u64; 64]; 64], [[u64; 64]; 64]) {
    let mut ray_between = [[0u64; 64]; 64];
    let mut line_through = [[0u64; 64]; 64];

    for sq1 in 0..64 {
        for sq2 in 0..64 {
            if sq1 == sq2 {
                continue;
            }

            let r1 = (sq1 / 8) as i8;
            let f1 = (sq1 % 8) as i8;
            let r2 = (sq2 / 8) as i8;
            let f2 = (sq2 % 8) as i8;

            let dr = r2 - r1;
            let df = f2 - f1;

            if dr == 0 || df == 0 || dr.abs() == df.abs() {
                let step_r = dr.signum();
                let step_f = df.signum();

                // Ray Between
                let mut current_r = r1 + step_r;
                let mut current_f = f1 + step_f;
                let mut between = 0u64;

                while current_r != r2 || current_f != f2 {
                    between |= 1u64 << (current_r * 8 + current_f);
                    current_r += step_r;
                    current_f += step_f;
                }
                ray_between[sq1 as usize][sq2 as usize] = between;

                // Line Through
                let mut line = 0u64;
                let mut current_r = r1;
                let mut current_f = f1;

                // Trace forward
                while (0..8).contains(&current_r) && (0..8).contains(&current_f) {
                    line |= 1u64 << (current_r * 8 + current_f);
                    current_r += step_r;
                    current_f += step_f;
                }

                // Trace backward
                current_r = r1 - step_r;
                current_f = f1 - step_f;
                while (0..8).contains(&current_r) && (0..8).contains(&current_f) {
                    line |= 1u64 << (current_r * 8 + current_f);
                    current_r -= step_r;
                    current_f -= step_f;
                }

                line_through[sq1 as usize][sq2 as usize] = line;
            }
        }
    }

    (ray_between, line_through)
}

fn write_geometry_table(path: std::path::PathBuf, data: &[[u64; 64]; 64]) {
    let mut f = File::create(path).unwrap();
    writeln!(f, "[").unwrap();
    for row in data {
        writeln!(f, "    [").unwrap();
        for &val in row {
            writeln!(f, "        Bitboard::new({}),", val).unwrap();
        }
        writeln!(f, "    ],").unwrap();
    }
    writeln!(f, "]").unwrap();
}
