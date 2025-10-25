mod http;
mod models;
mod constants;

use axum::{
    Router,
    http::{
        header::{
            ACCEPT,
            AUTHORIZATION,
            CONTENT_TYPE
        },
        Method,
    },
};
use tower_http::{
    trace::TraceLayer,
    cors::{
        CorsLayer,
        Any,
    },
};
use std::sync::{Mutex, Arc};
use sqlx::{
    postgres::PgPoolOptions,
    migrate::{
        Migrator,
        MigrateDatabase
    },
};
use tracing_subscriber::{
    EnvFilter, layer::SubscriberExt,
    util::SubscriberInitExt
};
use tower_http::services::{ServeDir, ServeFile};
use tracing::{
    info,
    debug,
};
use maxminddb::Reader;
use std::{
    str::FromStr,
    env::var,
    path::Path,
};
use http::{
    health_router,
    user_router,
    shuul_router,
    util_router,
    api_user_router,
    record_router,
    rule_router,
    ignored_router,
};
use models::{
    Rule,
    Ignored,
};
use dotenv::dotenv;
use models::{
    AppState,
    Error,
};

const STATIC_DIR: &str = "static";

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();
    let log_level = var("RUST_LOG").unwrap_or("debug".to_string());
    tracing_subscriber::registry()
        .with(EnvFilter::from_str(&log_level).unwrap())
        .with(tracing_subscriber::fmt::layer())
        .init();
    info!("Log level: {log_level}");

    let db_url = var("DATABASE_URL").expect("DB_URL environment mandatory");
    info!("DB url: {}", db_url);
    let port = var("PORT").unwrap_or("3000".to_string());
    info!("Port: {}", port);
    let maxmind_db_path = var("MAXMIND_DB_PATH").unwrap_or("geo/GeoLite2-City.mmdb".to_string());
    info!("Maxmin DB Path: {}", maxmind_db_path);
    let secret = var("SECRET").unwrap_or("esto-es-un-secreto".to_string());
    debug!("Secret: {}", secret);
    let cache_enabled = var("CACHE_ENABLED").unwrap_or("false".to_string()).parse::<bool>().unwrap_or(false);
    debug!("cache_enabled: {}", cache_enabled);
    let cache_size = var("CACHE_SIZE").unwrap_or("10".to_string()).parse::<usize>().unwrap_or(10);
    debug!("cache_size: {}", cache_size);


    if !sqlx::Postgres::database_exists(&db_url).await.unwrap(){
        sqlx::Postgres::create_database(&db_url).await.unwrap();
    }

    let migrations = if var("RUST_ENV") == Ok("production".to_string()){
        std::env::current_exe().unwrap().parent().unwrap().join("migrations")
    }else{
        let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        Path::new(&crate_dir).join("migrations")
    };
    info!("{}", &migrations.display());

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await
        .expect("Pool failed");

    Migrator::new(migrations)
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();

    let cors = CorsLayer::new()
        //.allow_origin(url.parse::<HeaderValue>().unwrap())
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH,
            Method::DELETE])
        //.allow_credentials(true)
        .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE]);

    let rules = Mutex::new(Rule::read_all_active(&pool).await.unwrap_or_default());
    let ignored = Mutex::new(Ignored::read_all_active(&pool).await.unwrap_or_default());
    let cache = Mutex::new(Vec::new());
    let api_routes = Router::new()
        .nest("/shuul", shuul_router())
        .nest("/util", util_router())
        .nest("/health", health_router())
        .nest("/auth", user_router())
        .nest("/users", api_user_router())
        .nest("/records", record_router())
        .nest("/ignored", ignored_router())
        .nest("/rules", rule_router())
        .with_state(Arc::new(AppState {
            pool,
            secret,
            maxmind_db: Reader::open_readfile(maxmind_db_path).unwrap(),
            static_dir: STATIC_DIR.to_string(),
            rules,
            ignored,
            cache,
            cache_enabled,
            cache_size,
    }));

    let app = Router::new()
        .nest("/api/v1", api_routes)
        .fallback_service(ServeDir::new(STATIC_DIR)
            .fallback(ServeFile::new("static/index.html")))
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    tracing::info!("🚀 Server started successfully");
    axum::serve(listener, app).await?;

    Ok(())
}
