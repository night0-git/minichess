use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::chess::Move;
use crate::engine::evaluate;
use crate::position::board::Board;

// i32::MIN has an absolute value 1 greater than i32::MAX, negating it will cause an overflow.
const NEG_INF: i32 = i32::MIN + 1;
const POS_INF: i32 = i32::MAX;

const CHECK_STOP_INTERVAL: u64 = 2048;

pub type UciWriter = Arc<Mutex<dyn Write + Send>>;

#[derive(Debug, Clone, Default)]
pub struct SearchLimits {
    pub max_depth: Option<u32>,
    pub time_allocated: Option<Duration>,
    pub infinite: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Bound {
    Exact = 0,
    LowerBound = 1,
    UpperBound = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TTEntry {
    pub key: u64,
    pub best_move: Option<Move>,
    pub score: i32,
    pub depth: u8,
    pub bound: Bound,
    pub age: u8,
}

impl Default for TTEntry {
    fn default() -> Self {
        Self {
            key: 0,
            best_move: None,
            score: 0,
            depth: 0,
            bound: Bound::Exact,
            age: 0,
        }
    }
}

pub struct TranspositionTable {
    entries: Vec<TTEntry>,
    mask: usize,
    age: u8,
}

impl TranspositionTable {
    pub fn new(size_mb: usize) -> Self {
        let entry_size = std::mem::size_of::<TTEntry>();
        let target_bytes = size_mb.max(1) * 1024 * 1024;
        let mut count = (target_bytes / entry_size).next_power_of_two();
        if count * entry_size > target_bytes && count > 1 {
            count /= 2;
        }
        let count = count.max(1);

        Self {
            entries: vec![TTEntry::default(); count],
            mask: count - 1,
            age: 0,
        }
    }

    #[inline(always)]
    pub fn probe(&self, key: u64) -> Option<&TTEntry> {
        let index = (key as usize) & self.mask;
        let entry = &self.entries[index];
        if entry.key == key { Some(entry) } else { None }
    }

    pub fn store(
        &mut self,
        key: u64,
        depth: u8,
        score: i32,
        bound: Bound,
        best_move: Option<Move>,
    ) {
        let index = (key as usize) & self.mask;
        let existing = &self.entries[index];

        if existing.key == 0 || self.age != existing.age || depth >= existing.depth {
            let preserved_move = if best_move.is_none() && existing.key == key {
                existing.best_move
            } else {
                best_move
            };

            self.entries[index] = TTEntry {
                key,
                best_move: preserved_move,
                score,
                depth,
                bound,
                age: self.age,
            };
        } else if existing.key == key && existing.best_move.is_none() && best_move.is_some() {
            self.entries[index].best_move = best_move;
        }
    }

    pub fn new_search(&mut self) {
        self.age = self.age.wrapping_add(1);
    }

    pub fn clear(&mut self) {
        for entry in &mut self.entries {
            *entry = TTEntry::default();
        }
    }

    pub fn resize(&mut self, size_mb: usize) {
        *self = Self::new(size_mb);
    }

    pub fn extract_pv(&self, board: &Board, max_depth: usize) -> Vec<Move> {
        let mut pv = Vec::new();
        let mut current_board = board.clone();
        let mut visited = std::collections::HashSet::new();

        for _ in 0..max_depth {
            let key = current_board.state.zobrist_key;
            if !visited.insert(key) {
                break;
            }

            if let Some(entry) = self.probe(key)
                && let Some(mv) = entry.best_move
            {
                let legal_moves = crate::move_gen::gen_all_moves(&current_board);
                if legal_moves.contains(&mv) {
                    pv.push(mv);
                    current_board.make_move(mv);
                    continue;
                }
            }
            break;
        }

        pv
    }
}

pub struct SearchContext<'a> {
    pub stop_signal: Arc<AtomicBool>,
    pub start_time: Instant,
    pub max_time: Option<Duration>,
    pub nodes: u64,
    pub aborted: bool,
    pub tt: &'a mut TranspositionTable,
}

impl<'a> SearchContext<'a> {
    pub fn new(
        stop_signal: Arc<AtomicBool>,
        max_time: Option<Duration>,
        tt: &'a mut TranspositionTable,
    ) -> Self {
        Self {
            stop_signal,
            start_time: Instant::now(),
            max_time,
            nodes: 0,
            aborted: false,
            tt,
        }
    }

