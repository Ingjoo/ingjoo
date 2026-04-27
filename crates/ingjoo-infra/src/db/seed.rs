//! 元数据种子数据
//!
//! 首次启动时向 ir_menu / ir_view / ir_action 插入默认记录。
//! 使用 INSERT OR IGNORE (SQLite) / ON CONFLICT DO NOTHING (PG) 保证幂等。

use anyhow::Result;
use ingjoo_core::module::{ActionDescriptor, MenuDescriptor, ViewDescriptor};
use ingjoo_core::pool::Pool;
use ingjoo_core::Dialect;
use std::collections::HashMap;

fn upsert_sql(dialect: &Dialect, table: &str, columns: &str, values: &str) -> String {
    match dialect {
        Dialect::Sqlite => format!("INSERT OR IGNORE INTO {table} ({columns}) VALUES ({values})"),
        _ => format!("INSERT INTO {table} ({columns}) VALUES ({values}) ON CONFLICT (id) DO NOTHING"),
    }
}

pub async fn seed_model_access(pool: &Pool, dialect: &Dialect, models: &[&str]) -> Result<()> {
    type Perm = (bool, bool, bool, bool, bool, bool);
    let groups: &[(&str, Perm)] = &[
        ("admin", (true, true, true, true, true, true)),
        ("user", (true, true, true, false, true, true)),
        ("viewer", (true, false, false, false, false, true)),
    ];

    for model in models {
        for (group, (pr, pw, pc, pd, pi, pe)) in groups {
            let id = format!("ma_{group}_{model}");
            let sql = dialect.prepare(&upsert_sql(
                dialect,
                "model_access",
                "id, group_id, model, perm_read, perm_write, perm_create, perm_delete, perm_import, perm_export",
                "?, ?, ?, ?, ?, ?, ?, ?, ?",
            ));
            sqlx::query(&sql)
                .bind(&id)
                .bind(group)
                .bind(model)
                .bind(*pr as i32)
                .bind(*pw as i32)
                .bind(*pc as i32)
                .bind(*pd as i32)
                .bind(*pi as i32)
                .bind(*pe as i32)
                .execute(pool)
                .await?;
        }
    }

    Ok(())
}

