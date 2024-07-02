pub trait RequestContext {
    fn client(
        &self,
    ) -> impl std::future::Future<Output = tokio::sync::RwLockReadGuard<'_, reqwest::Client>>;

    fn update_client(&mut self);
}
