use std::fmt;

/// 存储层错误类型，提供比 anyhow::Error 更精细的错误分类
#[derive(Debug)]
pub enum StoreError {
    /// 记录不存在
    NotFound(String),
    /// 唯一约束冲突
    UniqueViolation { table: String, column: String },
    /// 外键约束冲突
    ForeignKeyViolation(String),
    /// 数据库内部错误
    Database(String),
    /// 配置错误
    Config(String),
    /// 请求参数错误
    BadRequest(String),
    /// IO 错误
    Io(std::io::Error),
}

/// 存储层操作结果类型
pub type StoreResult<T> = Result<T, StoreError>;

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::NotFound(msg) => write!(f, "记录不存在: {}", msg),
            StoreError::UniqueViolation { table, column } => {
                write!(f, "唯一约束冲突: {}.{}", table, column)
            }
            StoreError::ForeignKeyViolation(msg) => write!(f, "外键约束冲突: {}", msg),
            StoreError::Database(msg) => write!(f, "数据库错误: {}", msg),
            StoreError::Config(msg) => write!(f, "配置错误: {}", msg),
            StoreError::BadRequest(msg) => write!(f, "请求错误: {}", msg),
            StoreError::Io(e) => write!(f, "IO 错误: {}", e),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StoreError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e)
    }
}

impl From<sqlx::Error> for StoreError {
    fn from(e: sqlx::Error) -> Self {
        match &e {
            sqlx::Error::RowNotFound => StoreError::NotFound("record".into()),
            sqlx::Error::Database(db_err) => {
                let msg = db_err.message().to_string();
                // SQLite: "UNIQUE constraint failed: users.email"
                // PostgreSQL: "duplicate key value violates unique constraint"
                // PG error code: 23505
                let is_unique = msg.contains("UNIQUE constraint")
                    || msg.contains("duplicate key")
                    || db_err.code().map(|c| c == "23505").unwrap_or(false);
                if is_unique {
                    StoreError::UniqueViolation { table: String::new(), column: String::new() }
                } else if msg.contains("FOREIGN KEY") {
                    StoreError::ForeignKeyViolation(msg)
                } else {
                    StoreError::Database(msg)
                }
            }
            _ => StoreError::Database(e.to_string()),
        }
    }
}
