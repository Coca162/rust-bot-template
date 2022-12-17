use sqlx::PgPool;

pub mod help;
pub mod ping;

pub type Context<'a> = poise::Context<'a, Data, Error>;
pub type Error = Box<dyn std::error::Error + Send + Sync>;

// Data shared across commands and events
pub struct Data {
    pub db: PgPool
}