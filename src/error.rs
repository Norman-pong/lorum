/// Unified error type for the lorum crate.
///
/// All public APIs return [`Result<T, LorumError>`] so callers receive
/// structured, human-readable errors without panicking.
#[derive(thiserror::Error, Debug)]
pub enum LorumError {
    /// Configuration file was not found at the given path.
    #[error("configuration not found: {path}")]
    ConfigNotFound {
        /// Path to the missing configuration file.
        path: std::path::PathBuf,
    },

    /// Configuration file could not be parsed.
    #[error("failed to parse {format} config at {path}")]
    ConfigParse {
        /// Format of the configuration file (e.g. "yaml", "toml").
        format: String,
        /// Path to the malformed configuration file.
        path: std::path::PathBuf,
        /// The underlying parse error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Configuration file could not be written.
    #[error("failed to write config at {path}")]
    ConfigWrite {
        /// Path to the configuration file that could not be written.
        path: std::path::PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Configuration file could not be serialized.
    #[error("failed to serialize config for {path}")]
    ConfigSerialize {
        /// Path to the configuration file that could not be serialized.
        path: std::path::PathBuf,
        /// The underlying serialization error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Requested adapter name is not recognised.
    #[error("adapter not found: {name}")]
    AdapterNotFound {
        /// Name of the adapter that was requested.
        name: String,
    },

    /// An I/O error occurred.
    #[error("{source}")]
    Io {
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// A catch-all variant for miscellaneous errors.
    #[error("{message}")]
    Other {
        /// Human-readable description of the error.
        message: String,
    },
}

impl From<std::io::Error> for LorumError {
    fn from(source: std::io::Error) -> Self {
        Self::Io { source }
    }
}
