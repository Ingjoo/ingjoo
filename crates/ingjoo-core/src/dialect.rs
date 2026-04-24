#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dialect {
    Sqlite,
    Postgres,
}

impl Dialect {
    pub fn now(&self) -> &'static str {
        match self {
            Self::Sqlite => "datetime('now')",
            Self::Postgres => "CURRENT_TIMESTAMP",
        }
    }

    pub fn now_offset(&self, offset_expr: &str) -> String {
        match self {
            Self::Sqlite => format!("datetime('now', '{}')", offset_expr),
            Self::Postgres => format!("NOW() + INTERVAL '{}'", offset_expr),
        }
    }

    pub fn now_offset_negative(&self, offset_expr: &str) -> String {
        match self {
            Self::Sqlite => format!("datetime('now', '-{}')", offset_expr),
            Self::Postgres => format!("NOW() - INTERVAL '{}'", offset_expr),
        }
    }

    pub fn now_offset_bind(&self, unit: &str) -> String {
        match self {
            Self::Sqlite => format!("datetime('now', ? || '{}')", unit),
            Self::Postgres => format!("NOW() + (? || '{}')::interval", unit),
        }
    }

    pub fn serial_pk(&self) -> &'static str {
        match self {
            Self::Sqlite => "INTEGER PRIMARY KEY AUTOINCREMENT",
            Self::Postgres => "BIGSERIAL PRIMARY KEY",
        }
    }

    pub fn text_type(&self) -> &'static str {
        match self {
            Self::Sqlite => "TEXT",
            Self::Postgres => "VARCHAR",
        }
    }

    pub fn text_type_with_len(&self, len: u32) -> String {
        match self {
            Self::Sqlite => "TEXT".to_string(),
            Self::Postgres => format!("VARCHAR({})", len),
        }
    }

    pub fn timestamp_type(&self) -> &'static str {
        match self {
            Self::Sqlite => "TEXT",
            Self::Postgres => "TIMESTAMPTZ",
        }
    }

    pub fn boolean_type(&self) -> &'static str {
        match self {
            Self::Sqlite => "INTEGER",
            Self::Postgres => "BOOLEAN",
        }
    }

    pub fn blob_type(&self) -> &'static str {
        match self {
            Self::Sqlite => "BLOB",
            Self::Postgres => "BYTEA",
        }
    }

    pub fn auto_migrate_column(&self) -> &'static str {
        match self {
            Self::Sqlite => "ALTER TABLE {table} ADD COLUMN {col} {type_} NOT NULL DEFAULT {default}",
            Self::Postgres => "ALTER TABLE {table} ADD COLUMN IF NOT EXISTS {col} {type_} NOT NULL DEFAULT {default}",
        }
    }

    pub fn unique_constraint(&self, name: &str, columns: &str) -> String {
        match self {
            Self::Sqlite => format!("UNIQUE({})", columns),
            Self::Postgres => format!("CONSTRAINT {} UNIQUE ({})", name, columns),
        }
    }

    pub fn create_index_if_not_exists(&self, name: &str, table: &str, columns: &str) -> String {
        match self {
            Self::Sqlite => format!("CREATE INDEX IF NOT EXISTS {} ON {}({})", name, table, columns),
            Self::Postgres => format!("CREATE INDEX IF NOT EXISTS {} ON {}({})", name, table, columns),
        }
    }

    pub fn upsert(&self, table: &str, columns: &str, conflict_cols: &str, update_cols: &str) -> String {
        match self {
            Self::Sqlite => format!(
                "INSERT INTO {} ({}) VALUES {{}} ON CONFLICT({}) DO UPDATE SET {}",
                table, columns, conflict_cols, update_cols
            ),
            Self::Postgres => format!(
                "INSERT INTO {} ({}) VALUES {{}} ON CONFLICT({}) DO UPDATE SET {}",
                table, columns, conflict_cols, update_cols
            ),
        }
    }

    pub fn placeholder(&self, idx: usize) -> String {
        match self {
            Self::Sqlite => "?".to_string(),
            Self::Postgres => format!("${}", idx),
        }
    }

    pub fn placeholders(&self, count: usize, start_idx: usize) -> Vec<String> {
        match self {
            Self::Sqlite => vec!["?".to_string(); count],
            Self::Postgres => (start_idx..start_idx + count).map(|i| format!("${}", i)).collect(),
        }
    }

    pub fn default_timestamp(&self) -> String {
        format!("DEFAULT {}", self.now())
    }

    pub fn not_null_default_now(&self) -> String {
        format!("{} NOT NULL {}", self.timestamp_type(), self.default_timestamp())
    }

    pub fn reference(&self, col: &str, ref_table: &str, ref_col: &str, on_delete: &str) -> String {
        format!("{} {} REFERENCES {}({}) ON DELETE {}", col, self.text_type(), ref_table, ref_col, on_delete)
    }

    pub fn text_primary_key(&self) -> &'static str {
        "TEXT PRIMARY KEY"
    }

    pub fn integer_not_null_default(&self, default: i64) -> String {
        format!("INTEGER NOT NULL DEFAULT {}", default)
    }

    pub fn prepare(&self, sql: &str) -> String {
        let sql = self.replace_standalone_datetime_now(sql);
        let sql = sql.replace("INTEGER PRIMARY KEY AUTOINCREMENT", self.serial_pk());
        self.format_sql(&sql)
    }

    fn replace_standalone_datetime_now(&self, sql: &str) -> String {
        if matches!(self, Self::Sqlite) {
            return sql.to_string();
        }
        let pattern = "datetime('now')";
        let pattern_with_comma = "datetime('now',";
        let mut result = String::with_capacity(sql.len());
        let mut i = 0;
        while i < sql.len() {
            if sql[i..].starts_with(pattern_with_comma) {
                result.push_str(pattern_with_comma);
                i += pattern_with_comma.len();
            } else if sql[i..].starts_with(pattern) {
                result.push_str(self.now());
                i += pattern.len();
            } else {
                result.push(sql.as_bytes()[i] as char);
                i += 1;
            }
        }
        result
    }

    pub fn format_sql(&self, sql: &str) -> String {
        match self {
            Self::Sqlite => sql.to_string(),
            Self::Postgres => {
                let chars: Vec<char> = sql.chars().collect();
                let mut result = String::with_capacity(sql.len() + 16);
                let mut n = 1usize;
                let mut in_string = false;
                let mut string_char = ' ';
                let mut i = 0;
                while i < chars.len() {
                    let ch = chars[i];
                    if in_string {
                        result.push(ch);
                        if ch == string_char {
                            in_string = false;
                        }
                        i += 1;
                    } else if ch == '\'' {
                        in_string = true;
                        string_char = ch;
                        result.push(ch);
                        i += 1;
                    } else if ch == '?' {
                        let mut num_start = i + 1;
                        while num_start < chars.len() && chars[num_start].is_ascii_digit() {
                            num_start += 1;
                        }
                        if num_start > i + 1 {
                            let pos: String = chars[i + 1..num_start].iter().collect();
                            result.push('$');
                            result.push_str(&pos);
                            i = num_start;
                        } else {
                            result.push_str(&format!("${}", n));
                            n += 1;
                             i += 1;
                        }
                    } else {
                        result.push(ch);
                        i += 1;
                    }
                }
                result
            }
        }
    }

    pub fn column_exists_sql(&self, table: &str, column: &str) -> String {
        match self {
            Self::Sqlite => format!(
                "SELECT COUNT(*) FROM pragma_table_info('{}') WHERE name = '{}'",
                table, column
            ),
            Self::Postgres => format!(
                "SELECT COUNT(*) FROM information_schema.columns WHERE table_name = '{}' AND column_name = '{}'",
                table, column
            ),
        }
    }

    pub fn split_ddl(ddl: &str) -> Vec<&str> {
        ddl.split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_now() {
        assert_eq!(Dialect::Sqlite.now(), "datetime('now')");
    }

    #[test]
    fn postgres_now() {
        assert_eq!(Dialect::Postgres.now(), "CURRENT_TIMESTAMP");
    }

    #[test]
    fn sqlite_now_offset() {
        assert_eq!(Dialect::Sqlite.now_offset("1 day"), "datetime('now', '1 day')");
    }

    #[test]
    fn postgres_now_offset() {
        assert_eq!(Dialect::Postgres.now_offset("1 day"), "NOW() + INTERVAL '1 day'");
    }

    #[test]
    fn sqlite_serial_pk() {
        assert_eq!(Dialect::Sqlite.serial_pk(), "INTEGER PRIMARY KEY AUTOINCREMENT");
    }

    #[test]
    fn postgres_serial_pk() {
        assert_eq!(Dialect::Postgres.serial_pk(), "BIGSERIAL PRIMARY KEY");
    }

    #[test]
    fn placeholder_sqlite() {
        assert_eq!(Dialect::Sqlite.placeholder(1), "?");
        assert_eq!(Dialect::Sqlite.placeholder(5), "?");
    }

    #[test]
    fn placeholder_postgres() {
        assert_eq!(Dialect::Postgres.placeholder(1), "$1");
        assert_eq!(Dialect::Postgres.placeholder(5), "$5");
    }

    #[test]
    fn boolean_type_sqlite() {
        assert_eq!(Dialect::Sqlite.boolean_type(), "INTEGER");
    }

    #[test]
    fn boolean_type_postgres() {
        assert_eq!(Dialect::Postgres.boolean_type(), "BOOLEAN");
    }

    #[test]
    fn timestamp_type() {
        assert_eq!(Dialect::Sqlite.timestamp_type(), "TEXT");
        assert_eq!(Dialect::Postgres.timestamp_type(), "TIMESTAMPTZ");
    }

    #[test]
    fn placeholders_sqlite() {
        let phs = Dialect::Sqlite.placeholders(3, 1);
        assert_eq!(phs, vec!["?", "?", "?"]);
    }

    #[test]
    fn placeholders_postgres() {
        let phs = Dialect::Postgres.placeholders(3, 1);
        assert_eq!(phs, vec!["$1", "$2", "$3"]);
    }

    #[test]
    fn placeholders_postgres_with_offset() {
        let phs = Dialect::Postgres.placeholders(3, 4);
        assert_eq!(phs, vec!["$4", "$5", "$6"]);
    }

    #[test]
    fn format_sql_sqlite_unchanged() {
        let d = Dialect::Sqlite;
        assert_eq!(
            d.format_sql("SELECT * FROM users WHERE id = ?"),
            "SELECT * FROM users WHERE id = ?"
        );
    }

    #[test]
    fn format_sql_postgres_placeholders() {
        let d = Dialect::Postgres;
        assert_eq!(
            d.format_sql("SELECT * FROM users WHERE id = ? AND name = ?"),
            "SELECT * FROM users WHERE id = $1 AND name = $2"
        );
    }

    #[test]
    fn format_sql_preserves_string_literals() {
        let d = Dialect::Postgres;
        assert_eq!(
            d.format_sql("INSERT INTO t (a, b) VALUES (?, 'what?')"),
            "INSERT INTO t (a, b) VALUES ($1, 'what?')"
        );
    }

    #[test]
    fn format_sql_no_placeholders() {
        let d = Dialect::Postgres;
        assert_eq!(
            d.format_sql("SELECT COUNT(*) FROM users"),
            "SELECT COUNT(*) FROM users"
        );
    }

    #[test]
    fn format_sql_multiple_placeholders() {
        let d = Dialect::Postgres;
        assert_eq!(
            d.format_sql("UPDATE t SET a=?, b=?, c=? WHERE id=?"),
            "UPDATE t SET a=$1, b=$2, c=$3 WHERE id=$4"
        );
    }

    #[test]
    fn prepare_standalone_datetime_now_postgres() {
        let d = Dialect::Postgres;
        assert_eq!(
            d.prepare("created_at TEXT NOT NULL DEFAULT (datetime('now'))"),
            "created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)"
        );
    }

    #[test]
    fn prepare_standalone_datetime_now_sqlite_unchanged() {
        let d = Dialect::Sqlite;
        assert_eq!(
            d.prepare("created_at TEXT NOT NULL DEFAULT (datetime('now'))"),
            "created_at TEXT NOT NULL DEFAULT (datetime('now'))"
        );
    }

    #[test]
    fn prepare_skips_datetime_now_with_offset_postgres() {
        let d = Dialect::Postgres;
        assert_eq!(
            d.prepare("created_at > datetime('now', '-24 hours')"),
            "created_at > datetime('now', '-24 hours')"
        );
    }

    #[test]
    fn prepare_mixed_datetime_postgres() {
        let d = Dialect::Postgres;
        assert_eq!(
            d.prepare("DEFAULT (datetime('now')), CHECK(created_at > datetime('now', '-1 day'))"),
            "DEFAULT (CURRENT_TIMESTAMP), CHECK(created_at > datetime('now', '-1 day'))"
        );
    }

    #[test]
    fn prepare_autoincrement_postgres() {
        let d = Dialect::Postgres;
        assert_eq!(
            d.prepare("id INTEGER PRIMARY KEY AUTOINCREMENT"),
            "id BIGSERIAL PRIMARY KEY"
        );
    }

    #[test]
    fn prepare_autoincrement_sqlite_unchanged() {
        let d = Dialect::Sqlite;
        assert_eq!(
            d.prepare("id INTEGER PRIMARY KEY AUTOINCREMENT"),
            "id INTEGER PRIMARY KEY AUTOINCREMENT"
        );
    }

    #[test]
    fn format_sql_positional_placeholders_postgres() {
        let d = Dialect::Postgres;
        assert_eq!(
            d.format_sql("WHERE name LIKE ?1 OR email LIKE ?1 LIMIT ?2"),
            "WHERE name LIKE $1 OR email LIKE $1 LIMIT $2"
        );
    }

    #[test]
    fn format_sql_positional_placeholders_sqlite_unchanged() {
        let d = Dialect::Sqlite;
        assert_eq!(
            d.format_sql("WHERE name LIKE ?1 OR email LIKE ?1 LIMIT ?2"),
            "WHERE name LIKE ?1 OR email LIKE ?1 LIMIT ?2"
        );
    }

    #[test]
    fn format_sql_mixed_positional_and_sequential_postgres() {
        let d = Dialect::Postgres;
        assert_eq!(
            d.format_sql("WHERE id = ? AND name LIKE ?1 LIMIT ?2"),
            "WHERE id = $1 AND name LIKE $1 LIMIT $2"
        );
    }

    #[test]
    fn column_exists_sql_sqlite() {
        let sql = Dialect::Sqlite.column_exists_sql("entries", "folder_id");
        assert_eq!(
            sql,
            "SELECT COUNT(*) FROM pragma_table_info('entries') WHERE name = 'folder_id'"
        );
    }

    #[test]
    fn column_exists_sql_postgres() {
        let sql = Dialect::Postgres.column_exists_sql("entries", "folder_id");
        assert_eq!(
            sql,
            "SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'entries' AND column_name = 'folder_id'"
        );
    }

    #[test]
    fn split_ddl_basic() {
        let ddl = "CREATE TABLE t1 (id INTEGER); CREATE TABLE t2 (id INTEGER);";
        let stmts = Dialect::split_ddl(ddl);
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "CREATE TABLE t1 (id INTEGER)");
        assert_eq!(stmts[1], "CREATE TABLE t2 (id INTEGER)");
    }

    #[test]
    fn split_ddl_trims_whitespace() {
        let ddl = r#"
            CREATE TABLE t1 (id INTEGER);

            CREATE TABLE t2 (id INTEGER);
        "#;
        let stmts = Dialect::split_ddl(ddl);
        assert_eq!(stmts.len(), 2);
        assert!(stmts[0].starts_with("CREATE TABLE t1"));
        assert!(stmts[1].starts_with("CREATE TABLE t2"));
    }

    #[test]
    fn split_ddl_with_datetime_default() {
        let ddl = r#"CREATE TABLE t (id TEXT, ts TEXT NOT NULL DEFAULT (datetime('now')));"#;
        let stmts = Dialect::split_ddl(ddl);
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("datetime('now')"));
    }

    #[test]
    fn prepare_ddl_roundtrip_postgres() {
        let d = Dialect::Postgres;
        let ddl = r#"CREATE TABLE t (id INTEGER PRIMARY KEY AUTOINCREMENT, ts TEXT NOT NULL DEFAULT (datetime('now')));"#;
        let transformed = d.prepare(ddl);
        let stmts = Dialect::split_ddl(&transformed);
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("BIGSERIAL PRIMARY KEY"));
        assert!(stmts[0].contains("CURRENT_TIMESTAMP"));
        assert!(!stmts[0].contains("AUTOINCREMENT"));
        assert!(!stmts[0].contains("datetime('now')"));
    }

    #[test]
    fn prepare_ddl_roundtrip_sqlite_unchanged() {
        let d = Dialect::Sqlite;
        let ddl = r#"CREATE TABLE t (id INTEGER PRIMARY KEY AUTOINCREMENT, ts TEXT NOT NULL DEFAULT (datetime('now')));"#;
        let transformed = d.prepare(ddl);
        assert_eq!(transformed, ddl);
    }
}
