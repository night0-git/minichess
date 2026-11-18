use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::chess::Move;
use crate::engine::evaluate;
use crate::engine::pawn::PawnTable;
use crate::position::board::Board;

const NEG_INF: i32 = -32_000;
const POS_INF: i32 = 32_000;
pub const MATE_SCORE: i32 = 30_000;
pub const MATE_THRESHOLD: i32 = 29_000;

const CHECK_STOP_INTERVAL: u64 = 2048;

#[inline(always)]
fn score_to_tt(score: i32, ply: i32) -> i32 {
    if score >= MATE_THRESHOLD {
        score + ply
    } else if score <= -MATE_THRESHOLD {
        score - ply
    } else {
        score
    }
}

#[inline(always)]
fn score_from_tt(score: i32, ply: i32) -> i32 {
    if score >= MATE_THRESHOLD {
        score - ply
    } else if score <= -MATE_THRESHOLD {
        score + ply
    } else {
        score
    }
}

#[derive(Debug, Clone, Default)]
pub struct SearchLimits {
    pub max_depth: Option<u32>,
    pub time_allocated: Option<Duration>,
    pub infinite: bool,
}

/// Telemetry produced after each completed iterative-deepening depth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchInfo {
    pub depth: u32,
    pub score: i32,
    pub nodes: u64,
    pub nps: u64,
    pub time_ms: u64,
    pub pv: Vec<Move>,
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
        let mut visited = HashSet::new();
        self.extend_pv(&mut current_board, &mut visited, &mut pv, max_depth);
        pv
    }

    fn extend_pv(
        &self,
        current_board: &mut Board,
        visited: &mut HashSet<u64>,
        pv: &mut Vec<Move>,
        max_len: usize,
    ) {
        for _ in 0..max_len {
            let key = current_board.state.zobrist_key;
            if !visited.insert(key) {
                break;
            }

            if let Some(entry) = self.probe(key)
                && let Some(mv) = entry.best_move
            {
                let legal_moves = crate::move_gen::gen_all_moves(current_board);
                if legal_moves.contains(&mv) {
                    pv.push(mv);
                    current_board.make_move(mv);
                    if current_board.is_threefold_repetition() || current_board.is_fifty_move_draw()
                    {
                        break;
                    }
                    continue;
                }
            }
            break;
        }
    }
}

pub struct SearchContext<'a> {
    pub stop_signal: Arc<AtomicBool>,
    pub start_time: Instant,
    pub max_time: Option<Duration>,
    pub nodes: u64,
    pub aborted: bool,
    pub tt: &'a mut TranspositionTable,
    pub pawn_table: PawnTable,
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
            pawn_table: PawnTable::default(),
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

pub fn alpha_beta_search(
    board: &mut Board,
    mut alpha: i32,
    beta: i32,
    depth_left: i32,
    ply: i32,
    ctx: &mut SearchContext,
) -> i32 {
    ctx.increment_nodes();
    if ctx.aborted {
        return 0;
    }

    // Check path-dependent draws before probing the TT.
    if ply > 0 && board.is_twofold_repetition() {
        return 0;
    }
    if ply > 0 && board.is_fifty_move_draw() && !crate::move_gen::is_in_check(board) {
        return 0;
    }

    let key = board.state.zobrist_key;
    let tt_entry = ctx.tt.probe(key).copied();
    let tt_move = tt_entry.and_then(|e| e.best_move);

    if let Some(entry) = tt_entry
        && entry.depth as i32 >= depth_left
    {
        let tt_score = score_from_tt(entry.score, ply);
        match entry.bound {
            Bound::Exact => return tt_score,
            Bound::LowerBound => {
                if tt_score >= beta {
                    return tt_score;
                }
            }
            Bound::UpperBound => {
                if tt_score <= alpha {
                    return tt_score;
                }
            }
        }
    }

    if depth_left <= 0 {
        return quiesce(board, alpha, beta, ctx);
    }

    let mut moves = crate::move_gen::gen_all_moves(board);
    if moves.is_empty() {
        return if crate::move_gen::is_in_check(board) {
            -MATE_SCORE + ply
        } else {
            0
        };
    }
    if ply > 0 && board.is_fifty_move_draw() {
        return 0;
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
        let score = -alpha_beta_search(board, -beta, -alpha, depth_left - 1, ply + 1, ctx);
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

        ctx.tt.store(
            key,
            depth_left.max(0) as u8,
            score_to_tt(best_value, ply),
            bound,
            best_move,
        );
    }

    best_value
}

