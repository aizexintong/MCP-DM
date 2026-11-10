//! 知识库：Actix-web + PostgreSQL(embedded) + rmcp
mod access;
mod config;
mod db;
mod handlers;
mod mcp;
mod models;
mod queries;

use config::Config;

use actix_web::{web, App, HttpServer};
use anyhow::Result;
use std::sync::Arc;

#[actix_web::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,actix_web=warn".into()),
        )
        .init();

    let cfg = Config::from_env();
    std::fs::create_dir_all(&cfg.data_dir)?;

    tracing::info!("数据目录: {}", cfg.data_dir.display());
    let database = db::Db::start(&cfg).await?;
    database.migrate().await?;

    let pool = Arc::new(database.pool.clone());
    let pool_mcp = pool.clone();
    let bind_mcp = cfg.bind.clone();
    let bind_web = cfg.bind.clone();
    let _ = &bind_web;

    // MCP 服务：独立线程 + 独立 runtime（rmcp 的 future 不满足 Send）
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("mcp runtime");
        rt.block_on(async move {
            if let Err(e) = mcp::serve(pool_mcp, cfg.mcp_port, bind_mcp).await {
                tracing::error!("MCP 服务异常: {:#}", e);
            }
        });
    });

    let listen = cfg.bind.listen;
    tracing::info!("访问控制: {}", cfg.bind.describe());
    tracing::info!("Web  http://{}:{}", listen, cfg.web_port);
    tracing::info!("MCP  http://{}:{}/mcp", listen, cfg.mcp_port);

    // 保持 database 存活（PG 进程）
    let _keep = database;

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(cfg.bind.clone()))
            .configure(handlers::config)
    })
    .bind((listen, cfg.web_port))?
    .run()
    .await?;

    Ok(())
}
