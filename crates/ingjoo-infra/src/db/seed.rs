//! 元数据种子数据
//!
//! 首次启动时向 ir_menu / ir_view / ir_action 插入默认记录。
//! 使用 INSERT OR IGNORE (SQLite) / ON CONFLICT DO NOTHING (PG) 保证幂等。

use anyhow::Result;
use ingjoo_core::pool::Pool;
use ingjoo_core::Dialect;
use ingjoo_core::module::{MenuDescriptor, ViewDescriptor, ActionDescriptor};
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
        ("user",  (true, true, true, false, true, true)),
        ("viewer",(true, false, false, false, false, true)),
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
                .bind(&id).bind(group).bind(model)
                .bind(*pr as i32).bind(*pw as i32).bind(*pc as i32).bind(*pd as i32).bind(*pi as i32).bind(*pe as i32)
                .execute(pool).await?;
        }
    }

    Ok(())
}

/// 插入默认元数据种子（幂等）
pub async fn seed_metadata(pool: &Pool, dialect: &Dialect) -> Result<()> {
    // 检查是否已有数据
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM ir_menu")
        .fetch_one(pool)
        .await?;
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
        ("action_demo_articles", "文章管理", "act_window", "article", "list,form", r#"["view_article_list","view_article_form"]"#),
    ];

    for (id, name, atype, res_model, view_mode, view_ids) in &actions {
        let sql = format!(
            "{ignore_sql} ir_action (id, name, type, res_model, view_mode, view_ids, target, page_limit, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, 'current', 40, {now_fn}, {now_fn}){on_conflict}"
        );
        sqlx::query(&dialect.prepare(&sql))
            .bind(id).bind(name).bind(atype).bind(res_model).bind(view_mode).bind(view_ids)
            .execute(pool).await?;
    }

    // ── Views ──
    let views = vec![
        ("view_article_list", "文章列表", "article", "list", r#"{"columns":[{"name":"title","sortable":true},{"name":"status","widget":"badge"},{"name":"views","sortable":true,"align":"right"}]}"#),
        ("view_article_form", "文章表单", "article", "form", r#"{"layout":"grouped","groups":[{"title":"基本信息","columns":2,"fields":[{"name":"title","widget":"text","placeholder":"输入标题"},{"name":"status","widget":"select","options":["draft","published","archived"]},{"name":"body","widget":"richtext","colspan":2}]}]}"#),
    ];

    for (id, name, model, vtype, arch) in &views {
        let sql = format!(
            "{ignore_sql} ir_view (id, name, model, type, arch, priority, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 16, {now_fn}, {now_fn}){on_conflict}"
        );
        sqlx::query(&dialect.prepare(&sql))
            .bind(id).bind(name).bind(model).bind(vtype).bind(arch)
            .execute(pool).await?;
    }

    // ── Menus ──
    let menus = vec![
        ("menu_demo", "演示", None::<&str>, "fa fa-flask", Some("action_demo_articles")),
    ];

    for (id, name, parent_id, icon, action_id) in &menus {
        let sql = format!(
            "{ignore_sql} ir_menu (id, name, parent_id, web_icon, action_id, sequence, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 10, {now_fn}, {now_fn}){on_conflict}"
        );
        sqlx::query(&dialect.prepare(&sql))
            .bind(id).bind(name).bind(parent_id).bind(icon).bind(action_id)
            .execute(pool).await?;
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
        let view_mode_str = action.view_mode.iter()
            .map(|vt| vt.as_str())
            .collect::<Vec<_>>()
            .join(",");
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
        tracing::info!(
            "已播种插件元数据: {} 菜单, {} 视图, {} 动作",
            menus.len(), views.len(), actions.len()
        );
    }
    Ok(())
}

/// 插入插件声明的初始记录到对应模型表（幂等）
pub async fn seed_plugin_records(
    pool: &Pool,
    dialect: &Dialect,
    records: &HashMap<String, Vec<serde_json::Value>>,
) -> Result<()> {
    for (model, rows) in records {
        for row in rows {
            let obj = row.as_object()
                .ok_or_else(|| anyhow::anyhow!("records.{} 的记录必须是 JSON 对象", model))?;

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
                    let conflict_cols = columns.iter()
                        .map(|c| format!("{c}=EXCLUDED.{c}"))
                        .collect::<Vec<_>>()
                        .join(", ");
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
