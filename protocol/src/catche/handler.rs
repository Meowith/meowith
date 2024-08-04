use async_trait::async_trait;
use std::any::Any;
use std::fmt::Debug;

#[async_trait]
pub trait CatcheHandler: Any + Send + Debug {
    async fn handle_invalidate(&mut self, cache_id: u32, cache_key: String);

    #[allow(unused)]
    fn as_any(&self) -> &dyn Any;
}
