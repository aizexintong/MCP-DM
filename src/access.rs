//! 访问控制：绑定地址与来源 IP 白名单
//!
//! 一个 `BIND` 参数同时表达「绑哪儿」和「谁能连」：
//!
//! | 写法 | 绑定地址 | 允许来源 |
//! |---|---|---|
//! | `127.0.0.1`（默认） | 127.0.0.1 | 仅本机 |
//! | `10.0.0.1` | 10.0.0.1 | 仅该网卡可达者 |
//! | `10.0.0.0/8` | 0.0.0.0 | 仅 10.0.0.0/8 网段 |
//! | `0.0.0.0/0` | 0.0.0.0 | 不限制 |
//! | `0.0.0.0` | 0.0.0.0 | 不限制 |
//!
//! 原因：`bind()` 只能绑具体 IP，不能绑网段。
//! 所以网段写法要绑 `0.0.0.0`，再在请求入口按来源 IP 过滤。

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// 一个 IPv4 网段
#[derive(Debug, Clone, Copy)]
pub struct Cidr {
    net: u32,
    mask: u32,
}

impl Cidr {
    /// 解析 `a.b.c.d/len` 或裸 IP（视为 /32）
    pub fn parse(s: &str) -> Option<Self> {
        let (ip_part, len_part) = match s.split_once('/') {
            Some((ip, len)) => (ip, Some(len)),
            None => (s, None),
        };

        let ip: Ipv4Addr = ip_part.trim().parse().ok()?;
        let len: u32 = match len_part {
            Some(l) => l.trim().parse().ok()?,
            None => 32,
        };
        if len > 32 {
            return None;
        }

        let mask = if len == 0 { 0 } else { u32::MAX << (32 - len) };
        Some(Self {
            net: u32::from(ip) & mask,
            mask,
        })
    }

    pub fn contains(&self, ip: Ipv4Addr) -> bool {
        (u32::from(ip) & self.mask) == self.net
    }

    /// /0 表示不限来源
    pub fn is_wildcard(&self) -> bool {
        self.mask == 0
    }
}

/// 绑定策略
#[derive(Debug, Clone)]
pub struct BindPolicy {
    /// 实际 bind() 用的地址
    pub listen: IpAddr,
    /// 来源白名单；`None` 表示不限制
    pub allow: Option<Vec<Cidr>>,
    /// 原始写法，用于日志
    pub raw: String,
}

impl BindPolicy {
    /// 从 BIND 变量解析。默认 `127.0.0.1`（仅本机）。
    pub fn parse(raw: &str) -> Result<Self, String> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Self::parse("127.0.0.1");
        }

        // 带 / 的按网段处理
        if raw.contains('/') {
            let cidr = Cidr::parse(raw).ok_or_else(|| format!("无法解析网段: {raw}"))?;
            // 网段必须绑 0.0.0.0 才能收到对应流量
            return Ok(Self {
                listen: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                allow: if cidr.is_wildcard() { None } else { Some(vec![cidr]) },
                raw: raw.to_string(),
            });
        }

        // 裸 IP：直接绑该地址
        let ip: IpAddr = raw.parse().map_err(|_| format!("无法解析地址: {raw}"))?;
        let allow = if ip.is_unspecified() {
            None // 0.0.0.0 = 不限制
        } else if ip.is_loopback() {
            Some(vec![Cidr::parse("127.0.0.0/8").expect("常量网段合法")])
        } else {
            // 绑到具体网卡 IP：操作系统的路由已限定，不再额外过滤
            None
        };

        Ok(Self {
            listen: ip,
            allow,
            raw: raw.to_string(),
        })
    }

    /// 来源是否允许
    pub fn permits(&self, addr: &SocketAddr) -> bool {
        let Some(list) = &self.allow else { return true };
        match addr.ip() {
            IpAddr::V4(v4) => list.iter().any(|c| c.contains(v4)),
            // IPv6 只放行回环（本机自测）
            IpAddr::V6(v6) => v6.is_loopback(),
        }
    }

    /// 人类可读的说明
    pub fn describe(&self) -> String {
        match &self.allow {
            None => format!("{}（不限制来源）", self.raw),
            Some(list) if self.raw.contains('/') => {
                let nets: Vec<String> = list
                    .iter()
                    .map(|c| {
                        let bits = (!c.mask).leading_zeros();
                        let len = 32 - bits;
                        let ip = Ipv4Addr::from(c.net);
                        format!("{ip}/{len}")
                    })
                    .collect();
                format!("{}（仅允许 {}）", self.raw, nets.join(", "))
            }
            Some(_) => format!("{}（仅本机）", self.raw),
        }
    }
}

// ============================================================================
// actix 提取器：把它加进 handler 参数即可强制校验来源
// ============================================================================

use actix_web::{dev::Payload, web, FromRequest, HttpRequest};
use std::future::{ready, Ready};

/// 通过校验的来源标记。handler 声明它即触发检查。
#[derive(Debug, Clone, Copy)]
pub struct Allowed;

impl FromRequest for Allowed {
    type Error = actix_web::Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let policy = req.app_data::<web::Data<BindPolicy>>().map(|d| d.get_ref().clone());
        let peer = req.peer_addr();

        let allowed = match (policy, peer) {
            (Some(p), Some(a)) => p.permits(&a),
            // 没有策略 / 拿不到对端（Unix socket 等）→ 放行
            _ => true,
        };

        if allowed {
            ready(Ok(Allowed))
        } else {
            if let Some(a) = peer {
                tracing::warn!("拒绝来源 {a} 访问 {}", req.path());
            }
            ready(Err(actix_web::error::ErrorForbidden("来源地址不在允许范围内")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sa(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    #[test]
    fn default_is_loopback_only() {
        let p = BindPolicy::parse("127.0.0.1").unwrap();
        assert_eq!(p.listen, IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert!(p.permits(&sa("127.0.0.1:1234")));
        assert!(!p.permits(&sa("10.0.1.5:1234")));
    }

    #[test]
    fn cidr_binds_all_and_filters() {
        let p = BindPolicy::parse("10.0.0.0/8").unwrap();
        assert_eq!(p.listen, IpAddr::V4(Ipv4Addr::UNSPECIFIED));
        assert!(p.permits(&sa("10.1.2.3:80")));
        assert!(p.permits(&sa("10.255.255.255:80")));
        assert!(!p.permits(&sa("192.168.1.1:80")));
        assert!(!p.permits(&sa("127.0.0.1:80")));
    }

    #[test]
    fn wildcard_allows_everything() {
        for spec in ["0.0.0.0/0", "0.0.0.0"] {
            let p = BindPolicy::parse(spec).unwrap();
            assert_eq!(p.listen, IpAddr::V4(Ipv4Addr::UNSPECIFIED));
            assert!(p.permits(&sa("8.8.8.8:80")));
            assert!(p.permits(&sa("10.0.1.5:80")));
        }
    }

    #[test]
    fn concrete_ip_binds_itself() {
        let p = BindPolicy::parse("10.0.1.123").unwrap();
        assert_eq!(p.listen, "10.0.1.123".parse::<IpAddr>().unwrap());
        // 绑到具体网卡后由系统路由兜底，应用层不再拦
        assert!(p.permits(&sa("10.0.1.5:80")));
    }

    #[test]
    fn rejects_garbage() {
        assert!(BindPolicy::parse("not-an-ip").is_err());
        assert!(BindPolicy::parse("10.0.0.0/33").is_err());
    }
}
