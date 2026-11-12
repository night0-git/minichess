use std::io::{BufRead, Write};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;

use crate::chess::Move;
use crate::chess::piece::Color;
use crate::engine::search::{
    SearchLimits, TranspositionTable, UciWriter, search_position, send_line,
};
use crate::position::board::Board;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Error {
    #[error("invalid position argument: {0}")]
    InvalidPosition(String),
    #[error("invalid move '{0}': {1}")]
    InvalidMove(String, String),
    #[error("invalid FEN '{0}': {1}")]
    InvalidFen(String, String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineOptions {
    pub name: String,
    pub author: String,
    pub hash_size_mb: usize,
}

impl EngineOptions {
    pub const DEFAULT_NAME: &'static str = "minichess";
    pub const DEFAULT_AUTHOR: &'static str = "night0";
    pub const DEFAULT_HASH_MB: usize = 16;
    pub const MIN_HASH_MB: usize = 1;
    pub const MAX_HASH_MB: usize = 1024;
}

impl Default for EngineOptions {
    fn default() -> Self {
        Self {
            name: Self::DEFAULT_NAME.to_string(),
            author: Self::DEFAULT_AUTHOR.to_string(),
            hash_size_mb: Self::DEFAULT_HASH_MB,
        }
    }
}

/// Parse a `setoption name <id> [value <x>]` command into (name, optional value)
pub fn parse_setoption(line: &str) -> Option<(String, Option<String>)> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.is_empty() || tokens[0] != "setoption" {
        return None;
    }
    let name_idx = tokens.iter().position(|&t| t == "name")?;
    let value_idx = tokens.iter().position(|&t| t == "value");

    match value_idx {
        Some(v_idx) if v_idx > name_idx => {
            let name = tokens[name_idx + 1..v_idx].join(" ");
            let val = tokens[v_idx + 1..].join(" ");
            Some((name, Some(val)))
        }
        _ => {
            let name = tokens[name_idx + 1..].join(" ");
            Some((name, None))
        }
    }
}

/// Parse a `position [startpos | fen <fen>] [moves <m1> ...]` command into a `Board`
pub fn parse_position(line: &str) -> Result<Board, Error> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.is_empty() || tokens[0] != "position" {
        return Err(Error::InvalidPosition("not a position command".to_string()));
    }

    if tokens.len() < 2 {
        return Err(Error::InvalidPosition("missing position type".to_string()));
    }

    let moves_idx = tokens.iter().position(|&t| t == "moves");

    let (pos_tokens, move_tokens) = match moves_idx {
        Some(idx) => (&tokens[1..idx], &tokens[idx + 1..]),
        None => (&tokens[1..], &tokens[tokens.len()..]),
    };

    if pos_tokens.is_empty() {
        return Err(Error::InvalidPosition(
            "empty position specification".to_string(),
        ));
    }

    let mut board = match pos_tokens[0] {
        "startpos" => Board::default(),
        "fen" => {
            let fen_str = pos_tokens[1..].join(" ");
            Board::from_fen(&fen_str).map_err(|e| Error::InvalidFen(fen_str, e.to_string()))?
        }
        other => {
            return Err(Error::InvalidPosition(format!(
                "unknown position type: {other}"
            )));
        }
    };

    for &mv_str in move_tokens {
        let mv = Move::from_str(mv_str)
            .map_err(|e| Error::InvalidMove(mv_str.to_string(), e.to_string()))?;
        board
            .try_make_move(mv)
            .map_err(|e| Error::InvalidMove(mv_str.to_string(), e.to_string()))?;
    }

    Ok(board)
}

/// Time management constants for search allocation
const TIME_DIVISOR: u64 = 20;
const INC_DIVISOR: u64 = 2;
const MOVE_OVERHEAD_MS: u64 = 50;
const MOVETIME_OVERHEAD_MS: u64 = 5;
const DEFAULT_MAX_DEPTH: u32 = 6;

