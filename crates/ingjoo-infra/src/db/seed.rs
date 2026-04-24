//! 元数据种子数据
//!
//! 首次启动时向 ir_menu / ir_view / ir_action 插入默认记录。
//! 使用 INSERT OR IGNORE (SQLite) / ON CONFLICT DO NOTHING (PG) 保证幂等。

use anyhow::Result;
use ingjoo_core::pool::Pool;
use ingjoo_core::Dialect;

fn upsert_sql(dialect: &Dialect, table: &str, columns: &str, values: &str) -> String {
    match dialect {
        Dialect::Sqlite => format!("INSERT OR IGNORE INTO {table} ({columns}) VALUES ({values})"),
        _ => format!("INSERT INTO {table} ({columns}) VALUES ({values}) ON CONFLICT (id) DO NOTHING"),
    }
}

pub async fn seed_model_access(pool: &Pool, dialect: &Dialect, models: &[&str]) -> Result<()> {
    let groups: &[(&str, (bool, bool, bool, bool, bool, bool))] = &[
        ("admin", (true, true, true, true, true, true)),
        ("user",  (true, true, true, false, true, true)),
        ("viewer",(true, false, false, false, false, true)),
    ];

    for model in models {
        for (group, (pr, pw, pc, pd, pi, pe)) in groups {
            let id = format!("ma_{group}_{model}");
            let sql = upsert_sql(
                dialect,
                "model_access",
                "id, group_id, model, perm_read, perm_write, perm_create, perm_delete, perm_import, perm_export",
                "?, ?, ?, ?, ?, ?, ?, ?, ?",
            );
            sqlx::query(&sql)
                .bind(&id).bind(group).bind(model)
                .bind(*pr).bind(*pw).bind(*pc).bind(*pd).bind(*pi).bind(*pe)
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
        sqlx::query(&sql)
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
        sqlx::query(&sql)
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
        sqlx::query(&sql)
            .bind(id).bind(name).bind(parent_id).bind(icon).bind(action_id)
            .execute(pool).await?;
    }

    tracing::info!("已插入元数据种子数据");
    Ok(())
}
