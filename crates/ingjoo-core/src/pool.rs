use crate::Dialect;

pub type Pool = sqlx::pool::Pool<sqlx::Any>;

pub fn install_drivers() {
    sqlx::any::install_default_drivers();
}

pub async fn connect_pool(url: &str) -> Result<(Pool, Dialect), sqlx::Error> {
    let dialect = if url.starts_with("sqlite:") {
        Dialect::Sqlite
    } else if url.starts_with("postgres:") || url.starts_with("postgresql:") {
        Dialect::Postgres
    } else {
        return Err(sqlx::Error::Configuration(format!("Unsupported database URL: {}", url).into()));
    };
    let pool = Pool::connect(url).await?;
    Ok((pool, dialect))
}

pub fn dialect_from_url(url: &str) -> Dialect {
    if url.starts_with("sqlite:") {
        Dialect::Sqlite
    } else {
        Dialect::Postgres
    }
}
