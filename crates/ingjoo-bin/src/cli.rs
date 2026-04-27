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

    /// 多数据库基础 URL 模板（启用后支持动态创建数据库连接）
    /// SQLite 示例: "sqlite:./data/"（拼接后为 sqlite:./data/{name}.db）
    /// PostgreSQL 示例: "postgres://user:pass@localhost/"
    #[arg(long, env = "DATABASE_BASE_URL")]
    pub database_base_url: Option<String>,

    /// JWT 签名密钥
    #[arg(long, default_value = "ingjoo-default-secret-change-me", env = "INGJOO_JWT_SECRET")]
    pub jwt_secret: String,

    /// 日志级别 (如 "ingjoo_bin=debug,tower_http=trace")
    #[arg(long, default_value = "ingjoo_bin=info", env = "INGJOO_LOG")]
    pub log_level: String,

    /// 日志格式 (text 或 json)
    #[arg(long, default_value = "text", env = "INGJOO_LOG_FORMAT")]
    pub log_format: String,

    /// 审计日志保留天数（超过此天数的记录自动清理）
    #[arg(long, default_value = "90", env = "INGJOO_AUDIT_RETENTION_DAYS")]
    pub audit_retention_days: u64,

    /// 插件目录路径
    #[arg(long, default_value = "./plugins", env = "PLUGINS_DIR")]
    pub plugins_dir: String,

    /// 数据库管理密码（Basic Auth，仅环境变量）
    #[arg(long, env = "INGJOO_ADMIN_PASSWD", default_value = "admin")]
    pub admin_passwd: String,

    /// 是否允许列出数据库（默认 true）
    #[arg(long, default_value = "true", env = "INGJOO_LIST_DB")]
    pub list_db: bool,

    /// 默认数据库名（可选，启动时自动创建并迁移）
    #[arg(long, env = "INGJOO_DEFAULT_DB")]
    pub default_db: Option<String>,

    /// 数据库名称过滤正则（可选，限制 /api/database/list 返回范围）
    #[arg(long, env = "INGJOO_DBFILTER")]
    pub dbfilter: Option<String>,
}
