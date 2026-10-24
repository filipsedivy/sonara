//! The tool contract: every tool's name, title, description, safety class and input model in
//! one declarative table (`tools`, `inputs`), with the JSON Schemas (`schema`), the argument
//! validation (`validation`) and the typed argument structs (`args`) all derived from it
//! (CTR-01).
//!
//! The table ports the prototype's pydantic models (`sonara.agent.schemas.models`) and the
//! `@tool` decorators of `sonara.agent.tools.handlers`. The golden files
//! `contracts/tools.json` and `contracts/mcp-tools-list.json` are the oracle for the schemas,
//! and the validation errors recorded in the golden transcripts are the oracle for validation.

pub mod args;
mod coerce;
pub mod inputs;
pub mod model;
pub mod schema;
pub mod tools;
pub mod validation;

pub use args::{ToolArgs, parse};
pub use model::Model;
pub use schema::tool_contract;
pub use tools::{API_VERSION, Safety, TOOLS, ToolSpec, find};
pub use validation::{FieldError, validate};
