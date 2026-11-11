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

pub struct SearchContext {
    pub stop_signal: Arc<AtomicBool>,
    pub start_time: Instant,
    pub max_time: Option<Duration>,
    pub nodes: u64,
    pub aborted: bool,
}

impl SearchContext {
    pub fn new(stop_signal: Arc<AtomicBool>, max_time: Option<Duration>) -> Self {
        Self {
            stop_signal,
            start_time: Instant::now(),
            max_time,
            nodes: 0,
            aborted: false,
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

    if depth_left == 0 {
        return quiesce(board, alpha, beta, ctx);
    }

    let mut best_value = NEG_INF;
    let moves = crate::move_gen::gen_all_moves(board);

    for mv in moves {
        board.make_move(mv);
        let score = -alpha_beta_search(board, -beta, -alpha, depth_left - 1, ctx);
        board.unmake_move(mv);

        if ctx.aborted {
            return 0;
        }

        if score > best_value {
            best_value = score;
            if score > alpha {
                alpha = score;
            }
        }
        if score >= beta {
            return score;
        }
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
) -> Option<Move> {
    let root_moves = crate::move_gen::gen_all_moves(&board);
    if root_moves.is_empty() {
        send_line(&output, "bestmove 0000");
        return None;
    }

    let mut ctx = SearchContext::new(stop_signal, limits.time_allocated);
    let max_depth = limits.max_depth.unwrap_or(64);

    let mut last_completed_best_move: Option<Move> = None;

    for depth in 1..=max_depth {
        if ctx.aborted {
            break;
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

        let elapsed = ctx.start_time.elapsed();
        let time_ms = elapsed.as_millis() as u64;
        let nps = (ctx.nodes * 1000)
            .checked_div(time_ms)
            .unwrap_or(ctx.nodes * 1000);

        // TODO: Full PV extraction via Transposition Table
        // TODO: score mate <x> once checkmate distance detection is added
        let pv_str = best_move_this_depth
            .map(|m| m.to_string())
            .unwrap_or_default();

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
        let best_move = search_position(board, limits, stop_signal, Some(output_buf.clone()));
        assert!(best_move.is_some());
        let output = String::from_utf8(output_buf.lock().unwrap().clone()).unwrap();
        assert!(output.contains("info depth 1"));
        assert!(output.contains("info depth 2"));
        assert!(output.contains("bestmove "));
    }
}
