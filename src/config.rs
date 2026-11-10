//! 配置：全部从环境变量读取，带合理默认值
use crate::access::BindPolicy;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    /// 数据目录（含 PG 数据、日志）
    pub data_dir: PathBuf,
    /// Web 端口
    pub web_port: u16,
    /// MCP 端口
    pub mcp_port: u16,
    /// 内嵌 PostgreSQL 端口（Unix socket，不监听 TCP）
    pub pg_port: u16,
    /// PostgreSQL 口令
    pub pg_password: String,
    /// 绑定地址与来源白名单，默认仅本机
    pub bind: BindPolicy,
}

impl Config {
    pub fn from_env() -> Self {
        let data_dir = std::env::var("DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join("data")
            });

        Self {
            data_dir,
            web_port: env_u16("WEB_PORT", 8767),
            mcp_port: env_u16("MCP_PORT", 8765),
            pg_port: env_u16("PG_PORT", 55432),
            pg_password: std::env::var("PG_PASSWORD")
                .unwrap_or_else(|_| "memory_local_only".into()),
            bind: {
                let raw = std::env::var("BIND").unwrap_or_else(|_| "127.0.0.1".into());
                BindPolicy::parse(&raw).unwrap_or_else(|e| {
                    eprintln!("BIND 参数无效（{e}），回退到 127.0.0.1");
                    BindPolicy::parse("127.0.0.1").expect("默认值合法")
                })
            },
        }
    }

    pub fn pg_dir(&self) -> PathBuf {
        self.data_dir.join("pg")
    }

    pub fn pg_socket_dir(&self) -> PathBuf {
        self.pg_dir().join("run")
    }
}

fn env_u16(key: &str, default: u16) -> u16 {
    std::env::var(key).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}
