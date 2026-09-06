use carddue::{AppState,StateInner,config::Config};
use sqlx::ConnectOptions;
use std::{str::FromStr,sync::Arc};

#[tokio::main]
async fn main()->anyhow::Result<()>{
    let command=std::env::args().nth(1);
    if command.as_deref()==Some("openapi") {println!("{}",serde_json::to_string_pretty(&carddue::spec::document())?);return Ok(());}
    if command.as_deref()==Some("generate-key") {println!("{}",carddue::crypto::random_token());return Ok(());}
    tracing_subscriber::fmt().json().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_|"carddue=info,tower_http=warn,sqlx=warn".into())).init();
    let cfg=Config::load()?;
    let options=sqlx::postgres::PgConnectOptions::from_str(&cfg.database_url)?.disable_statement_logging();
    let db=sqlx::postgres::PgPoolOptions::new().max_connections(12).acquire_timeout(std::time::Duration::from_secs(40)).connect_with(options).await?;
    sqlx::migrate!("./migrations").run(&db).await?;
    if command.as_deref()==Some("migrate"){return Ok(());}
    let state:AppState=Arc::new(StateInner{db,config:cfg.clone(),passwords:Arc::new(tokio::sync::Semaphore::new(4))});
    if cfg.role!="web" {
        tokio::spawn(carddue::worker::plan_all(state.clone()));
        tokio::spawn(carddue::worker::run(state.clone()));
    }
    if cfg.role=="worker" {shutdown().await;}else{
        let listener=tokio::net::TcpListener::bind(cfg.bind).await?;
        tracing::info!(address=%cfg.bind,role=%cfg.role,"CardDue started");
        axum::serve(listener,carddue::web::router(state.clone()).into_make_service_with_connect_info::<std::net::SocketAddr>()).with_graceful_shutdown(shutdown()).await?;
    }
    state.db.close().await;Ok(())
}
async fn shutdown(){
    #[cfg(unix)] {
        let mut signal=tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select!{_ = tokio::signal::ctrl_c()=>{},_ = signal.recv()=>{}}
    }
    #[cfg(not(unix))] {let _=tokio::signal::ctrl_c().await;}
}
