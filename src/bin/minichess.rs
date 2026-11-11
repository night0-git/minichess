fn main() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    minichess::uci::run_uci(stdin.lock(), stdout);
}