/// Quiescence search for the maximizing player.
fn quiesce(board: &mut Board, mut alpha: i32, beta: i32, ctx: &mut SearchContext) -> i32 {
    ctx.increment_nodes();
    if ctx.aborted {
        return 0;
    }

    // The static evaluation establishes the baseline "stand pat" score.
    let stand_pat = evaluate::evaluate_with_pawn_table(board, &mut ctx.pawn_table);

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

/// Search the current position within given limits, invoking `on_info` after each completed depth.
pub fn search_position<F>(
    mut board: Board,
    limits: SearchLimits,
    stop_signal: Arc<AtomicBool>,
    shared_tt: Option<Arc<Mutex<TranspositionTable>>>,
    mut on_info: F,
) -> Option<Move>
where
    F: FnMut(SearchInfo),
{
    let mut root_moves = crate::move_gen::gen_all_moves(&board);
    if root_moves.is_empty() {
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
            let score =
                -alpha_beta_search(&mut board, -beta, -alpha, (depth as i32) - 1, 1, &mut ctx);
            board.unmake_move(mv);

            if ctx.aborted {
                break;
            }

            if best_move_this_depth.is_none() || score > best_score_this_depth {
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
                score_to_tt(best_score_this_depth, 0),
                Bound::Exact,
                Some(mv),
            );
        }

        let elapsed = ctx.start_time.elapsed();
        let time_ms = elapsed.as_millis() as u64;
        let nps = (ctx.nodes * 1000)
            .checked_div(time_ms)
            .unwrap_or(ctx.nodes * 1000);

        // Seed PV with the chosen root move so pv[0] always matches bestmove,
        // then extend via TT while stopping on threefold repetition / 50-move draw.
        let mut pv = Vec::new();
        if let Some(mv) = best_move_this_depth {
            pv.push(mv);
            let mut pv_board = board.clone();
            let mut visited = HashSet::from([pv_board.state.zobrist_key]);
            pv_board.make_move(mv);
            if !pv_board.is_threefold_repetition() && !pv_board.is_fifty_move_draw() {
                ctx.tt.extend_pv(
                    &mut pv_board,
                    &mut visited,
                    &mut pv,
                    (depth as usize).saturating_sub(1),
                );
            }
        }

        on_info(SearchInfo {
            depth,
            score: best_score_this_depth,
            nodes: ctx.nodes,
            nps,
            time_ms,
            pv,
        });

        if limits.max_depth.is_some_and(|md| depth >= md) {
            break;
        }

        // Searching a new depth would just abort mid-search anyway.
        if let Some(allocated) = limits.time_allocated
            && (elapsed >= allocated || elapsed.as_millis() * 2 >= allocated.as_millis())
        {
            break;
        }
    }

    last_completed_best_move.or_else(|| root_moves.first().copied())
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
        let mut infos = Vec::new();
        let best_move = search_position(board, limits, stop_signal, None, |info| infos.push(info));
        assert!(best_move.is_some());
        assert_eq!(infos.len(), 2);
        assert_eq!(infos[0].depth, 1);
        assert_eq!(infos[1].depth, 2);
        assert!(!infos[1].pv.is_empty());
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
        alpha_beta_search(&mut b1, NEG_INF, POS_INF, 3, 0, &mut ctx);
        let nodes_first_pass = ctx.nodes;

        // Second search with populated TT
        let mut ctx2 = SearchContext::new(stop_signal, None, &mut tt);
        let mut b2 = board.clone();
        alpha_beta_search(&mut b2, NEG_INF, POS_INF, 3, 0, &mut ctx2);
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
        let best_move = search_position(
            board.clone(),
            limits,
            stop_signal,
            Some(shared_tt.clone()),
            |_| {},
        );
        assert!(best_move.is_some());

        let tt = shared_tt.lock().unwrap();
        let pv = tt.extract_pv(&board, 3);
        assert!(!pv.is_empty());
        assert_eq!(pv[0], best_move.unwrap());
    }

    #[test]
    fn forced_mate_bestmove_matches_pv_and_scores_mate() {
        // Every legal reply (Kg8 or a2a1*) allows Rc1c8#.
        let board = Board::from_fen("7k/8/6K1/8/8/8/p7/2R5 b - - 0 1").unwrap();
        let stop_signal = Arc::new(AtomicBool::new(false));
        let limits = SearchLimits {
            max_depth: Some(4),
            time_allocated: None,
            infinite: false,
        };
        let mut infos = Vec::new();
        let best_move = search_position(board, limits, stop_signal, None, |info| infos.push(info));
        let chosen = best_move.expect("Expected a legal move even when getting mated");
        let last_info = infos.last().expect("Expected search info");
        assert_eq!(last_info.pv.first().copied(), Some(chosen));
        assert!(
            last_info.score <= -MATE_THRESHOLD,
            "Expected negative mate score, got {}",
            last_info.score
        );
    }

    #[test]
    fn stalemate_scores_zero() {
        let mut board = Board::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").unwrap();
        let stop_signal = Arc::new(AtomicBool::new(false));
        let mut tt = TranspositionTable::new(1);
        let mut ctx = SearchContext::new(stop_signal, None, &mut tt);
        let score = alpha_beta_search(&mut board, NEG_INF, POS_INF, 2, 1, &mut ctx);
        assert_eq!(score, 0);
    }

    #[test]
    fn pv_stops_at_threefold_repetition() {
        // After 7 plies of knight shuffling, Black playing f6g8 creates a 3rd occurrence of startpos.
        let mut board = Board::default();
        for mv_str in ["g1f3", "g8f6", "f3g1", "f6g8", "g1f3", "g8f6", "f3g1"] {
            board
                .try_make_move(Move::from_str(mv_str).unwrap())
                .unwrap();
        }

        let mut tt = TranspositionTable::new(1);
        let rep_mv = board
            .try_make_move(Move::from_str("f6g8").unwrap())
            .unwrap();
        let after_rep_key = board.state.zobrist_key;
        board.unmake_move(rep_mv);

        // Seed TT with both the repeating move and a follow-up from the repeated position.
        tt.store(board.state.zobrist_key, 4, 0, Bound::Exact, Some(rep_mv));
        let cont_mv = Move::from_str("e2e4").unwrap();
        tt.store(after_rep_key, 3, 0, Bound::Exact, Some(cont_mv));

        let pv = tt.extract_pv(&board, 4);
        assert_eq!(pv, vec![rep_mv]);
    }
}
