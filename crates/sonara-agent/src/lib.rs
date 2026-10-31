//! The agent interface: the tool registry and its schemas, handlers, permissions and confirmations,
//! the audit log, project-state views, the IPC server and client, and the MCP server.
//!
//! In place so far: the tool contract ([`contract`]), the MCP view of it ([`mcp`]), the file
//! sandbox and confirmations ([`permissions`]), the audit log ([`audit`]), the conversion of
//! position, length, pitch, articulation and part arguments ([`conversion`]), the dispatcher
//! ([`registry`]) with the handler context ([`context`]) and the service facade ([`service`]),
//! the state views ([`state`]), the first handlers ([`handlers`]), and the IPC server and client
//! ([`ipc`]).
#![forbid(unsafe_code)]

pub mod audit;
pub mod context;
pub mod contract;
pub mod conversion;
pub mod handlers;
pub mod ipc;
pub mod mcp;
pub mod orchestration;
pub mod permissions;
mod pystr;
pub mod registry;
pub mod service;
pub mod state;
mod vocabulary;
