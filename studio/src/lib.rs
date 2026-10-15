//! The services of NEXUL Studio, independent of any interface: file operations on the
//! workspace, running programs, and the bridge to the debug adapter. The current web
//! interface (the `NEXUL-Studio` server) uses them, and a native interface can call them
//! directly.

pub mod compiler;
pub mod debug;
pub mod runner;
pub mod search;
pub mod watch;
pub mod workspace;
