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
//! 6. Arranca el servidor Axum en `0.0.0.0:3000`

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
    auth_router, ban_router, health_router, rate_limit_profile_router, report_router,
    request_router, require_auth, rule_router, settings_router, shuul_router, template_router,
    util_router,
};
use maxminddb::Reader;
use models::CacheRule;
use models::{AppState, BanManager, Error, JwtValidator, OidcMetadata, RateLimiter, Settings};
use sqlx::{
    Row,
    migrate::{MigrateDatabase, Migrator},
    postgres::PgPoolOptions,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::{env::var, path::Path, str::FromStr};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{debug, error, info, warn};
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

    let db_url = var("DATABASE_URL").expect("DB_URL environment mandatory");
    debug!("DB url: {}", db_url);
    let port = var("PORT").unwrap_or("3000".to_string());
    info!("Port: {}", port);
    let maxmind_db_path = var("MAXMIND_DB_PATH").unwrap_or("geo/GeoLite2-City.mmdb".to_string());
    info!("Maxmin DB Path: {}", maxmind_db_path);
    let secret = var("SECRET").expect("SECRET environment variable is mandatory");
    debug!("Secret: {}", secret);
    let cache_enabled = var("CACHE_ENABLED")
        .unwrap_or("false".to_string())
        .parse::<bool>()
        .unwrap_or(false);
    debug!("cache_enabled: {}", cache_enabled);
    let cache_size = var("CACHE_SIZE")
        .unwrap_or("10".to_string())
        .parse::<usize>()
        .unwrap_or(10);
    debug!("cache_size: {}", cache_size);

    // Asegurarse de que la base de datos exista; propagar errores vía `?`
    if !sqlx::Postgres::database_exists(&db_url).await? {
        sqlx::Postgres::create_database(&db_url).await?;
    }

    // Ruta de migraciones (compatible con producción y desarrollo)
    let migrations = if var("RUST_ENV") == Ok("production".to_string()) {
        let exe_path = std::env::current_exe()
            .map_err(|e| Error::Other(format!("failed to get current exe: {e}")))?;
        let parent = exe_path
            .parent()
            .ok_or_else(|| Error::Other("executable has no parent".to_string()))?;
        parent.join("migrations")
    } else {
        let crate_dir = std::env::var("CARGO_MANIFEST_DIR").map_err(Error::from)?;
        Path::new(&crate_dir).join("migrations")
    };
    info!("Migrations path: {}", migrations.display());

    // Crear el pool de conexiones (propagar error con `?`)
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create DB pool: {}", e);
            Error::Other(format!("Failed to create DB pool: {e}"))
        })?;

    // Ejecutar migraciones
    Migrator::new(migrations)
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
    let cache = Mutex::new(Vec::new());
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
        cache,
        cache_enabled,
        cache_size,
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

    // Background task: daily cleanup of old requests
    let cleanup_state2 = Arc::clone(&app_state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(86400));
        loop {
            interval.tick().await;
            // Read retention days from settings table
            let days = sqlx::query("SELECT value FROM settings WHERE key = 'log_retention_days'")
                .fetch_optional(&cleanup_state2.pool)
                .await
                .ok()
                .flatten()
                .and_then(|r| r.get::<Option<String>, _>("value"))
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(30);

            match models::Request::delete_before(&cleanup_state2.pool, days).await {
                Ok(deleted) => {
                    if !deleted.is_empty() {
                        debug!(
                            "Daily cleanup: deleted {} old requests (retention: {} days)",
                            deleted.len(),
                            days
                        );
                    }
                },
                Err(e) => error!("Daily cleanup failed: {}", e),
            }
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
        .nest("/requests", request_router())
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
