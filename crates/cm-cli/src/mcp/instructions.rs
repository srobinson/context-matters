//! Server instructions advertised through the MCP initialize response.

#[path = "generated_instructions.rs"]
mod generated_instructions;

pub(super) use generated_instructions::SERVER_INSTRUCTIONS;
