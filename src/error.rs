use thiserror::Error;

#[derive(Debug, Error)]
pub enum WifimonError {
    #[error("No wireless interfaces found")]
    NoInterfaces,

    #[error("Interface '{0}' not found or is not a wireless device")]
    InterfaceNotFound(String),

    #[error("Scan failed on '{iface}': {reason}")]
    ScanFailed { iface: String, reason: String },

    #[error("GNTP notification error: {0}")]
    NotificationFailed(String),

    #[error("State file I/O error: {0}")]
    StateIo(#[from] std::io::Error),

    #[error("State file JSON error: {0}")]
    StateJson(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Platform error: {0}")]
    Platform(String),
}
