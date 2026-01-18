//! NST Node
//!
//! A substrate-based blockchain node for Non Speculative Tokens (NST).
//! NST is a burn-only UBI cryptocurrency where tokens cannot be transferred,
//! only burned to signal value to recipients.

mod chain_spec;
mod cli;
mod command;
mod rpc;
mod service;

fn main() -> sc_cli::Result<()> {
    command::run()
}
