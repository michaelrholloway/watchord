//! The `watchord` binary. For now it prints its version and exits; the terminal
//! skin arrives with a later ticket.

fn main() {
    println!("watchord {}", env!("CARGO_PKG_VERSION"));
}
