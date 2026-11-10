use crate::engine::evaluate;
use crate::position::board::Board;

// i32::MIN has an absolute value 1 greater than i32::MAX, negating it will cause an overflow.
const NEG_INF: i32 = i32::MIN + 1;

/// Find the minimax score of the current board state.
pub fn alpha_beta(board: &mut Board, mut alpha: i32, beta: i32, depth_left: i32) -> i32 {
    if depth_left == 0 {
        return quiesce(board, alpha, beta);
    }

    let mut best_value = NEG_INF;
    let moves = crate::move_gen::gen_all_moves(board);

    for mv in moves {
        board.make_move(mv);
        let score = -alpha_beta(board, -beta, -alpha, depth_left - 1);
        board.unmake_move(mv);

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
fn quiesce(board: &mut Board, mut alpha: i32, beta: i32) -> i32 {
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
        let score = -quiesce(board, -beta, -alpha);
        board.unmake_move(mv);

        if score >= beta {
            return beta;
        }
        if score > alpha {
            alpha = score;
        }
    }

    alpha
}