/// Parse a `go` command with parameters into `SearchLimits`
pub fn parse_go(line: &str, side_to_move: Color) -> SearchLimits {
    let parts: Vec<&str> = line.split_whitespace().collect();
    let mut depth = None;
    let mut movetime = None;
    let mut wtime = None;
    let mut btime = None;
    let mut winc = None;
    let mut binc = None;
    let mut infinite = false;

    let mut i = 1;
    while i < parts.len() {
        match parts[i] {
            "depth" => {
                if i + 1 < parts.len() {
                    depth = parts[i + 1].parse::<u32>().ok();
                    i += 1;
                }
            }
            "movetime" => {
                if i + 1 < parts.len() {
                    movetime = parts[i + 1].parse::<u64>().ok();
                    i += 1;
                }
            }
            "wtime" => {
                if i + 1 < parts.len() {
                    wtime = parts[i + 1].parse::<u64>().ok();
                    i += 1;
                }
            }
            "btime" => {
                if i + 1 < parts.len() {
                    btime = parts[i + 1].parse::<u64>().ok();
                    i += 1;
                }
            }
            "winc" => {
                if i + 1 < parts.len() {
                    winc = parts[i + 1].parse::<u64>().ok();
                    i += 1;
                }
            }
            "binc" => {
                if i + 1 < parts.len() {
                    binc = parts[i + 1].parse::<u64>().ok();
                    i += 1;
                }
            }
            "infinite" => {
                infinite = true;
            }
            _ => {}
        }
        i += 1;
    }

    let time_allocated = if let Some(mt) = movetime {
        Some(Duration::from_millis(
            mt.saturating_sub(MOVETIME_OVERHEAD_MS).max(1),
        ))
    } else if !infinite {
        let (my_time, my_inc) = match side_to_move {
            Color::White => (wtime, winc),
            Color::Black => (btime, binc),
        };
        if let Some(t) = my_time {
            let inc = my_inc.unwrap_or(0);
            let alloc = (t / TIME_DIVISOR) + (inc / INC_DIVISOR);
            let safe_alloc = alloc.min(t.saturating_sub(MOVE_OVERHEAD_MS)).max(1);
            Some(Duration::from_millis(safe_alloc))
        } else {
            None
        }
    } else {
        None
    };

    let max_depth = if depth.is_none() && time_allocated.is_none() && !infinite {
        Some(DEFAULT_MAX_DEPTH)
    } else {
        depth
    };

    SearchLimits {
        max_depth,
        time_allocated,
        infinite,
    }
}

pub struct UciSession {
    pub board: Board,
    pub options: EngineOptions,
    pub stop_signal: Arc<AtomicBool>,
    pub search_thread: Option<std::thread::JoinHandle<()>>,
    pub writer: UciWriter,
    pub tt: Arc<Mutex<TranspositionTable>>,
}

impl UciSession {
    pub fn new(writer: UciWriter) -> Self {
        Self {
            board: Board::default(),
            options: EngineOptions::default(),
            stop_signal: Arc::new(AtomicBool::new(false)),
            search_thread: None,
            writer,
            tt: Arc::new(Mutex::new(TranspositionTable::new(
                EngineOptions::DEFAULT_HASH_MB,
            ))),
        }
    }

    pub fn stop_search(&mut self) {
        self.stop_signal.store(true, Ordering::SeqCst);
        if let Some(handle) = self.search_thread.take() {
            let _ = handle.join();
        }
    }

    pub fn cleanup(&mut self) {
        self.stop_search();
    }

    /// Handles a single UCI command line. Returns true if the session should terminate.
    pub fn handle_line(&mut self, line: &str) -> bool {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return false;
        }

        // Clean up finished search handle if any
        if let Some(handle) = &self.search_thread
            && handle.is_finished()
        {
            let _ = self.search_thread.take().unwrap().join();
        }

        let first_word = trimmed.split_whitespace().next().unwrap_or("");
        match first_word {
            "uci" => {
                send_line(
                    &Some(Arc::clone(&self.writer)),
                    &format!("id name {}", self.options.name),
                );
                send_line(
                    &Some(Arc::clone(&self.writer)),
                    &format!("id author {}", self.options.author),
                );
                send_line(
                    &Some(Arc::clone(&self.writer)),
                    &format!(
                        "option name Hash type spin default {} min {} max {}",
                        EngineOptions::DEFAULT_HASH_MB,
                        EngineOptions::MIN_HASH_MB,
                        EngineOptions::MAX_HASH_MB
                    ),
                );
                send_line(&Some(Arc::clone(&self.writer)), "uciok");
            }
            "isready" => {
                send_line(&Some(Arc::clone(&self.writer)), "readyok");
            }
            "setoption" => {
                if let Some((name, val)) = parse_setoption(trimmed)
                    && name.eq_ignore_ascii_case("hash")
                    && let Some(val_str) = val
                    && let Ok(mb) = val_str.parse::<usize>()
                {
                    self.options.hash_size_mb =
                        mb.clamp(EngineOptions::MIN_HASH_MB, EngineOptions::MAX_HASH_MB);
                    self.tt.lock().unwrap().resize(self.options.hash_size_mb);
                }
            }
            "ucinewgame" => {
                self.stop_search();
                self.board = Board::default();
                self.tt.lock().unwrap().clear();
            }
            "position" => {
                self.stop_search();
                if let Ok(board) = parse_position(trimmed) {
                    self.board = board;
                }
            }
            "go" => {
                self.stop_search();
                self.stop_signal.store(false, Ordering::SeqCst);
                let limits = parse_go(trimmed, self.board.side_to_move);
                let board_clone = self.board.clone();
                let stop_sig_clone = Arc::clone(&self.stop_signal);
                let writer_clone = Arc::clone(&self.writer);
                let tt_clone = Arc::clone(&self.tt);

                let handle = std::thread::spawn(move || {
                    search_position(
                        board_clone,
                        limits,
                        stop_sig_clone,
                        Some(writer_clone),
                        Some(tt_clone),
                    );
                });
                self.search_thread = Some(handle);
            }
            "stop" => {
                self.stop_signal.store(true, Ordering::SeqCst);
            }
            "quit" => {
                self.stop_search();
                return true;
            }
            _ => {
                // Ignore unknown commands per UCI specification.
            }
        }

