//! MCP 服务（rmcp 标准库，streamable-http）
use crate::queries;
use anyhow::Result;
use rmcp::handler::server::ServerHandler;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData as McpError, RoleServer};
use serde_json::{json, Map, Value};
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct Memory { pub pool: Arc<PgPool> }

fn schema(props: Value, required: Vec<&str>) -> Arc<Map<String, Value>> {
    Arc::new({
        let mut m = Map::new();
        m.insert("type".into(), json!("object"));
        m.insert("properties".into(), props);
        m.insert("required".into(), json!(required));
        m
    })
}

fn tools() -> Vec<Tool> {
    let t = |name: &'static str, desc: &'static str, props: Value, req: Vec<&'static str>| -> Tool {
        let mut tool = Tool::default();
        tool.name = name.into();
        tool.description = Some(desc.into());
        tool.input_schema = schema(props, req);
        tool
    };
    vec![
        t("mem_search",
          "在知识库里搜索。全文+模糊混合（PostgreSQL tsvector + pg_trgm）。返回带因果(cause/effect)、状态(state)、标记(marks)的条目。查不到返回空数组，此时应询问用户而非编造。",
          json!({
            "query":{"type":"string","description":"搜索词"},
            "limit":{"type":"integer","description":"返回条数，默认20"},
            "sort":{"type":"string","enum":["relevance","newest","marked"]},
            "cat":{"type":"string","description":"按分类过滤，如 MC服务器 或 MC服务器/服务端部署"},
            "days":{"type":"integer","description":"只要最近N天"},
            "state":{"type":"string","enum":["current","archived","obsolete"]}
          }), vec!["query"]),
        t("mem_get","读取条目完整内容（正文/因果/引用/被引用/标记）。",
          json!({"id":{"type":"integer"}}), vec!["id"]),
        t("mem_add","新建条目。必须写 cause（起因）和 effect（结果）。cat_path 用 / 分隔，如 MC服务器/服务端部署/配置。",
          json!({
            "title":{"type":"string"},
            "body":{"type":"string"},
            "cause":{"type":"string"},
            "effect":{"type":"string"},
            "cat_path":{"type":"string"},
            "tags":{"type":"array","items":{"type":"string"}},
            "source":{"type":"array","items":{"type":"string"}}
          }), vec!["title","cat_path"]),
        t("mem_categories","列出所有分类及其条目数。", json!({}), vec![]),
        t("mem_stats","知识库统计。", json!({}), vec![]),
        t("mem_timeline","时间线：按时间倒序的条目演进。",
          json!({
            "cat":{"type":"string"},
            "limit":{"type":"integer","description":"默认 100"}
          }), vec![]),
        t("mem_mark","标记：0=取消 1=收藏 2=精选 3=精选+收藏。",
          json!({"id":{"type":"integer"},"level":{"type":"integer"}}), vec!["id","level"]),
        t("mem_like","点赞。",
          json!({"id":{"type":"integer"},"delta":{"type":"integer"}}), vec!["id"]),
    ]
}

fn ok_text(s: String) -> CallToolResponse {
    CallToolResult::success(vec![ContentBlock::text(s)]).into()
}
fn arg_str(a: &Option<Map<String, Value>>, k: &str) -> Option<String> {
    a.as_ref()?.get(k)?.as_str().map(|s| s.to_string())
}
fn arg_i64(a: &Option<Map<String, Value>>, k: &str) -> Option<i64> {
    a.as_ref()?.get(k).and_then(|v| v.as_i64())
}
fn arg_vec(a: &Option<Map<String, Value>>, k: &str) -> Vec<String> {
    a.as_ref()
        .and_then(|m| m.get(k))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

impl Memory {
    async fn search(&self, a: &Option<Map<String, Value>>) -> Result<String, McpError> {
        let q = arg_str(a, "query").unwrap_or_default();
        if q.trim().is_empty() {
            return Err(McpError::invalid_params("query is required", None));
        }
        let limit = arg_i64(a, "limit").unwrap_or(20);
        let sort = arg_str(a, "sort").unwrap_or_else(|| "relevance".into());
        let cat = arg_str(a, "cat");
        let days = arg_i64(a, "days");
        let state = arg_str(a, "state");
        let hits = queries::search(&self.pool, &q, limit, &sort, cat.as_deref(), days, state.as_deref())
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let ids: Vec<i64> = hits.iter().take(20).map(|h| h.id).collect();
        let _ = queries::record_hits(&self.pool, &ids).await;
        Ok(serde_json::to_string_pretty(&json!({
            "status": if hits.is_empty() { "no_seed" } else { "found" },
            "query": q,
            "total": hits.len(),
            "results": hits,
        })).unwrap_or_default())
    }

    async fn get(&self, a: &Option<Map<String, Value>>) -> Result<String, McpError> {
        let id = arg_i64(a, "id").ok_or_else(|| McpError::invalid_params("id required", None))?;
        let e = queries::get_entry(&self.pool, id).await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params("not found", None))?;
        let out = queries::get_links_out(&self.pool, id).await.unwrap_or_default();
        let inn = queries::get_links_in(&self.pool, id).await.unwrap_or_default();
        Ok(serde_json::to_string_pretty(&json!({
            "id": e.id, "title": e.title, "cat_path": e.cat_path,
            "state": e.state, "secrecy": e.secrecy,
            "cause": e.cause, "effect": e.effect,
            "body": e.body, "tags": e.tags, "source": e.source,
            "mark_level": e.mark_level, "likes": e.likes, "hits": e.hits,
            "updated_at": e.updated_at,
            "links_out": out, "links_in": inn,
        })).unwrap_or_default())
    }

    async fn add(&self, a: &Option<Map<String, Value>>) -> Result<String, McpError> {
        let title = arg_str(a, "title").unwrap_or_default();
        let cat = arg_str(a, "cat_path").unwrap_or_else(|| "杂项/待分类".into());
        if title.is_empty() {
            return Err(McpError::invalid_params("title required", None));
        }
        let id = queries::create_entry(
            &self.pool, &title,
            &arg_str(a, "body").unwrap_or_default(),
            &arg_str(a, "cause").unwrap_or_default(),
            &arg_str(a, "effect").unwrap_or_default(),
            &cat,
            &arg_vec(a, "tags"),
            &arg_vec(a, "source"),
        ).await.map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(json!({ "ok": true, "id": id }).to_string())
    }
}

