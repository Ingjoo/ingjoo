use clap::Parser;

/// 莺竹框架 — 可动态扩展的 Rust 后端框架
#[derive(Parser, Debug)]
#[command(name = "ingjoo", version, about = "莺竹框架服务")]
pub struct Cli {
    /// 监听地址
    #[arg(long, default_value = "0.0.0.0:3000", env = "INGJOO_BIND")]
    pub bind: String,

    /// 数据库连接 URL
    #[arg(long, default_value = "sqlite:./data/ingjoo.db?mode=rwc", env = "DATABASE_URL")]
    pub database_url: String,

    /// JWT 签名密钥
    #[arg(long, default_value = "ingjoo-default-secret-change-me", env = "INGJOO_JWT_SECRET")]
    pub jwt_secret: String,

    /// 日志级别 (如 "ingjoo_bin=debug,tower_http=trace")
    #[arg(long, default_value = "ingjoo_bin=info", env = "INGJOO_LOG")]
    pub log_level: String,

    /// 插件目录路径
    #[arg(long, default_value = "./plugins", env = "PLUGINS_DIR")]
    pub plugins_dir: String,
}
