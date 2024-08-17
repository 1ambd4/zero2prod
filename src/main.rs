use std::net::TcpListener;

use sqlx::PgPool;
use zero2prod::config::get_config;
use zero2prod::startup::run;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = get_config().expect("Failed to load config.");
    let address = "127.0.0.1:8000".to_string();
    let listener = TcpListener::bind(address)?;
    let connection_pool = PgPool::connect(&config.database.connection_string())
        .await
        .expect("Failed to connect to Postgres");
    run(listener, connection_pool)?.await
}
