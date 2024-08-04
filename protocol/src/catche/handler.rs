use std::any::Any;
use std::fmt::Debug;
use async_trait::async_trait;

#[async_trait]
pub trait CatcheHandler : Any + Send + Debug {
    async fn handle_invalidate(&mut self);

    fn as_any(&self) -> &dyn Any;
}
