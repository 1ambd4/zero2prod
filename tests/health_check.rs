use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::net::TcpListener;
use uuid::Uuid;
use zero2prod::config::{get_config, DatabaseSettings};

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
}

async fn spawn_app() -> TestApp {
    let mut config = get_config().expect("Failed to load config.");
    config.database.database_name = Uuid::new_v4().to_string();
    let db_pool = configure_database(&config.database).await;

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port.");
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

    let server = zero2prod::startup::run(listener, db_pool.clone()).expect("Failed to spawn app.");
    let _ = tokio::spawn(server);

    TestApp { address, db_pool }
}

pub async fn configure_database(config: &DatabaseSettings) -> PgPool {
    let mut connection = PgConnection::connect(&config.connection_string_without_db())
        .await
        .expect("Failed to connect to Postgres");

    // Create database
    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database");

    // Migrate database
    let connection_pool = PgPool::connect(&config.connection_string())
        .await
        .expect("Failed to connect to Postgres");
    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate the database");

    connection_pool
}

#[actix_rt::test]
async fn health_check_works() {
    let app = spawn_app().await;

    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/health_check", app.address))
        .send()
        .await
        .expect("Failed to send request.");

    assert!(resp.status().is_success());
    assert_eq!(Some(0), resp.content_length());
}

#[actix_rt::test]
async fn subscribe_returns_200_for_valid_form_data() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    let body = "name=lisi&email=lisi@mail.com";

    let resp = client
        .post(format!("{}/subscriptions", app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to send request.");

    assert_eq!(200, resp.status().as_u16());

    let config = get_config().expect("Failed to load config.");
    let connection_string = config.database.connection_string();

    let mut connection = PgConnection::connect(&connection_string)
        .await
        .expect("Failed to connect to psql");

    let saved = sqlx::query!("SELECT email, name FROM subscriptions",)
        .fetch_one(&mut connection)
        .await
        .expect("Failed to fetch saved subscription.");

    assert_eq!(saved.name, "lisi");
    assert_eq!(saved.email, "lisi@mail.com");
}

#[actix_rt::test]
async fn subscribe_returns_400_for_invalid_form_data() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    let test_cases = vec![
        ("name=lisi", "missing email"),
        ("email=lisi@mail.com", "missing name"),
        ("", "missing both name and email"),
    ];

    for (invalid_body, error_message) in test_cases {
        let resp = client
            .post(format!("{}/subscriptions", app.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to send reqwest.");

        assert_eq!(
            400,
            resp.status().as_u16(),
            "The API did not fail with 400 Bad Response: {}",
            error_message
        );
    }
}
