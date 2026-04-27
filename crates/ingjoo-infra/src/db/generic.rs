use async_trait::async_trait;
use ingjoo_core::db::error::StoreResult;
use ingjoo_core::module::{FieldType, IdType, ModelDescriptor};
use ingjoo_core::query::domain::{Domain, SqlCondition};
use ingjoo_core::{Dialect, PaginatedResult};
use serde_json::{json, Value};
use sqlx::Row;

/// 动态模型通用 CRUD 存储 trait
#[async_trait]
pub trait GenericRecordStore: Send + Sync {
    async fn ensure_table(&self, model: &ModelDescriptor) -> StoreResult<()>;
    async fn generic_create(&self, model: &ModelDescriptor, data: &Value, uid: Option<&str>) -> StoreResult<Value>;
    async fn generic_read(&self, model: &ModelDescriptor, id: &str) -> StoreResult<Option<Value>>;
    async fn generic_update(
        &self,
        model: &ModelDescriptor,
        id: &str,
        data: &Value,
        uid: Option<&str>,
    ) -> StoreResult<Option<Value>>;
    async fn generic_delete(&self, model: &ModelDescriptor, id: &str) -> StoreResult<bool>;
    async fn generic_list(
        &self,
        model: &ModelDescriptor,
        domain: Option<&Domain>,
        limit: i64,
        offset: i64,
    ) -> StoreResult<PaginatedResult<Value>>;
    async fn generic_count(&self, model: &ModelDescriptor, domain: Option<&Domain>) -> StoreResult<i64>;
}

fn field_type_sql(ft: &FieldType, dialect: &Dialect) -> String {
    match ft {
        FieldType::Text => dialect.text_type().to_string(),
        FieldType::Integer => "INTEGER".to_string(),
        FieldType::Float => "REAL".to_string(),
        FieldType::Boolean => dialect.boolean_type().to_string(),
        FieldType::Timestamp => dialect.timestamp_type().to_string(),
        FieldType::Json => dialect.text_type().to_string(),
        FieldType::Many2one => dialect.text_type().to_string(),
    }
}

fn generate_ddl(model: &ModelDescriptor, dialect: &Dialect) -> String {
    let mut cols = Vec::new();
    match model.id_type {
        IdType::Text => cols.push("id TEXT PRIMARY KEY".to_string()),
        IdType::Integer => cols.push(format!("id {}", dialect.serial_pk())),
    }
    cols.push(format!("created_at {} NOT NULL DEFAULT ({})", dialect.timestamp_type(), dialect.now()));
    cols.push(format!("updated_at {} NOT NULL DEFAULT ({})", dialect.timestamp_type(), dialect.now()));

    for f in &model.fields {
        if f.field_type == FieldType::Many2one {
            if let Some(ref rel) = f.relation {
                let mut col_def = dialect.reference(&f.name, &rel.related_model, "id", "SET NULL");
                if f.required {
                    col_def.push_str(" NOT NULL");
                }
                cols.push(col_def);
            } else {
                let mut col_def = format!("{} {}", f.name, field_type_sql(&f.field_type, dialect));
                if f.required {
                    col_def.push_str(" NOT NULL");
                }
                cols.push(col_def);
            }
        } else {
            let mut col_def = format!("{} {}", f.name, field_type_sql(&f.field_type, dialect));
            if f.required {
                col_def.push_str(" NOT NULL");
            }
            if let Some(ref default) = f.default_value {
                col_def.push_str(&format!(" DEFAULT {}", default));
            }
            if f.unique {
                col_def.push_str(" UNIQUE");
            }
            cols.push(col_def);
        }
    }

    if model.audit_fields {
        cols.push(format!("create_uid {}", dialect.text_type()));
        cols.push(format!("write_uid {}", dialect.text_type()));
        cols.push(format!("create_date {} NOT NULL DEFAULT ({})", dialect.timestamp_type(), dialect.now()));
        cols.push(format!("write_date {} NOT NULL DEFAULT ({})", dialect.timestamp_type(), dialect.now()));
    }

    format!("CREATE TABLE IF NOT EXISTS {} ({})", model.table_name, cols.join(", "))
}

