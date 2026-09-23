//! Ask on the terminal for marked args the user left out.
//!
//! A mark that can never become a single prompt fails to compile:
//!
//! ```compile_fail
//! use clap::Parser;
//! use clap_couture::Couture;
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[arg(long)]
//!     #[couture(prompt)]
//!     tags: Vec<String>,
//! }
//! ```
//!
//! ```compile_fail
//! use clap::{Args, Parser};
//! use clap_couture::Couture;
//!
//! #[derive(Parser, Couture)]
//! struct Cli {
//!     #[command(flatten)]
//!     #[couture(prompt)]
//!     common: Common,
//! }
//!
//! #[derive(Args, Couture)]
//! struct Common {
//!     #[arg(long)]
//!     name: String,
//! }
//! ```

mod tree;

#[doc(hidden)]
pub use tree::{Mark, PromptChild, PromptNode, PromptSpec};
