use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub type DbPool = PgPool;

pub async fn get_db_pool() -> DbPool {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("Failed to create database pool")
}

pub async fn init_db(pool: &DbPool) -> Result<(), sqlx::Error> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}
