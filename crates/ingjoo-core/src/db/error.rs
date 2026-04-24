use std::fmt;

/// 存储层错误类型，提供比 anyhow::Error 更精细的错误分类
#[derive(Debug)]
pub enum StoreError {
    NotFound(String),
    UniqueViolation { table: String, column: String },
    ForeignKeyViolation(String),
    Database(String),
    Config(String),
    BadRequest(String),
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
                if msg.contains("UNIQUE constraint") {
                    StoreError::UniqueViolation {
                        table: String::new(),
                        column: String::new(),
                    }
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
