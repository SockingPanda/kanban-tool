mod affected;
mod check;
mod cli;
mod document;
mod git;
mod package;
mod process;
mod repository;

fn main() {
    if let Err(error) = cli::run() {
        eprintln!("xtask 失败: {error}");
        std::process::exit(1);
    }
}
