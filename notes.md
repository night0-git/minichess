### Proposed structure
chess/
```
├── Cargo.toml
├── src/
│   ├── lib.rs          // public API
│   ├── board/
│   │   ├── mod.rs
│   │   ├── board.rs
│   │   ├── move.rs
│   │   ├── piece.rs
│   │   └── square.rs
│   ├── movegen/
│   │   ├── mod.rs
│   │   ├── generator.rs
│   │   └── attacks.rs
│   ├── position/
│   │   ├── mod.rs
│   │   └── fen.rs
│   ├── search/
│   │   ├── mod.rs
│   │   ├── search.rs
│   │   └── eval.rs
│   ├── uci/            // optional inside lib OR separate bin
│   │   ├── mod.rs
│   │   └── parser.rs
│   └── types.rs
│
├── src/bin/
│   └── engine.rs       // UCI executable
```