fn row_to_json(row: &sqlx::any::AnyRow, model: &ModelDescriptor) -> Value {
    let mut map = serde_json::Map::new();

    let id: Option<String> = row.try_get("id").ok();
    match id {
        Some(s) => {
            map.insert("id".to_string(), json!(s));
        }
        None => {
            let iid: Option<i64> = row.try_get("id").ok();
            if let Some(i) = iid {
                map.insert("id".to_string(), json!(i));
            }
        }
    }

    let created_at: Option<String> = row.try_get("created_at").ok();
    if let Some(v) = created_at {
        map.insert("created_at".to_string(), json!(v));
    }
    let updated_at: Option<String> = row.try_get("updated_at").ok();
    if let Some(v) = updated_at {
        map.insert("updated_at".to_string(), json!(v));
    }

    if model.audit_fields {
        let create_uid: Option<String> = row.try_get("create_uid").ok().flatten();
        if let Some(v) = create_uid {
            map.insert("create_uid".to_string(), json!(v));
        }
        let write_uid: Option<String> = row.try_get("write_uid").ok().flatten();
        if let Some(v) = write_uid {
            map.insert("write_uid".to_string(), json!(v));
        }
        let create_date: Option<String> = row.try_get("create_date").ok();
        if let Some(v) = create_date {
            map.insert("create_date".to_string(), json!(v));
        }
        let write_date: Option<String> = row.try_get("write_date").ok();
        if let Some(v) = write_date {
            map.insert("write_date".to_string(), json!(v));
        }
    }

    for f in &model.fields {
        let val: Option<String> = row.try_get(f.name.as_str()).ok().flatten();
        match val {
            Some(v) => match f.field_type {
                FieldType::Text | FieldType::Timestamp | FieldType::Json | FieldType::Many2one => {
                    map.insert(f.name.clone(), json!(v));
                }
                FieldType::Integer => {
                    map.insert(f.name.clone(), json!(v.parse::<i64>().unwrap_or(0)));
                }
                FieldType::Float => {
                    map.insert(f.name.clone(), json!(v.parse::<f64>().unwrap_or(0.0)));
                }
                FieldType::Boolean => {
                    let b = v != "0" && !v.is_empty();
                    map.insert(f.name.clone(), json!(b));
                }
            },
            None => {
                let int_val: Option<i64> = row.try_get(f.name.as_str()).ok();
                match int_val {
                    Some(i) => match f.field_type {
                        FieldType::Boolean => {
                            map.insert(f.name.clone(), json!(i != 0));
                        }
                        _ => {
                            map.insert(f.name.clone(), json!(i));
                        }
                    },
                    None => {
                        let real_val: Option<f64> = row.try_get(f.name.as_str()).ok();
                        match real_val {
                            Some(r) => {
                                map.insert(f.name.clone(), json!(r));
                            }
                            None => {
                                map.insert(f.name.clone(), Value::Null);
                            }
                        }
                    }
                }
            }
        }
    }

    Value::Object(map)
}

fn extract_field_value(_model: &ModelDescriptor, data: &Value, field_name: &str) -> Option<String> {
    data.get(field_name).and_then(|v| match v {
        Value::Null => None,
        Value::Bool(b) => Some(if *b { "1".to_string() } else { "0".to_string() }),
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(s.clone()),
        Value::Array(_) | Value::Object(_) => Some(serde_json::to_string(v).unwrap_or_default()),
    })
}

/// SQL 实现的通用动态模型存储
pub struct GenericDb<'a> {
    pool: &'a ingjoo_core::pool::Pool,
    dialect: &'a Dialect,
}

impl<'a> GenericDb<'a> {
    /// 创建通用存储实例
    pub fn new(pool: &'a ingjoo_core::pool::Pool, dialect: &'a Dialect) -> Self {
        Self { pool, dialect }
    }

    fn sql(&self, query: &str) -> String {
        self.dialect.prepare(query)
    }
}

#[async_trait]
impl<'a> GenericRecordStore for GenericDb<'a> {
    async fn ensure_table(&self, model: &ModelDescriptor) -> StoreResult<()> {
        let ddl = generate_ddl(model, self.dialect);
        let sql = self.sql(&ddl);
        sqlx::query(&sql).execute(self.pool).await?;
        Ok(())
    }

