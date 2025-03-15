use async_trait::async_trait;

/// Handle allowing for pausing app operation in a degraded state,
/// such as mgpp being broken.
#[async_trait]
pub trait ApplicationPauseHandle: Send + Sync {
    /// Pause receiving client requests
    /// Does not interrupt currently ongoing requests
    async fn pause(&self);

    /// Resume receiving client requests
    async fn resume(&self);
}