/// 插入默认元数据种子（幂等）
pub async fn seed_metadata(pool: &Pool, dialect: &Dialect) -> Result<()> {
    // 检查是否已有数据
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM ir_menu").fetch_one(pool).await?;
    if count.0 > 0 {
        return Ok(());
    }

    let (ignore_sql, now_fn) = match dialect {
        Dialect::Sqlite => ("INSERT OR IGNORE INTO", "datetime('now')"),
        _ => ("INSERT INTO", "CURRENT_TIMESTAMP"),
    };
    let on_conflict = match dialect {
        Dialect::Sqlite => "",
        _ => " ON CONFLICT (id) DO NOTHING",
    };

    // ── Actions ──
    let actions = vec![
        ("action_apps", "应用管理", "ir.actions.client", "", "", "[]"),
        ("action_settings", "系统设置", "ir.actions.client", "", "", "[]"),
    ];

    for (id, name, atype, res_model, view_mode, view_ids) in &actions {
        let sql = format!(
            "{ignore_sql} ir_action (id, name, type, res_model, view_mode, view_ids, target, page_limit, url, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, 'current', 40, ?, {now_fn}, {now_fn}){on_conflict}"
        );
        sqlx::query(&dialect.prepare(&sql))
            .bind(id)
            .bind(name)
            .bind(atype)
            .bind(res_model)
            .bind(view_mode)
            .bind(view_ids)
            .bind(name)
            .execute(pool)
            .await?;
    }

    // ── Search Views ──
    let search_views = vec![
        (
            "search_users",
            "用户搜索",
            "users",
            r#"{"fields":[{"name":"name","label":"姓名","type":"text"},{"name":"email","label":"邮箱","type":"text"},{"name":"role","label":"角色","type":"select","options":[["admin","管理员"],["user","用户"],["viewer","查看者"]]}],"filters":[{"name":"admins","label":"管理员","domain":[["role","=","admin"]]}],"group_by":[{"name":"role","label":"按角色分组"}]}"#,
        ),
        (
            "search_articles",
            "文章搜索",
            "article",
            r#"{"fields":[{"name":"title","label":"标题","type":"text"},{"name":"content","label":"内容","type":"text"},{"name":"status","label":"状态","type":"select","options":[["draft","草稿"],["published","已发布"]]}],"filters":[{"name":"published","label":"已发布","domain":[["status","=","published"]]}],"group_by":[{"name":"status","label":"按状态分组"}]}"#,
        ),
        (
            "search_products",
            "产品搜索",
            "product",
            r#"{"fields":[{"name":"name","label":"名称","type":"text"},{"name":"price","label":"价格","type":"number"},{"name":"category","label":"分类","type":"text"}],"filters":[{"name":"low_price","label":"低价产品","domain":[["price","<","100"]]}],"group_by":[{"name":"category","label":"按分类分组"}]}"#,
        ),
    ];

    for (id, name, model, arch) in &search_views {
        let sql = format!(
            "{ignore_sql} ir_view (id, name, model, type, priority, arch, active, group_ids, created_at, updated_at) VALUES (?, ?, ?, 'search', 16, ?, 1, '[]', {now_fn}, {now_fn}){on_conflict}"
        );
        sqlx::query(&dialect.prepare(&sql)).bind(id).bind(name).bind(model).bind(arch).execute(pool).await?;
    }

    // ── Demo Actions ──
    let demo_actions = vec![
        ("action_users", "用户管理", "act_window", "users", "list,form", "search_users"),
        ("action_articles", "文章管理", "act_window", "article", "list,form", "search_articles"),
        ("action_products", "产品管理", "act_window", "product", "list,form", "search_products"),
    ];

    for (id, name, atype, res_model, view_mode, search_view_id) in &demo_actions {
        let sql = format!(
            "{ignore_sql} ir_action (id, name, type, res_model, view_mode, search_view_id, view_ids, target, page_limit, url, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, '[]', 'current', 80, '', {now_fn}, {now_fn}){on_conflict}"
        );
        sqlx::query(&dialect.prepare(&sql))
            .bind(id)
            .bind(name)
            .bind(atype)
            .bind(res_model)
            .bind(view_mode)
            .bind(search_view_id)
            .execute(pool)
            .await?;
    }

    // ── Menus ──
    let menus = vec![
        ("menu_apps", "应用管理", None::<&str>, "fa fa-th-large", Some("action_apps")),
        ("menu_settings", "系统设置", None::<&str>, "fa fa-cog", Some("action_settings")),
        ("menu_users", "用户管理", None::<&str>, "fa fa-users", Some("action_users")),
        ("menu_articles", "文章管理", None::<&str>, "fa fa-file-alt", Some("action_articles")),
        ("menu_products", "产品管理", None::<&str>, "fa fa-box", Some("action_products")),
    ];

    for (id, name, parent_id, icon, action_id) in &menus {
        let sql = format!(
            "{ignore_sql} ir_menu (id, name, parent_id, web_icon, action_id, sequence, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 10, {now_fn}, {now_fn}){on_conflict}"
        );
        sqlx::query(&dialect.prepare(&sql))
            .bind(id)
            .bind(name)
            .bind(parent_id)
            .bind(icon)
            .bind(action_id)
            .execute(pool)
            .await?;
    }

    tracing::info!("已插入元数据种子数据");
    Ok(())
}

