use surrealdb::{Surreal, engine::any::Any};

#[derive(Clone)]
pub struct AppState {
    pub db: Surreal<Any>,
}
