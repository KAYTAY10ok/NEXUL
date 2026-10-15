//! Language Server Protocol client for NEXUL Studio.
//!
//! No dependency, in particular none on GPUI: the client runs the server in a process,
//! reads it on a thread and hands over answers and events through channels, so that the
//! interface can wait for them on its own background tasks.
//!
//! - [`Client`]: start a server ([`ServerCommand`]), open and edit documents, ask for
//!   hover, completion, definition, references, rename and formatting, shut it down;
//! - [`registry`]: the languages, their servers and the SDKs, and their [`Detector`]
//!   (nothing is downloaded); a language without a server is "coloration seulement";
//! - [`types`]: the data of the protocol and the conversions an editor needs (positions
//!   in UTF-16 or UTF-8 to byte offsets, paths to `file://` URIs, applying edits);
//! - [`json`] and [`rpc`]: the JSON values and the `Content-Length` framing.
//!
//! NEXUL itself is not served here until `nxc lsp` exists: Studio keeps `nxc ide`.

pub mod client;
pub mod json;
pub mod registry;
pub mod rpc;
pub mod types;

pub use client::{codes, Client, Error, Event, Pending, ServerCapabilities, ServerCommand, StartOptions, SyncKind};
pub use json::Json;
pub use registry::{DetectedSdk, DetectedServer, Detection, Detector, Language, LanguageStatus};
pub use types::{
    apply_edits, offset_to_position, path_to_uri, position_to_offset, uri_to_path, CompletionItem, Completions,
    Diagnostic, FileEdit, FormattingOptions, Hover, Location, Position, PositionEncoding, Range, Severity, TextEdit,
    WorkspaceEdit,
};