/// 插入插件声明的菜单、视图、动作到 ir_* 表（幂等）
pub async fn seed_plugin_metadata(
    pool: &Pool,
    dialect: &Dialect,
    menus: &[MenuDescriptor],
    views: &[ViewDescriptor],
    actions: &[ActionDescriptor],
) -> Result<()> {
    let (ignore_prefix, now_fn, on_conflict) = match dialect {
        Dialect::Sqlite => ("INSERT OR IGNORE INTO", "datetime('now')", ""),
        _ => ("INSERT INTO", "CURRENT_TIMESTAMP", " ON CONFLICT (id) DO NOTHING"),
    };

    for menu in menus {
        let sql = format!(
            "{ignore_prefix} ir_menu (id, name, parent_id, sequence, action_id, web_icon, active, group_ids, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, {now_fn}, {now_fn}){on_conflict}"
        );
        let group_json = serde_json::to_string(&menu.group_ids)?;
        sqlx::query(&dialect.prepare(&sql))
            .bind(&menu.id)
            .bind(&menu.name)
            .bind(&menu.parent_id)
            .bind(menu.sequence)
            .bind(&menu.action_id)
            .bind(&menu.web_icon)
            .bind(menu.active)
            .bind(&group_json)
            .execute(pool)
            .await?;
    }

    for view in views {
        let sql = format!(
            "{ignore_prefix} ir_view (id, name, model, type, priority, arch, inherit_id, active, group_ids, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, {now_fn}, {now_fn}){on_conflict}"
        );
        let arch_str = serde_json::to_string(&view.arch)?;
        let group_json = serde_json::to_string(&view.group_ids)?;
        sqlx::query(&dialect.prepare(&sql))
            .bind(&view.id)
            .bind(&view.name)
            .bind(&view.model)
            .bind(view.view_type.as_str())
            .bind(view.priority)
            .bind(&arch_str)
            .bind(&view.inherit_id)
            .bind(view.active)
            .bind(&group_json)
            .execute(pool)
            .await?;
    }

    for action in actions {
        let sql = format!(
            "{ignore_prefix} ir_action (id, name, type, res_model, view_mode, view_ids, domain, context, page_limit, target, search_view_id, url, help, group_ids, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, {now_fn}, {now_fn}){on_conflict}"
        );
        let view_mode_str = action.view_mode.iter().map(|vt| vt.as_str()).collect::<Vec<_>>().join(",");
        let view_ids_json = serde_json::to_string(&action.view_ids)?;
        let domain_str = action.domain.as_ref().map(serde_json::to_string).transpose()?;
        let context_str = action.context.as_ref().map(serde_json::to_string).transpose()?;
        let group_json = serde_json::to_string(&action.group_ids)?;
        sqlx::query(&dialect.prepare(&sql))
            .bind(&action.id)
            .bind(&action.name)
            .bind(action.action_type.as_str())
            .bind(&action.res_model)
            .bind(&view_mode_str)
            .bind(&view_ids_json)
            .bind(domain_str.as_deref())
            .bind(context_str.as_deref())
            .bind(action.limit)
            .bind(&action.target)
            .bind(&action.search_view_id)
            .bind(&action.url)
            .bind(&action.help)
            .bind(&group_json)
            .execute(pool)
            .await?;
    }

    if !menus.is_empty() || !views.is_empty() || !actions.is_empty() {
        tracing::info!("已播种插件元数据: {} 菜单, {} 视图, {} 动作", menus.len(), views.len(), actions.len());
    }
    Ok(())
}

/// 校验模型名是否为合法 SQL 标识符（防止注入）
fn is_valid_identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars().next().map(|c| c.is_ascii_alphabetic() || c == '_').unwrap_or(false)
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// 插入插件声明的初始记录到对应模型表（幂等）
pub async fn seed_plugin_records(
    pool: &Pool,
    dialect: &Dialect,
    records: &HashMap<String, Vec<serde_json::Value>>,
) -> Result<()> {
    for (model, rows) in records {
        if !is_valid_identifier(model) {
            anyhow::bail!("无效的模型名 '{}': 只允许字母、数字和下划线", model);
        }
        for row in rows {
            if let Some(obj) = row.as_object() {
                for key in obj.keys() {
                    if !is_valid_identifier(key) {
                        anyhow::bail!("无效的列名 '{}': 只允许字母、数字和下划线", key);
                    }
                }
            }
        }
        for row in rows {
            let obj = row.as_object().ok_or_else(|| anyhow::anyhow!("records.{} 的记录必须是 JSON 对象", model))?;

            let mut columns: Vec<String> = Vec::new();
            let mut placeholders: Vec<String> = Vec::new();
            let mut values: Vec<String> = Vec::new();

            for (key, val) in obj {
                columns.push(key.clone());
                placeholders.push("?".to_string());
                let s = match val {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => if *b { "1" } else { "0" }.to_string(),
                    serde_json::Value::Null => continue,
                    other => serde_json::to_string(other)?,
                };
                values.push(s);
            }

            if columns.is_empty() {
                continue;
            }

            let cols_str = columns.join(", ");
            let vals_str = placeholders.join(", ");

            let sql = match dialect {
                Dialect::Sqlite => format!("INSERT OR IGNORE INTO {model} ({cols_str}) VALUES ({vals_str})"),
                _ => {
                    let conflict_cols =
                        columns.iter().map(|c| format!("{c}=EXCLUDED.{c}")).collect::<Vec<_>>().join(", ");
                    let pks = columns.first().cloned().unwrap_or_default();
                    format!("INSERT INTO {model} ({cols_str}) VALUES ({vals_str}) ON CONFLICT ({pks}) DO UPDATE SET {conflict_cols}")
                }
            };

            let prepared_sql = dialect.prepare(&sql);
            let mut query = sqlx::query(&prepared_sql);
            for val in &values {
                query = query.bind(val);
            }
            query.execute(pool).await?;
        }
        tracing::info!("已播种 {} 条 {} 记录", rows.len(), model);
    }
    Ok(())
}

