//! # shuul — Backend
//!
//! Punto de entrada de la aplicación. Configura el servidor HTTP,
//! la conexión a PostgreSQL, las migraciones, el logging, CORS,
//! y monta todas las rutas de la API.
//!
//! ## Flujo de inicio
//! 1. Carga variables de entorno (`.env`)
//! 2. Inicializa el subscriber de tracing
//! 3. Verifica/crea la base de datos
//! 4. Ejecuta migraciones SQLx
//! 5. Carga las reglas activas en memoria
//! 6. Carga las estadísticas desde la BD
//! 7. Arranca el servidor Axum en `0.0.0.0:3000`

mod constants;
mod http;
mod models;
mod templates;
use axum::{
    Router,
    http::{
        Method,
        header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    },
    middleware as axum_middleware,
};
use dotenv::dotenv;
use http::{
    auth_router, ban_router, health_router, rate_limit_profile_router, report_router, require_auth,
    rule_router, settings_router, shuul_router, stats_router, template_router, util_router,
};
use maxminddb::Reader;
use models::CacheRule;
use models::{
    AppState, BanManager, Error, JwtValidator, OidcMetadata, RateLimiter, Settings, StatsCollector,
};
use sqlx::{
    migrate::{MigrateDatabase, Migrator},
    sqlite::{SqlitePool, SqlitePoolOptions},
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::{env::var, str::FromStr};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{debug, info, warn};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

const STATIC_DIR: &str = "static";

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();
    // Nivel de log (por defecto "debug")
    let log_level = var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    let env_filter = EnvFilter::from_str(&log_level).unwrap_or_else(|e| {
        eprintln!("Invalid RUST_LOG value '{log_level}': {e}, falling back to 'info'");
        EnvFilter::new("info")
    });
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
    info!("Log level: {log_level}");

    let db_url = var("DATABASE_URL").expect("DATABASE_URL environment mandatory");
    debug!("DATABASE_URL url: {}", db_url);
    let port = var("PORT").unwrap_or("3000".to_string());
    info!("Port: {}", port);
    let maxmind_db_path = var("MAXMIND_DB_PATH").unwrap_or("geo/GeoLite2-City.mmdb".to_string());
    info!("Maxmin DB Path: {}", maxmind_db_path);
    let secret = var("SECRET").expect("SECRET environment variable is mandatory");
    debug!("Secret: {}", secret);

    if !sqlx::Sqlite::database_exists(&db_url).await.unwrap() {
        sqlx::Sqlite::create_database(&db_url).await.unwrap();
    }

    // Crear el pool de conexiones (propagar error con `?`)
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create DB pool: {}", e);
            Error::Other(format!("Failed to create DB pool: {e}"))
        })?;
    debug!("Created databae pool");

    // Ejecutar migraciones
    // Runtime: migrations at ./migrations/ (Docker: /app/migrations, dev: project root)
    const MIGRATIONS_DIR: &str = "migrations";
    let migrations_path = if var("RUST_ENV").as_deref() == Ok("production") {
        // En producción: relativo al ejecutable, no al working directory
        let exe_dir = std::env::current_exe()
            .map(|p| p.parent().unwrap().to_path_buf())
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        exe_dir.join(MIGRATIONS_DIR)
    } else if std::path::Path::new(MIGRATIONS_DIR).exists() {
        std::path::PathBuf::from(MIGRATIONS_DIR)
    } else {
        // Fallback for development via cargo run
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        manifest_dir.join(MIGRATIONS_DIR)
    };
    debug!("Migrations path: {:?}", migrations_path);
    Migrator::new(migrations_path)
        .await
        .map_err(|e| {
            tracing::error!("Failed to load migrations: {}", e);
            Error::Other(format!("Failed to load migrations: {e}"))
        })?
        .run(&pool)
        .await
        .map_err(|e| {
            tracing::error!("Migration run failed: {}", e);
            Error::Other(format!("Migration run failed: {e}"))
        })?;

    let cors = CorsLayer::new()
        //.allow_origin(url.parse::<HeaderValue>().unwrap())
        .allow_origin(Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
        ])
        //.allow_credentials(true)
        .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE]);

    let rules = Mutex::new(CacheRule::read_all_active(&pool).await.unwrap_or_default());
    let ban_manager = Mutex::new(
        BanManager::load_from_db(&pool)
            .await
            .map(|(bm, loaded)| {
                info!("Loaded {} active bans from database", loaded.len());
                bm
            })
            .unwrap_or_else(|e| {
                warn!("Failed to load bans from DB (starting fresh): {e}");
                BanManager::new(
                    3600,  // default_ban_duration (1h)
                    false, // bantime_increment (per-rule config)
                    vec![1, 2, 4, 8],
                    604800, // bantime_maxtime (1w)
                    30,     // ban_count_decay_days
                )
            }),
    );
    let rate_limiter: Mutex<HashMap<i32, RateLimiter>> = Mutex::new(HashMap::new());
    let settings = Mutex::new(Settings::load(&pool).await.unwrap_or_default());
    let stats = StatsCollector::load(&pool).await;

    // ── OIDC / SSO Configuration (REQUIRED) ──
    let oidc_issuer_url =
        var("OIDC_ISSUER_URL").expect("OIDC_ISSUER_URL environment variable is mandatory");
    let oidc_client_id =
        var("OIDC_CLIENT_ID").expect("OIDC_CLIENT_ID environment variable is mandatory");
    let _oidc_client_secret =
        var("OIDC_CLIENT_SECRET").expect("OIDC_CLIENT_SECRET environment variable is mandatory");
    let oidc_redirect_url = var("OIDC_REDIRECT_URL")
        .unwrap_or_else(|_| "http://localhost:3000/api/v1/auth/callback".to_string());

    info!("OIDC configured: issuer={oidc_issuer_url}, client_id={oidc_client_id}");

    // OIDC metadata y JWT validator se inicializan lazy en background task

    let app_state = Arc::new(AppState {
        pool,
        secret,
        maxmind_db: Reader::open_readfile(&maxmind_db_path)
            .map_err(|e| Error::Other(format!("Failed to open MaxMind DB: {e}")))?,
        static_dir: STATIC_DIR.to_string(),
        rules,
        stats,
        ban_manager,
        rate_limiter,
        settings,
        oidc_metadata: tokio::sync::RwLock::new(None),
        jwt_validator: tokio::sync::RwLock::new(None),
        oidc_states: tokio::sync::Mutex::new(HashMap::new()),
        oidc_client_id: Some(oidc_client_id.clone()),
        oidc_redirect_url: Some(oidc_redirect_url),
    });

    // Spawn OIDC lazy initialization background task
    let oidc_state = Arc::clone(&app_state);
    let oidc_issuer_url = oidc_issuer_url.clone();
    let oidc_client_id = oidc_client_id.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            let discovery_url = format!("{}/.well-known/openid-configuration", oidc_issuer_url);
            match reqwest::get(&discovery_url).await {
                Ok(resp) => match resp.json::<OidcMetadata>().await {
                    Ok(metadata) => {
                        let mut validator = JwtValidator::new(&metadata.issuer, &oidc_client_id);
                        match validator.fetch_jwks(&metadata.jwks_uri).await {
                            Ok(()) => {
                                info!(
                                    "OIDC initialized: issuer={}, auth_endpoint={}",
                                    metadata.issuer, metadata.authorization_endpoint
                                );
                                *oidc_state.oidc_metadata.write().await = Some(metadata);
                                *oidc_state.jwt_validator.write().await = Some(validator);
                                break; // Success — task done
                            },
                            Err(e) => {
                                warn!("Failed to fetch JWKS (will retry): {e}");
                            },
                        }
                    },
                    Err(e) => {
                        warn!("Failed to parse OIDC metadata (will retry): {e}");
                    },
                },
                Err(e) => {
                    warn!("Failed to fetch OIDC metadata (will retry): {e}");
                },
            }
        }
    });

    // Background task: cleanup expired bans every 60 seconds
    let cleanup_state = Arc::clone(&app_state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Ok(mut ban_manager) = cleanup_state.ban_manager.lock() {
                let before = ban_manager.active_count();
                ban_manager.cleanup_expired();
                let after = ban_manager.active_count();
                if before != after {
                    debug!("Ban cleanup: {} → {} active bans", before, after);
                }
            }
            if let Ok(mut rate_limiters) = cleanup_state.rate_limiter.lock() {
                rate_limiters.retain(|_, rl| {
                    rl.cleanup_expired();
                    !rl.is_empty()
                });
            }
        }
    });

    // Background task: persist stats every 30 minutes
    let stats_state = Arc::clone(&app_state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1800));
        loop {
            interval.tick().await;
            stats_state.stats.persist(&stats_state.pool).await;
        }
    });

    let api_routes = Router::new()
        .nest("/shuul", shuul_router())
        .nest("/util", util_router())
        .nest("/health", health_router())
        .nest("/auth", auth_router())
        .nest("/report", report_router())
        .with_state(app_state.clone());

    let protected_routes = Router::new()
        .nest("/stats", stats_router())
        .nest("/rules", rule_router())
        .nest("/bans", ban_router())
        .nest("/templates", template_router())
        .nest("/settings", settings_router())
        .nest("/rate-limit-profiles", rate_limit_profile_router())
        .route_layer(axum_middleware::from_fn_with_state(
            app_state.clone(),
            require_auth,
        ))
        .with_state(app_state);

    let app = Router::new()
        .nest("/api/v1", api_routes)
        .nest("/api/v1", protected_routes)
        .fallback_service(ServeDir::new(STATIC_DIR).fallback(ServeFile::new("static/index.html")))
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    tracing::info!("🚀 Server started successfully 🚀");
    axum::serve(listener, app).await?;

    Ok(())
}