    #[inline(always)]
    pub fn increment_nodes(&mut self) {
        self.nodes += 1;
        // Equals to self.nodes % CHECK_STOP_INTERVAL
        if self.nodes & (CHECK_STOP_INTERVAL - 1) == 0 {
            self.check_stop();
        }
    }

    pub fn check_stop(&mut self) {
        if self.stop_signal.load(Ordering::Relaxed) {
            self.aborted = true;
            return;
        }
        if let Some(max_time) = self.max_time
            && self.start_time.elapsed() >= max_time
        {
            self.aborted = true;
        }
    }
}

pub fn send_line(output: &Option<UciWriter>, line: &str) {
    if let Some(w) = output {
        if let Ok(mut lock) = w.lock() {
            let _ = writeln!(lock, "{line}");
            let _ = lock.flush();
        }
    } else {
        println!("{line}");
    }
}

pub fn alpha_beta_search(
    board: &mut Board,
    mut alpha: i32,
    beta: i32,
    depth_left: i32,
    ctx: &mut SearchContext,
) -> i32 {
    ctx.increment_nodes();
    if ctx.aborted {
        return 0;
    }

    let key = board.state.zobrist_key;
    let tt_entry = ctx.tt.probe(key).copied();
    let tt_move = tt_entry.and_then(|e| e.best_move);

    if let Some(entry) = tt_entry
        && entry.depth as i32 >= depth_left
    {
        match entry.bound {
            Bound::Exact => return entry.score,
            Bound::LowerBound => {
                if entry.score >= beta {
                    return entry.score;
                }
            }
            Bound::UpperBound => {
                if entry.score <= alpha {
                    return entry.score;
                }
            }
        }
    }

    if depth_left == 0 {
        return quiesce(board, alpha, beta, ctx);
    }

    let mut moves = crate::move_gen::gen_all_moves(board);
    if moves.is_empty() {
        return NEG_INF;
    }

    // Move ordering: prioritize TT best move to maximize beta cutoffs
    if let Some(best_mv) = tt_move
        && let Some(pos) = moves.iter().position(|&m| m == best_mv)
    {
        moves.swap(0, pos);
    }

    let original_alpha = alpha;
    let mut best_value = NEG_INF;
    let mut best_move = None;

    for mv in moves {
        board.make_move(mv);
        let score = -alpha_beta_search(board, -beta, -alpha, depth_left - 1, ctx);
        board.unmake_move(mv);

        if ctx.aborted {
            return 0;
        }

        if score > best_value {
            best_value = score;
            best_move = Some(mv);
        }
        if score > alpha {
            alpha = score;
        }
        if score >= beta {
            break;
        }
    }

    if !ctx.aborted {
        let bound = if best_value >= beta {
            Bound::LowerBound
        } else if best_value > original_alpha {
            Bound::Exact
        } else {
            Bound::UpperBound
        };

        ctx.tt
            .store(key, depth_left.max(0) as u8, best_value, bound, best_move);
    }

    best_value
}

/// Quiescence search for the maximizing player
fn quiesce(board: &mut Board, mut alpha: i32, beta: i32, ctx: &mut SearchContext) -> i32 {
    ctx.increment_nodes();
    if ctx.aborted {
        return 0;
    }

    // The static evaluation establishes the baseline "stand pat" score
    let stand_pat = evaluate(board);

    if stand_pat >= beta {
        return beta;
    }
    if alpha < stand_pat {
        alpha = stand_pat;
    }

    // Generate only capturing moves to resolve tactical tension
    let captures = crate::move_gen::gen_qsearch_moves(board);

    for mv in captures {
        board.make_move(mv);
        let score = -quiesce(board, -beta, -alpha, ctx);
        board.unmake_move(mv);

        if ctx.aborted {
            return 0;
        }

        if score >= beta {
            return beta;
        }
        if score > alpha {
            alpha = score;
        }
    }

    alpha
}

/// Search the current position within given limits and output UCI info / bestmove.
pub fn search_position(
    mut board: Board,
    limits: SearchLimits,
    stop_signal: Arc<AtomicBool>,
    output: Option<UciWriter>,
    shared_tt: Option<Arc<Mutex<TranspositionTable>>>,
) -> Option<Move> {
    let mut root_moves = crate::move_gen::gen_all_moves(&board);
    if root_moves.is_empty() {
        send_line(&output, "bestmove 0000");
        return None;
    }

    let mut local_tt;
    let mut tt_guard;
    let tt: &mut TranspositionTable = if let Some(shared) = &shared_tt {
        tt_guard = shared.lock().unwrap();
        &mut tt_guard
    } else {
        local_tt = TranspositionTable::new(16);
        &mut local_tt
    };

    tt.new_search();

    let mut ctx = SearchContext::new(stop_signal, limits.time_allocated, tt);
    let max_depth = limits.max_depth.unwrap_or(64);

    let mut last_completed_best_move: Option<Move> = None;

    for depth in 1..=max_depth {
        if ctx.aborted {
            break;
        }

        // Prioritize last completed best move at root.
        if let Some(best_mv) = last_completed_best_move
            && let Some(pos) = root_moves.iter().position(|&m| m == best_mv)
        {
            root_moves.swap(0, pos);
        }

        let mut alpha = NEG_INF;
        let beta = POS_INF;
        let mut best_score_this_depth = NEG_INF;
        let mut best_move_this_depth = None;

        for &mv in &root_moves {
            board.make_move(mv);
            let score = -alpha_beta_search(&mut board, -beta, -alpha, (depth as i32) - 1, &mut ctx);
            board.unmake_move(mv);

            if ctx.aborted {
                break;
            }

            if score > best_score_this_depth {
                best_score_this_depth = score;
                best_move_this_depth = Some(mv);
            }
            if score > alpha {
                alpha = score;
            }
        }

        if ctx.aborted {
            break;
        }

        last_completed_best_move = best_move_this_depth;

        // Store root position in TT.
        if let Some(mv) = best_move_this_depth {
            ctx.tt.store(
                board.state.zobrist_key,
                depth as u8,
                best_score_this_depth,
                Bound::Exact,
                Some(mv),
            );
        }

        let elapsed = ctx.start_time.elapsed();
        let time_ms = elapsed.as_millis() as u64;
        let nps = (ctx.nodes * 1000)
            .checked_div(time_ms)
            .unwrap_or(ctx.nodes * 1000);

        // Extract full PV via Transposition Table.
        let pv = ctx.tt.extract_pv(&board, depth as usize);
        let pv_str = if pv.is_empty() {
            best_move_this_depth
                .map(|m| m.to_string())
                .unwrap_or_default()
        } else {
            pv.iter()
                .map(|m| m.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        };

        send_line(
            &output,
            &format!(
                "info depth {depth} score cp {best_score_this_depth} nodes {} nps {nps} time {time_ms} pv {pv_str}",
                ctx.nodes
            ),
        );

        if limits.max_depth.is_some_and(|md| depth >= md) {
            break;
        }

        if let Some(allocated) = limits.time_allocated
            && (elapsed >= allocated || elapsed.as_millis() * 2 >= allocated.as_millis())
        {
            break;
        }
    }

    let chosen_move = last_completed_best_move.or_else(|| root_moves.first().copied());
    if let Some(mv) = chosen_move {
        send_line(&output, &format!("bestmove {mv}"));
        Some(mv)
    } else {
        send_line(&output, "bestmove 0000");
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn search_completes_iterative_deepening() {
        let board = Board::default();
        let stop_signal = Arc::new(AtomicBool::new(false));
        let limits = SearchLimits {
            max_depth: Some(2),
            time_allocated: None,
            infinite: false,
        };
        let output_buf = Arc::new(Mutex::new(Vec::new()));
        let best_move = search_position(board, limits, stop_signal, Some(output_buf.clone()), None);
        assert!(best_move.is_some());
        let output = String::from_utf8(output_buf.lock().unwrap().clone()).unwrap();
        assert!(output.contains("info depth 1"));
        assert!(output.contains("info depth 2"));
        assert!(output.contains("bestmove "));
    }

    #[test]
    fn tt_stores_and_probes_entry() {
        let mut tt = TranspositionTable::new(1);
        let key = 0x1234_5678_9ABC_DEF0;
        let mv = crate::chess::Move::from_str("e2e4").unwrap();

        assert!(tt.probe(key).is_none());

        tt.store(key, 4, 150, Bound::Exact, Some(mv));
        let entry = tt.probe(key).expect("Entry should be found");
        assert_eq!(entry.key, key);
        assert_eq!(entry.depth, 4);
        assert_eq!(entry.score, 150);
        assert_eq!(entry.bound, Bound::Exact);
        assert_eq!(entry.best_move, Some(mv));
    }

    #[test]
    fn tt_replaces_shallow_with_deep() {
        let mut tt = TranspositionTable::new(1);
        let key = 0xABCD;

        tt.store(key, 2, 50, Bound::UpperBound, None);
        assert_eq!(tt.probe(key).unwrap().depth, 2);

        // Deeper search replaces it
        tt.store(key, 5, 80, Bound::Exact, None);
        assert_eq!(tt.probe(key).unwrap().depth, 5);
        assert_eq!(tt.probe(key).unwrap().score, 80);

        // Shallower search does not replace it in the same generation
        tt.store(key, 3, 20, Bound::LowerBound, None);
        assert_eq!(tt.probe(key).unwrap().depth, 5);
    }

    #[test]
    fn tt_clears_and_resizes() {
        let mut tt = TranspositionTable::new(1);
        let key = 0xDEADBEEF;
        tt.store(key, 3, 100, Bound::Exact, None);
        assert!(tt.probe(key).is_some());

        tt.clear();
        assert!(tt.probe(key).is_none());

        tt.resize(2);
        assert!(tt.probe(key).is_none());
    }

    #[test]
    fn tt_reduces_search_node_count() {
        let board = Board::default();
        let stop_signal = Arc::new(AtomicBool::new(false));

        // First search without shared TT (cold search)
        let mut tt = TranspositionTable::new(1);
        let mut ctx = SearchContext::new(stop_signal.clone(), None, &mut tt);
        let mut b1 = board.clone();
        alpha_beta_search(&mut b1, NEG_INF, POS_INF, 3, &mut ctx);
        let nodes_first_pass = ctx.nodes;

        // Second search with populated TT
        let mut ctx2 = SearchContext::new(stop_signal, None, &mut tt);
        let mut b2 = board.clone();
        alpha_beta_search(&mut b2, NEG_INF, POS_INF, 3, &mut ctx2);
        let nodes_second_pass = ctx2.nodes;

        // The second search should immediately hit the TT and explore fewer nodes
        assert!(
            nodes_second_pass < nodes_first_pass,
            "Populated TT should reduce nodes: first {nodes_first_pass}, second {nodes_second_pass}"
        );
    }

    #[test]
    fn tt_extracts_pv() {
        let board = Board::default();
        let stop_signal = Arc::new(AtomicBool::new(false));
        let limits = SearchLimits {
            max_depth: Some(3),
            time_allocated: None,
            infinite: false,
        };
        let shared_tt = Arc::new(Mutex::new(TranspositionTable::new(1)));
        let output_buf = Arc::new(Mutex::new(Vec::new()));
        let best_move = search_position(
            board.clone(),
            limits,
            stop_signal,
            Some(output_buf.clone()),
            Some(shared_tt.clone()),
        );
        assert!(best_move.is_some());

        let tt = shared_tt.lock().unwrap();
        let pv = tt.extract_pv(&board, 3);
        assert!(!pv.is_empty());
        assert_eq!(pv[0], best_move.unwrap());
    }
}
