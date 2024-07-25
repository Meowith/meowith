use async_trait::async_trait;

#[async_trait]
pub trait CatcheHandler : Send {
    async fn handle_invalidate(&mut self);
}