    async fn generic_create(&self, model: &ModelDescriptor, data: &Value, uid: Option<&str>) -> StoreResult<Value> {
        let id = match model.id_type {
            IdType::Text => uuid::Uuid::new_v4().to_string(),
            IdType::Integer => String::new(),
        };

        let mut cols = vec!["id".to_string()];
        let mut vals: Vec<String> = Vec::new();
        let mut params: Vec<String> = Vec::new();

        if matches!(model.id_type, IdType::Text) {
            vals.push(self.dialect.placeholder(1));
            params.push(id.clone());
        }

        for f in &model.fields {
            if let Some(v) = extract_field_value(model, data, &f.name) {
                cols.push(f.name.clone());
                vals.push(self.dialect.placeholder(1));
                params.push(v);
            } else if f.required {
                return Err(ingjoo_core::db::error::StoreError::BadRequest(format!("缺少必填字段: {}", f.name)));
            }
        }

        if model.audit_fields {
            if let Some(u) = uid {
                cols.push("create_uid".to_string());
                vals.push(self.dialect.placeholder(1));
                params.push(u.to_string());
                cols.push("write_uid".to_string());
                vals.push(self.dialect.placeholder(1));
                params.push(u.to_string());
            }
        }

        let sql =
            self.sql(&format!("INSERT INTO {} ({}) VALUES ({})", model.table_name, cols.join(", "), vals.join(", "),));

        let mut query = sqlx::query(&sql);
        for p in &params {
            query = query.bind(p);
        }
        query.execute(self.pool).await?;

        let created = if matches!(model.id_type, IdType::Text) {
            self.generic_read(model, &id).await?
        } else {
            let last_id_sql = match self.dialect {
                Dialect::Sqlite => self.sql("SELECT last_insert_rowid()"),
                Dialect::Postgres => format!("SELECT currval(pg_get_serial_sequence('{}', 'id'))", model.table_name),
            };
            let last_id: (i64,) = sqlx::query_as(&last_id_sql).fetch_one(self.pool).await?;
            self.generic_read(model, &last_id.0.to_string()).await?
        };

        Ok(created.unwrap_or(json!({})))
    }

    async fn generic_read(&self, model: &ModelDescriptor, id: &str) -> StoreResult<Option<Value>> {
        let sql = self.sql(&format!("SELECT * FROM {} WHERE id = {}", model.table_name, self.dialect.placeholder(1)));
        let row = sqlx::query(&sql).bind(id).fetch_optional(self.pool).await?;
        Ok(row.map(|r| row_to_json(&r, model)))
    }

    async fn generic_update(
        &self,
        model: &ModelDescriptor,
        id: &str,
        data: &Value,
        uid: Option<&str>,
    ) -> StoreResult<Option<Value>> {
        let mut sets = Vec::new();
        let mut params = Vec::new();

        for f in &model.fields {
            if let Some(v) = extract_field_value(model, data, &f.name) {
                sets.push(format!("{} = {}", f.name, self.dialect.placeholder(1)));
                params.push(v);
            }
        }

        if sets.is_empty() && !model.audit_fields {
            return self.generic_read(model, id).await;
        }

        if sets.is_empty() && model.audit_fields && uid.is_none() {
            return self.generic_read(model, id).await;
        }

        if model.audit_fields {
            if let Some(u) = uid {
                sets.push(format!("write_uid = {}", self.dialect.placeholder(1)));
                params.push(u.to_string());
            }
        }

        sets.push(format!("updated_at = {}", self.dialect.now()));

        let sql = self.sql(&format!(
            "UPDATE {} SET {} WHERE id = {}",
            model.table_name,
            sets.join(", "),
            self.dialect.placeholder(1),
        ));

        let mut query = sqlx::query(&sql);
        for p in &params {
            query = query.bind(p);
        }
        query = query.bind(id);
        query.execute(self.pool).await?;

        self.generic_read(model, id).await
    }

    async fn generic_delete(&self, model: &ModelDescriptor, id: &str) -> StoreResult<bool> {
        let sql = self.sql(&format!("DELETE FROM {} WHERE id = {}", model.table_name, self.dialect.placeholder(1)));
        let result = sqlx::query(&sql).bind(id).execute(self.pool).await?;
        Ok(result.rows_affected() > 0)
    }

