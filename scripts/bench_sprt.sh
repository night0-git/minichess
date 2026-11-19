#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

TOTAL_CPUS="$(nproc 2>/dev/null || echo 4)"
if [[ "$TOTAL_CPUS" -le 2 ]]; then
    DEFAULT_CONCURRENCY=1
elif [[ "$TOTAL_CPUS" -le 4 ]]; then
    DEFAULT_CONCURRENCY=$((TOTAL_CPUS - 1))
elif [[ "$TOTAL_CPUS" -le 8 ]]; then
    DEFAULT_CONCURRENCY=$((TOTAL_CPUS - 2))
else
    RESERVED=$((TOTAL_CPUS / 4))
    [[ "$RESERVED" -lt 2 ]] && RESERVED=2
    DEFAULT_CONCURRENCY=$((TOTAL_CPUS - RESERVED))
fi

TC="10+0.1"
CONCURRENCY="$DEFAULT_CONCURRENCY"
ROUNDS="1000"
ELO0="0"
ELO1="5"
USE_SPRT=true
BASE_REF=""
BASE_BIN=""
BOOK=""
FASTCHESS_BIN="${FASTCHESS:-}"
OUT_DIR="$ROOT_DIR/benchmarks"
AUTO_CONFIRM=false

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Run an incremental benchmark match between Candidate (current working tree)
and Baseline using fastchess.

Options:
  -b, --book <PATH>         Path to opening book file (.pgn or .epd).
                            Auto-detected from 'books/' directory if omitted.
  -r, --base-ref <REF>      Git ref for baseline (e.g. main, HEAD~1, v0.1.0).
                            Defaults to 'main' (or 'HEAD~1' if currently on main).
  -B, --base-bin <PATH>     Path to an already-built baseline binary (skips git build).
  -t, --tc <TC>             Time control per move, e.g. 10+0.1, 5+0.05 (default: 10+0.1).
  -c, --concurrency <NUM>   Number of parallel games (default: $DEFAULT_CONCURRENCY).
  -n, --rounds <NUM>        Maximum number of game pairs to play (default: 1000).
      --elo0 <VAL>          SPRT lower bound (default: 0).
      --elo1 <VAL>          SPRT upper bound (default: 5).
      --fixed               Run fixed rounds without SPRT.
  -o, --out-dir <DIR>       Directory to save timestamped .log and .pgn results
                            (default: benchmarks/).
  -y, --yes                 Skip confirmation prompt and start immediately.
      --fastchess <PATH>    Path to fastchess binary.
  -h, --help                Show this help message.

Examples:
  $(basename "$0")
  $(basename "$0") -b books/UHO_2024_+085_+094/UHO_2024_8mvs_+085_+094.epd
  $(basename "$0") -r HEAD~1 -t 8+0.08 -c 12
  $(basename "$0") -B ./target/benchmark/minichess_base
EOF
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        -b|--book)
            BOOK="$2"
            shift 2
            ;;
        -r|--base-ref)
            BASE_REF="$2"
            shift 2
            ;;
        -B|--base-bin)
            BASE_BIN="$2"
            shift 2
            ;;
        -t|--tc)
            TC="$2"
            shift 2
            ;;
        -c|--concurrency)
            CONCURRENCY="$2"
            shift 2
            ;;
        -n|--rounds)
            ROUNDS="$2"
            shift 2
            ;;
        --elo0)
            ELO0="$2"
            shift 2
            ;;
        --elo1)
            ELO1="$2"
            shift 2
            ;;
        --fixed)
            USE_SPRT=false
            shift
            ;;
        -o|--out-dir)
            OUT_DIR="$2"
            shift 2
            ;;
        -y|--yes)
            AUTO_CONFIRM=true
            shift
            ;;
        --fastchess)
            FASTCHESS_BIN="$2"
            shift 2
            ;;
        -h|--help)
            usage
            ;;
        *)
            echo "Error: Unknown option: $1" >&2
            usage
            ;;
    esac
done

cd "$ROOT_DIR"

# Locate fastchess binary
if [[ -z "$FASTCHESS_BIN" ]]; then
    if command -v fastchess >/dev/null 2>&1; then
        FASTCHESS_BIN="fastchess"
    elif [[ -x "$HOME/.cargo/bin/fastchess" ]]; then
        FASTCHESS_BIN="$HOME/.cargo/bin/fastchess"
    elif [[ -x "$HOME/fastchess/fastchess" ]]; then
        FASTCHESS_BIN="$HOME/fastchess/fastchess"
    elif [[ -x "/usr/local/bin/fastchess" ]]; then
        FASTCHESS_BIN="/usr/local/bin/fastchess"
    else
        echo "Error: fastchess binary not found in PATH." >&2
        echo "Please specify with --fastchess <PATH> or set FASTCHESS env variable." >&2
        exit 1
    fi
