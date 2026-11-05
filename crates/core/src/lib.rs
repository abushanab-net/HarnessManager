//! hrm-core: every rule of HarnessManager. The CLI and the desktop app are thin layers over it.

pub mod ctx;
pub mod dash;
pub mod db;
pub mod harness;
pub mod import;
pub mod index;
pub mod items;
pub mod journal;
pub mod launch;
pub mod model;
pub mod pricing;
pub mod procs;
pub mod projects;
pub mod registry;
pub mod secrets;
pub mod service;
pub mod shared;
pub mod sync;
pub mod transcripts;
pub mod util;
pub mod watch;
pub mod apps;
pub mod backup;
pub mod log;
pub mod archive;
pub mod restore;

pub use ctx::Ctx;
pub use model::*;
pub use registry::{Kind, Profile, Registry};
