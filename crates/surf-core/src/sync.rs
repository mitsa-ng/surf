#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub platform: String,
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("transport unavailable")]
    Unavailable,
    #[error("payload rejected: {0}")]
    Rejected(String),
}

/// Carries encrypted bookmark/history payloads between devices. The
/// transport is chosen by the shell (LAN, relay, or file export); the
/// core only defines the envelope.
pub trait SyncTransport {
    fn announce(&self, device: &DeviceInfo) -> Result<(), SyncError>;
    fn push(&self, payload: &[u8]) -> Result<(), SyncError>;
    fn poll(&self) -> Result<Vec<u8>, SyncError>;
}

pub struct NoopTransport;

impl SyncTransport for NoopTransport {
    fn announce(&self, _device: &DeviceInfo) -> Result<(), SyncError> {
        Err(SyncError::Unavailable)
    }
    fn push(&self, _payload: &[u8]) -> Result<(), SyncError> {
        Err(SyncError::Unavailable)
    }
    fn poll(&self) -> Result<Vec<u8>, SyncError> {
        Err(SyncError::Unavailable)
    }
}
