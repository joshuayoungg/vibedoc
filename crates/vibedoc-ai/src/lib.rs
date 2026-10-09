//! VibeDoc's assistant: turns requests like "put this list in a doc" or
//! "draw a picture here" into typed edits on the document.
//!
//! Models are pluggable. Chat: Claude or any OpenAI-compatible service.
//! Images: Gemini or OpenAI. See [`config`].

pub mod agent;
mod anthropic;
pub mod config;
mod images;
pub mod models;
mod openai;
pub mod text;
pub mod tools;

pub use agent::{spawn, AgentEvent, Turn};
pub use config::Config;