impl ServerHandler for Memory {
    fn get_info(&self) -> ServerConfig {
        let mut info = ServerConfig::new(ServerCapabilities::builder().enable_tools().build());
        info.server_info = Implementation::new("memory", "0.2.0");
        info.instructions = Some(
            "知识库（PostgreSQL 全文+模糊检索）。\
             遇到「上次/之前/那个」、问项目事实、开始新任务时，先调 mem_search。\
             返回 status=no_seed 表示没有，此时询问用户而非编造。\
             每条有 cause/effect，回答时说明因果。\
             state: current 有效 / archived 历史 / obsolete 失效，别引用 obsolete。".into());
        info
    }

    async fn list_tools(
        &self, _req: Option<PaginatedRequestParams>, _ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let mut r = ListToolsResult::default();
        r.tools = tools();
        Ok(r)
    }

    async fn call_tool(
        &self, req: CallToolRequestParams, _ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let a = req.arguments.clone();
        let out = match req.name.as_ref() {
            "mem_search" => self.search(&a).await,
            "mem_get" => self.get(&a).await,
            "mem_add" => self.add(&a).await,
            "mem_categories" => {
                let cats = queries::list_categories(&self.pool).await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(serde_json::to_string_pretty(&json!({"categories": cats})).unwrap_or_default())
            }
            "mem_stats" => {
                let (t, c, l, m) = queries::stats(&self.pool).await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(json!({"entries": t, "categories": c, "links": l, "marked": m}).to_string())
            }
            "mem_timeline" => {
                let cat = arg_str(&a, "cat");
                let limit = arg_i64(&a, "limit").unwrap_or(100);
                let items = queries::timeline(&self.pool, cat.as_deref(), limit).await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(serde_json::to_string_pretty(&json!({"timeline": items})).unwrap_or_default())
            }
            "mem_mark" => {
                let id = arg_i64(&a, "id").unwrap_or(0);
                let lv = arg_i64(&a, "level").unwrap_or(0) as i32;
                queries::set_mark(&self.pool, id, lv).await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(json!({"ok": true}).to_string())
            }
            "mem_like" => {
                let id = arg_i64(&a, "id").unwrap_or(0);
                let d = arg_i64(&a, "delta").unwrap_or(1) as i32;
                let n = queries::like(&self.pool, id, d).await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(json!({"ok": true, "likes": n}).to_string())
            }
            other => Err(McpError::invalid_params(format!("unknown tool: {}", other), None)),
        };
        out.map(ok_text)
    }
}

pub async fn serve(
    pool: Arc<PgPool>,
    port: u16,
    bind: crate::access::BindPolicy,
) -> Result<()> {
    let svc: StreamableHttpService<Memory, LocalSessionManager> = StreamableHttpService::new(
        move || Ok(Memory { pool: pool.clone() }),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let app = actix_web::App::new()
        .service(actix_web::web::scope("").default_service(actix_web::web::to(
            move |req: actix_web::HttpRequest, body: actix_web::web::Bytes| {
                let _ = (&req, &body);
                async { actix_web::HttpResponse::Ok().finish() }
            },
        )));
    let _ = app;
    // rmcp 的 StreamableHttpService 是 tower Service，用 axum 承载
    let axum_app = axum::Router::new().nest_service("/mcp", svc);
    let listener = tokio::net::TcpListener::bind((bind.listen, port)).await?;
    tracing::info!("MCP 监听 {}:{}", bind.listen, port);
    axum::serve(listener, axum_app).await?;
    Ok(())
}