        false
    }
}

/// Run the UCI loop with custom reader and writer.
pub fn run_uci<R: BufRead, W: Write + Send + 'static>(reader: R, writer: W) {
    let writer: UciWriter = Arc::new(Mutex::new(writer));
    let mut session = UciSession::new(writer);

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if session.handle_line(&line) {
            break;
        }
    }

    session.cleanup();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct SharedBuffer(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedBuffer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().write(buf)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.0.lock().unwrap().flush()
        }
    }

    #[test]
    fn parses_setoption_commands() {
        let (name, val) = parse_setoption("setoption name Hash value 32").unwrap();
        assert_eq!(name, "Hash");
        assert_eq!(val, Some("32".to_string()));

        let (name, val) = parse_setoption("setoption name Clear Hash").unwrap();
        assert_eq!(name, "Clear Hash");
        assert_eq!(val, None);

        let mut session = UciSession::new(Arc::new(Mutex::new(Vec::new())));
        session.handle_line("setoption name Hash value 2048");
        assert_eq!(session.options.hash_size_mb, EngineOptions::MAX_HASH_MB);

        session.handle_line("setoption name Hash value 0");
        assert_eq!(session.options.hash_size_mb, EngineOptions::MIN_HASH_MB);
    }

    #[test]
    fn parses_position_startpos_and_moves() {
        let board = parse_position("position startpos").unwrap();
        assert_eq!(board, Board::default());

        let board = parse_position("position startpos moves e2e4 e7e5").unwrap();
        assert_eq!(board.side_to_move, Color::White);
        assert_eq!(board.fullmove_counter, 2);
    }

    #[test]
    fn parses_position_fen() {
        let board = parse_position("position fen 8/8/8/8/8/8/8/4K2k w - - 0 1 moves e1e2").unwrap();
        assert_eq!(board.side_to_move, Color::Black);
    }

    #[test]
    fn parses_position_errors() {
        let err = parse_position("position startpos moves e2").unwrap_err();
        assert!(matches!(err, Error::InvalidMove(m, _) if m == "e2"));

        let err = parse_position("position startpos moves e5e6").unwrap_err();
        assert!(matches!(err, Error::InvalidMove(m, _) if m == "e5e6"));

        let err = parse_position("position fen invalid_fen").unwrap_err();
        assert!(matches!(err, Error::InvalidFen(..)));
    }

    #[test]
    fn parses_go_parameters() {
        let limits = parse_go("go depth 7", Color::White);
        assert_eq!(limits.max_depth, Some(7));
        assert!(limits.time_allocated.is_none());
        assert!(!limits.infinite);

        let limits = parse_go("go movetime 500", Color::White);
        assert_eq!(limits.time_allocated, Some(Duration::from_millis(495)));

        let limits = parse_go("go wtime 10000 btime 10000 winc 200 binc 200", Color::White);
        assert_eq!(limits.time_allocated, Some(Duration::from_millis(600)));

        let limits = parse_go("go infinite", Color::White);
        assert!(limits.infinite);
    }

    #[test]
    fn uci_session_responds_to_uci_and_isready() {
        let input = b"uci\nisready\nquit\n";
        let out_buf = Arc::new(Mutex::new(Vec::new()));
        let writer = SharedBuffer(out_buf.clone());

        run_uci(&input[..], writer);

        let output = String::from_utf8(out_buf.lock().unwrap().clone()).unwrap();
        assert!(output.contains("id name minichess\n"));
        assert!(output.contains("id author night0\n"));
        assert!(output.contains("option name Hash type spin default 16 min 1 max 1024\n"));
        assert!(output.contains("uciok\n"));
        assert!(output.contains("readyok\n"));
    }

    #[test]
    fn uci_session_supports_custom_name_and_author() {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let writer: UciWriter = buffer.clone();
        let mut session = UciSession::new(writer);
        session.options.name = "CustomEngine".to_string();
        session.options.author = "CustomAuthor".to_string();
        session.handle_line("uci");

        let output = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        assert!(output.contains("id name CustomEngine\n"));
        assert!(output.contains("id author CustomAuthor\n"));
    }

    #[test]
    fn uci_session_handles_go_and_bestmove() {
        let input = b"position startpos\ngo depth 1\nquit\n";
        let out_buf = Arc::new(Mutex::new(Vec::new()));
        let writer = SharedBuffer(out_buf.clone());

        run_uci(&input[..], writer);

        let output = String::from_utf8(out_buf.lock().unwrap().clone()).unwrap();
        assert!(output.contains("info depth 1"));
        assert!(output.contains("bestmove "));
    }

    #[test]
    fn uci_session_handles_stop_during_infinite_search() {
        let input = b"position startpos\ngo infinite\nstop\nquit\n";
        let out_buf = Arc::new(Mutex::new(Vec::new()));
        let writer = SharedBuffer(out_buf.clone());

        run_uci(&input[..], writer);

        let output = String::from_utf8(out_buf.lock().unwrap().clone()).unwrap();
        assert!(output.contains("bestmove "));
    }
}
