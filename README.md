# minichesss
minichess is a chess library and a UCI-compatible engine built in Rust, using Negamax with Alpha-Beta pruning. It implements traditional tapered PeSTO scoring and techniques such as pawn structure for evaluation.

> [!NOTE]
> This is a work in progress. There are other algorithms still to be implemented to improve search and evaluation strength such as piece mobility, king safety, and optionally Texel's tuning to optimize weights.

## What it supports

minichess can be imported as a lightweight dependency in a chess game application for board representation, fast move generation and a decent engine.

It provides a `minichess` binary that runs a UCI loop, wiring to any GUI through standard output.

### Example

```rust
use std::str::FromStr;

use minichess::engine::{SearchLimits, search};
use minichess::move_gen::gen_all_moves;
use minichess::{Board, Move};

let mut board = Board::default();
let _legal_moves = gen_all_moves(&board);

board.try_make_move(Move::from_str("e2e4")?)?;

let best = search(&board, SearchLimits::depth(4))?;
println!("Best move: {best}");
```

## Benchmark
A new version can be verified using the [sprt](./.github/workflows/sprt.yml) workflow, which runs the [`sprt.sh`](./scripts/bench_sprt.sh) script, with number of games set to 100000 by default. It compares the latest version to a previous baseline commit using [`fastchess`](https://github.com/Disservin/fastchess), if the exported result shows an LLR (Log-Likelihood Ratio) past 2.94 then the newest model is better than the baseline.
