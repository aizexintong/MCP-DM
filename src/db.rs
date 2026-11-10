//! 数据库：嵌入式 PostgreSQL 启动、连接池、迁移
use anyhow::{Context, Result};
use postgresql_embedded::{PostgreSQL, SettingsBuilder, VersionReq};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use crate::config::Config;

pub struct Db {
    pub pool: PgPool,
    _pg: Option<PostgreSQL>,   // 保持 PG 进程存活
}

impl Db {
    /// 启动嵌入式 PG（数据目录在 data/pg），返回连接池
    pub async fn start(cfg: &Config) -> Result<Self> {
        let pg_dir = cfg.pg_dir();
        std::fs::create_dir_all(&pg_dir)?;

        let settings = SettingsBuilder::new()
            .data_dir(&pg_dir)
            .port(cfg.pg_port)
            .username("memory")
            .password(&cfg.pg_password)
            .temporary(false)          // 持久化到磁盘
            .version(VersionReq::STAR)
            .build();

        let mut pg = PostgreSQL::new(settings);

        // 已初始化则跳过 setup
        if !pg_dir.join("PG_VERSION").exists() {
            tracing::info!("初始化 PostgreSQL 数据目录: {}", pg_dir.display());
            pg.setup().await.context("PG setup 失败")?;
        }

        tracing::info!("启动 PostgreSQL (port {})", cfg.pg_port);
        tokio::time::timeout(std::time::Duration::from_secs(60), pg.start())
            .await
            .map_err(|_| anyhow::anyhow!("PG start 超时（60秒）"))?
            .context("PG start 失败")?;
        tracing::info!("PostgreSQL 已就绪");

        // 通过 Unix socket 直接连接 memory 库
        let sock = cfg.pg_socket_dir();
        // sqlx: host 参数必须 URL 编码；socket 目录用 host=<urlencoded path>
        let sock_enc: String = sock.to_string_lossy().replace('/', "%2F");
        let url = format!("postgresql://memory@localhost:{}/memory?host={}",
                          pg.settings().port, sock_enc);
        tracing::info!("连接 {}", url);
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(std::time::Duration::from_secs(30))
            .connect(&url)
            .await
            .context("连接 PostgreSQL 失败")?;
        tracing::info!("数据库连接成功");

        Ok(Self { pool, _pg: Some(pg) })
    }

    /// 跑迁移
    pub async fn migrate(&self) -> Result<()> {
        // 检查是否已初始化（categories 表存在）
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM information_schema.tables
                            WHERE table_schema='public' AND table_name='categories')",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(false);

        if exists {
            tracing::info!("数据库已初始化，跳过迁移");
            // 但确保 _sqlx_migrations 有记录，避免下次又跑
            sqlx::query(
                "CREATE TABLE IF NOT EXISTS _sqlx_migrations (
                    version BIGINT PRIMARY KEY, description TEXT NOT NULL,
                    installed_on TIMESTAMPTZ NOT NULL DEFAULT now(),
                    success BOOLEAN NOT NULL, checksum BYTEA NOT NULL,
                    execution_time BIGINT NOT NULL)"
            ).execute(&self.pool).await.ok();
            return Ok(());
        }

        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .context("迁移失败")?;
        tracing::info!("数据库迁移完成");
        Ok(())
    }

}