pub async fn seed_core_data(pool: &Pool, dialect: &Dialect, password_hash: &str) -> Result<()> {
    let (ignore_prefix, now_fn, on_conflict) = match dialect {
        Dialect::Sqlite => ("INSERT OR IGNORE INTO", "datetime('now')", ""),
        _ => ("INSERT INTO", "CURRENT_TIMESTAMP", " ON CONFLICT DO NOTHING"),
    };

    let user_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users").fetch_one(pool).await?;
    if user_count.0 > 0 {
        return Ok(());
    }

    let uid = "admin";
    let sql = format!(
        "{ignore_prefix} users (id, email, name, password_hash, role, created_at, updated_at) \
         VALUES (?, ?, ?, ?, 'admin', {now_fn}, {now_fn}){on_conflict}"
    );
    sqlx::query(&dialect.prepare(&sql))
        .bind(uid)
        .bind("admin@ingjoo.local")
        .bind("admin")
        .bind(password_hash)
        .execute(pool)
        .await?;

    let ug_sql = match dialect {
        Dialect::Sqlite => "INSERT OR IGNORE INTO user_groups (user_id, group_id) VALUES (?, ?)",
        _ => "INSERT INTO user_groups (user_id, group_id) VALUES (?, ?) ON CONFLICT DO NOTHING",
    };
    for gid in &["admin"] {
        sqlx::query(&dialect.prepare(ug_sql)).bind(uid).bind(gid).execute(pool).await?;
    }

    let defs = [
        ("site_name", "string", "莺竹", "general", "站点名称", "网站的显示名称"),
        ("allow_registration", "bool", "true", "auth", "允许注册", "是否开放用户自主注册"),
        ("default_language", "string", "zh-CN", "general", "默认语言", "系统的默认语言"),
    ];
    for (key, typ, default, group, label, desc) in &defs {
        let def_sql = format!(
            "{ignore_prefix} ir_settings_definition (key, type, default_value, group_name, label, description) \
             VALUES (?, ?, ?, ?, ?, ?){on_conflict}"
        );
        sqlx::query(&dialect.prepare(&def_sql))
            .bind(key)
            .bind(typ)
            .bind(default)
            .bind(group)
            .bind(label)
            .bind(desc)
            .execute(pool)
            .await?;
    }

    tracing::info!("已播种核心数据: 默认管理员用户 + 设置定义");

    let si_sql = match dialect {
        Dialect::Sqlite => "INSERT OR IGNORE INTO ir_search_index (model, record_id, content, data) VALUES (?, ?, ?, ?)",
        _ => "INSERT INTO ir_search_index (model, record_id, content, data) VALUES (?, ?, ?, ?) ON CONFLICT (model, record_id) DO NOTHING",
    };
    let search_seeds = [
        ("users", uid, "admin admin@ingjoo.local 管理员", r#"{"name":"admin","email":"admin@ingjoo.local","role":"admin"}"#),
    ];
    for (model, record_id, content, data) in &search_seeds {
        sqlx::query(&dialect.prepare(si_sql))
            .bind(model)
            .bind(record_id)
            .bind(content)
            .bind(data)
            .execute(pool)
            .await?;
    }

    Ok(())
}

