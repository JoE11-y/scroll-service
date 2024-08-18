#![doc = include_str!("../Readme.md")]
#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::nursery)]
#![allow(clippy::multiple_crate_versions, clippy::too_many_arguments)]


pub mod app;
mod server;
mod utils;
mod contracts;
mod ethereum;
pub mod config;
mod processor;