    async fn generic_list(
        &self,
        model: &ModelDescriptor,
        domain: Option<&Domain>,
        limit: i64,
        offset: i64,
    ) -> StoreResult<PaginatedResult<Value>> {
        let mut where_clause = String::new();
        let mut domain_params = Vec::new();

        if let Some(d) = domain {
            let cond = d.to_sql_with_dialect(None, Some(self.dialect));
            if !cond.clause.is_empty() {
                where_clause = format!(" WHERE {}", cond.clause);
                domain_params = cond.params;
            }
        }

        let count_sql = self.sql(&format!("SELECT COUNT(*) FROM {}{}", model.table_name, where_clause));
        let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
        for p in &domain_params {
            count_query = count_query.bind(p);
        }
        let total = count_query.fetch_one(self.pool).await?;

        let list_sql = self.sql(&format!(
            "SELECT * FROM {}{} ORDER BY id DESC LIMIT {} OFFSET {}",
            model.table_name,
            where_clause,
            self.dialect.placeholder(1),
            self.dialect.placeholder(1)
        ));
        let mut list_query = sqlx::query(&list_sql);
        for p in &domain_params {
            list_query = list_query.bind(p);
        }
        list_query = list_query.bind(limit).bind(offset);

        let rows = list_query.fetch_all(self.pool).await?;
        let items: Vec<Value> = rows.iter().map(|r| row_to_json(r, model)).collect();

        Ok(PaginatedResult::new(items, total, limit, offset))
    }

    async fn generic_count(&self, model: &ModelDescriptor, domain: Option<&Domain>) -> StoreResult<i64> {
        let mut where_clause = String::new();
        let mut domain_params = Vec::new();

        if let Some(d) = domain {
            let cond = d.to_sql_with_dialect(None, Some(self.dialect));
            if !cond.clause.is_empty() {
                where_clause = format!(" WHERE {}", cond.clause);
                domain_params = cond.params;
            }
        }

        let sql = self.sql(&format!("SELECT COUNT(*) FROM {}{}", model.table_name, where_clause));
        let mut query = sqlx::query_scalar::<_, i64>(&sql);
        for p in &domain_params {
            query = query.bind(p);
        }
        let count = query.fetch_one(self.pool).await?;
        Ok(count)
    }
}

impl<'a> GenericDb<'a> {
    /// 带额外 SQL 过滤条件的分页查询（用于权限过滤等场景）
    pub async fn generic_list_with_filter(
        &self,
        model: &ModelDescriptor,
        domain: Option<&Domain>,
        extra_filter: Option<&SqlCondition>,
        limit: i64,
        offset: i64,
    ) -> StoreResult<PaginatedResult<Value>> {
        let mut where_parts = Vec::new();
        let mut all_params = Vec::new();

        if let Some(d) = domain {
            let cond = d.to_sql_with_dialect(None, Some(self.dialect));
            if !cond.clause.is_empty() {
                where_parts.push(cond.clause);
                all_params.extend(cond.params);
            }
        }

        if let Some(f) = extra_filter {
            if !f.clause.is_empty() {
                where_parts.push(f.clause.clone());
                all_params.extend(f.params.clone());
            }
        }

        let where_clause =
            if where_parts.is_empty() { String::new() } else { format!(" WHERE {}", where_parts.join(" AND ")) };

        let count_sql = self.sql(&format!("SELECT COUNT(*) FROM {}{}", model.table_name, where_clause));
        let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
        for p in &all_params {
            count_query = count_query.bind(p);
        }
        let total = count_query.fetch_one(self.pool).await?;

        let list_sql = self.sql(&format!(
            "SELECT * FROM {}{} ORDER BY id DESC LIMIT {} OFFSET {}",
            model.table_name,
            where_clause,
            self.dialect.placeholder(1),
            self.dialect.placeholder(1)
        ));
        let mut list_query = sqlx::query(&list_sql);
        for p in &all_params {
            list_query = list_query.bind(p);
        }
        list_query = list_query.bind(limit).bind(offset);

        let rows = list_query.fetch_all(self.pool).await?;
        let items: Vec<Value> = rows.iter().map(|r| row_to_json(r, model)).collect();

        Ok(PaginatedResult::new(items, total, limit, offset))
    }

    /// 带额外过滤条件读取单条记录
    pub async fn generic_read_with_filter(
        &self,
        model: &ModelDescriptor,
        id: &str,
        extra_filter: Option<&SqlCondition>,
    ) -> StoreResult<Option<Value>> {
        let mut where_clause = format!("WHERE id = {}", self.dialect.placeholder(1));
        let mut params = vec![id.to_string()];

        if let Some(f) = extra_filter {
            if !f.clause.is_empty() {
                where_clause.push_str(&format!(" AND ({})", f.clause));
                params.extend(f.params.clone());
            }
        }

        let sql = self.sql(&format!("SELECT * FROM {} {}", model.table_name, where_clause));
        let mut query = sqlx::query(&sql);
        for p in &params {
            query = query.bind(p);
        }
        let row = query.fetch_optional(self.pool).await?;
        Ok(row.map(|r| row_to_json(&r, model)))
    }
}
