pub trait RequestContext {
    fn client(
        &self,
    ) -> impl std::future::Future<Output = async_rwlock::RwLockReadGuard<'_, reqwest::Client>>;

    fn update_client(&mut self);
}
