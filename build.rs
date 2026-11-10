//! 构建期注入版本号
//!
//! - 本地构建：取 Cargo.toml 的 version
//! - CI 构建：取环境变量 BUILD_VERSION（由 tag 推导）
//!
//! 这样 CI 不需要修改任何文件就能打出正确的版本号，
//! 仓库历史里不会出现机器提交。
fn main() {
    println!("cargo:rerun-if-env-changed=BUILD_VERSION");

    let version = std::env::var("BUILD_VERSION")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "dev".into()));

    println!("cargo:rustc-env=APP_VERSION={}", version.trim());
}
