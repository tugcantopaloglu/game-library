//! Read-only smoke test: cargo run -p game-library-native --example broadcast-probe
#![allow(dead_code)]

#[path = "../src/broadcast.rs"]
mod broadcast;

fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&broadcast::inspect()).unwrap()
    );
}
