use crate::position::board::Board;

const INF: i32 = i32::MAX;
const NEG_INF: i32 = i32::MIN;

fn evaluate(_board: &Board) -> i32 {
    0
}

pub fn alpha_beta_max(board: &mut Board, mut alpha: i32, beta: i32, depth_left: i32) -> i32 {
    if depth_left == 0 {
        return quiesce_max(board, alpha, beta);
    }

    let mut best_value = NEG_INF;
    let moves = crate::move_gen::gen_all_moves(board);

    for mv in moves {
        board.make_move(mv);
        let score = alpha_beta_min(board, alpha, beta, depth_left - 1);
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

pub fn alpha_beta_min(board: &mut Board, alpha: i32, mut beta: i32, depth_left: i32) -> i32 {
    if depth_left == 0 {
        return quiesce_min(board, alpha, beta);
    }

    let mut best_value = INF;
    let moves = crate::move_gen::gen_all_moves(board);

    for mv in moves {
        board.make_move(mv);
        let score = alpha_beta_max(board, alpha, beta, depth_left - 1);
        board.unmake_move(mv);

        if score < best_value {
            best_value = score;
            if score < beta {
                beta = score;
            }
        }
        if score <= alpha {
            return score;
        }
    }

    best_value
}

/// Quiescence search for the maximizing player
fn quiesce_max(board: &mut Board, mut alpha: i32, beta: i32) -> i32 {
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
        let score = quiesce_min(board, alpha, beta);
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

/// Quiescence search for the minimizing player
fn quiesce_min(board: &mut Board, alpha: i32, mut beta: i32) -> i32 {
    // The static evaluation establishes the baseline "stand pat" score
    let stand_pat = -evaluate(board); // Perspective adjustment for min player

    if stand_pat <= alpha {
        return alpha;
    }
    if stand_pat < beta {
        beta = stand_pat;
    }

    // Generate ONLY capturing moves to resolve tactical tension
    let captures = crate::move_gen::gen_qsearch_moves(board);

    for mv in captures {
        board.make_move(mv);
        let score = quiesce_max(board, alpha, beta);
        board.unmake_move(mv);

        if score <= alpha {
            return alpha;
        }
        if score < beta {
            beta = score;
        }
    }

    beta
}