fi

# Locate opening book
if [[ -z "$BOOK" ]]; then
    mapfile -t FOUND_BOOKS < <(find "$ROOT_DIR/books" \( -name "*.pgn" -o -name "*.epd" \) 2>/dev/null | sort)
    
    if [[ ${#FOUND_BOOKS[@]} -eq 0 ]]; then
        echo "Error: No opening book specified and none found in 'books/'." >&2
        echo "Please place an opening book in 'books/' or pass --book <PATH>." >&2
        exit 1
    elif [[ ${#FOUND_BOOKS[@]} -eq 1 ]]; then
        BOOK="${FOUND_BOOKS[0]}"
    else
        for b in "${FOUND_BOOKS[@]}"; do
            if [[ "$b" =~ (merged|combined) ]]; then
                BOOK="$b"
                break
            fi
        done

        if [[ -z "$BOOK" ]]; then
            for b in "${FOUND_BOOKS[@]}"; do
                if [[ "$b" =~ 8mvs_.*\.epd$ && ! "$b" =~ big ]]; then
                    BOOK="$b"
                    break
                fi
            done
        fi

        if [[ -z "$BOOK" ]]; then
            BOOK="${FOUND_BOOKS[0]}"
        fi
    fi
fi

if [[ ! -f "$BOOK" ]]; then
    echo "Error: Opening book not found: $BOOK" >&2
    exit 1
fi

BOOK_EXT="${BOOK##*.}"
if [[ "$BOOK_EXT" == "pgn" ]]; then
    BOOK_FORMAT="pgn"
elif [[ "$BOOK_EXT" == "epd" ]]; then
    BOOK_FORMAT="epd"
else
    echo "Error: Unrecognized book extension '$BOOK_EXT', defaulting to epd" >&2
    exit 1
fi

BOOK_REL="${BOOK#$ROOT_DIR/}"
BOOK_LINES="$(wc -l < "$BOOK" 2>/dev/null || echo "N/A")"

# Resolve candidate details
CAND_BRANCH="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "detached")"
CAND_HASH="$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")"
CAND_MSG="$(git log -1 --pretty=format:"%s" 2>/dev/null || echo "No commit history")"

DIRTY_DIFF="$(git status --porcelain 2>/dev/null || true)"
if [[ -n "$DIRTY_DIFF" ]]; then
    CAND_STATUS="DIRTY (working tree has uncommitted changes)"
else
    CAND_STATUS="CLEAN (working tree matches HEAD)"
fi

# Resolve baseline details
if [[ -n "$BASE_BIN" ]]; then
    BASE_TAG="external"
    BASE_INFO="Pre-compiled binary: $BASE_BIN"
    BASE_DETAIL="N/A (external binary provided)"
else
    if [[ -z "$BASE_REF" ]]; then
        if [[ "$CAND_BRANCH" == "main" || "$CAND_BRANCH" == "master" ]]; then
            BASE_REF="HEAD~1"
        else
            BASE_REF="main"
        fi
    fi

    if ! git rev-parse --verify "$BASE_REF" >/dev/null 2>&1; then
        if git rev-parse --verify "origin/$BASE_REF" >/dev/null 2>&1; then
            BASE_REF="origin/$BASE_REF"
        else
            echo "Error: Git reference for baseline '$BASE_REF' could not be resolved." >&2
            exit 1
        fi
    fi

    BASE_HASH="$(git rev-parse --short "$BASE_REF")"
    BASE_TAG="$BASE_HASH"
    BASE_MSG="$(git log -1 --pretty=format:"%s" "$BASE_REF")"
    BASE_INFO="$BASE_REF ($BASE_HASH)"
    BASE_DETAIL="$BASE_MSG"
fi

# Prepare unique incremental output filenames
RUN_BASE="$(date -u +%Y%m%d_%H%M%S)_${CAND_HASH}_vs_${BASE_TAG}"
RUN_ID="$RUN_BASE"
SUFFIX=2
while [[ -e "$OUT_DIR/${RUN_ID}.pgn" || -e "$OUT_DIR/${RUN_ID}.log" ]]; do
    RUN_ID="${RUN_BASE}_${SUFFIX}"
    SUFFIX=$((SUFFIX + 1))
done
PGN_OUT="$OUT_DIR/${RUN_ID}.pgn"
LOG_OUT="$OUT_DIR/${RUN_ID}.log"
OUT_REL="${OUT_DIR#$ROOT_DIR/}"

print_summary() {
    echo ""
    echo "================================================================================"
    echo " [Candidate]"
    echo "   Ref        : $CAND_BRANCH ($CAND_HASH)"
    echo "   Commit     : $CAND_MSG"
    echo "   Status     : $CAND_STATUS"
    echo ""
    echo " [Baseline]"
    echo "   Ref        : $BASE_INFO"
    echo "   Commit     : $BASE_DETAIL"
    echo ""
    echo " [Match Settings]"
    echo "   Opening Book: $BOOK_REL ($BOOK_LINES positions, format: $BOOK_FORMAT)"
    echo "   Time Control: $TC"
    echo "   CPU Threads : $CONCURRENCY worker threads ($TOTAL_CPUS total)"
    if [[ "$USE_SPRT" == "true" ]]; then
        echo "   Evaluation  : SPRT [elo0=$ELO0, elo1=$ELO1, alpha=0.05, beta=0.05] (max $ROUNDS rounds)"
    else
        echo "   Evaluation  : Fixed match ($ROUNDS rounds = $((ROUNDS * 2)) games)"
    fi
    echo "   Fastchess   : $FASTCHESS_BIN"
    echo "   Output      : $OUT_REL/${RUN_ID}.{log,pgn}"
    echo "================================================================================"
    echo ""
}

print_summary

# Confirm before running
if [[ "$AUTO_CONFIRM" != "true" ]]; then
    if [[ -t 0 ]]; then
        read -r -p "Proceed with benchmark? [Y/n] " USER_DECISION
    else
        read -r USER_DECISION || USER_DECISION="y"
    fi
    if [[ "$USER_DECISION" =~ ^[nN] ]]; then
        echo "==> Benchmark aborted by user."
        exit 0
    fi
fi

# Build binaries
BENCH_DIR="$ROOT_DIR/target/benchmark"
mkdir -p "$BENCH_DIR"
CANDIDATE_BIN="$BENCH_DIR/minichess_candidate"

echo ""
echo "==> [1/2] Building Candidate (current tree)..."
cargo build --release
cp "$ROOT_DIR/target/release/minichess" "$CANDIDATE_BIN"

if [[ -z "$BASE_BIN" ]]; then
    BASE_BIN="$BENCH_DIR/minichess_base"
    echo "==> [2/2] Building Baseline from git ref '$BASE_REF'..."
    WORKTREE_DIR="$(mktemp -d /tmp/minichess-base.XXXXXX)"

    cleanup() {
        if [[ -d "$WORKTREE_DIR" ]]; then
            git worktree remove --force "$WORKTREE_DIR" 2>/dev/null || rm -rf "$WORKTREE_DIR"
        fi
    }
    trap cleanup EXIT INT TERM

    git worktree add --detach "$WORKTREE_DIR" "$BASE_REF"
    (
        cd "$WORKTREE_DIR"
        cargo build --release --target-dir "$WORKTREE_DIR/target"
        cp "$WORKTREE_DIR/target/release/minichess" "$BASE_BIN"
    )
    cleanup
    trap - EXIT INT TERM
fi

# Run fastchess
EXTRA_ARGS=()
if [[ "$USE_SPRT" == "true" ]]; then
    EXTRA_ARGS+=("-sprt" "elo0=$ELO0" "elo1=$ELO1" "alpha=0.05" "beta=0.05")
fi

mkdir -p "$OUT_DIR"
print_summary > "$LOG_OUT"

echo ""
echo "==> Launching fastchess match..."
"$FASTCHESS_BIN" \
  -engine cmd="$CANDIDATE_BIN" name="Candidate" \
  -engine cmd="$BASE_BIN" name="Baseline" \
  -each tc="$TC" \
  -rounds "$ROUNDS" \
  -repeat \
  -concurrency "$CONCURRENCY" \
  -openings file="$BOOK" format="$BOOK_FORMAT" order=random \
  -draw movenumber=40 movecount=8 score=10 \
  -resign movecount=4 score=800 \
  -pgnout file="$PGN_OUT" \
  "${EXTRA_ARGS[@]}" | tee -a "$LOG_OUT"
