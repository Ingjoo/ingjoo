use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ingjoo_core::query::domain::Domain;
use ingjoo_core::Dialect;
use serde_json::json;

/// Domain::parse 性能基准
fn bench_domain_parse(c: &mut Criterion) {
    let simple = json!(["status", "=", "published"]);
    let and_cond = json!(["&", ["status", "=", "published"], ["collection_id", "in", ["id1", "id2"]]]);
    // 3 层嵌套: OR( AND(status=published, type=article), NOT(archived=true) )
    let nested =
        json!(["|", ["&", ["status", "=", "published"], ["type", "=", "article"]], ["!", ["archived", "=", true]]]);
    // 大 IN 列表: 50 个值
    let large_in: Vec<serde_json::Value> = (1..=50).map(|i| json!(format!("v{}", i))).collect();
    let large_in_domain = json!(["id", "in", large_in]);

    let mut group = c.benchmark_group("domain_parse");
    group.bench_function("simple_leaf", |b| b.iter(|| Domain::parse(black_box(&simple)).unwrap()));
    group.bench_function("and_conditions", |b| b.iter(|| Domain::parse(black_box(&and_cond)).unwrap()));
    group.bench_function("nested_complex", |b| b.iter(|| Domain::parse(black_box(&nested)).unwrap()));
    group.bench_function("in_list_large_50", |b| b.iter(|| Domain::parse(black_box(&large_in_domain)).unwrap()));
    group.finish();
}

/// Domain::to_sql 性能基准
fn bench_domain_to_sql(c: &mut Criterion) {
    let simple = Domain::parse(&json!(["status", "=", "published"])).unwrap();
    let and_cond =
        Domain::parse(&json!(["&", ["status", "=", "published"], ["collection_id", "in", ["id1", "id2"]]])).unwrap();
    let nested = Domain::parse(&json!([
        "|",
        ["&", ["status", "=", "published"], ["type", "=", "article"]],
        ["!", ["archived", "=", true]]
    ]))
    .unwrap();
    let large_in: Vec<serde_json::Value> = (1..=50).map(|i| json!(format!("v{}", i))).collect();
    let large_domain = Domain::parse(&json!(["id", "in", large_in])).unwrap();

    let mut group = c.benchmark_group("domain_to_sql");
    group.bench_function("simple_leaf", |b| b.iter(|| black_box(&simple).to_sql(None)));
    group.bench_function("and_conditions", |b| b.iter(|| black_box(&and_cond).to_sql(None)));
    group.bench_function("nested_complex", |b| b.iter(|| black_box(&nested).to_sql(None)));
    group.bench_function("in_list_large_50", |b| b.iter(|| black_box(&large_domain).to_sql(None)));
    // 带别名前缀
    group.bench_function("and_with_alias", |b| b.iter(|| black_box(&and_cond).to_sql(Some("t"))));
    group.finish();
}

/// 端到端: parse + compile
fn bench_domain_end_to_end(c: &mut Criterion) {
    let input = json!(["&", ["status", "=", "published"], ["collection_id", "in", ["id1", "id2"]]]);

    c.bench_function("domain_parse_and_compile", |b| {
        b.iter(|| {
            let domain = Domain::parse(black_box(&input)).unwrap();
            black_box(&domain).to_sql(None);
        })
    });
}

/// Domain::from_json 性能基准
fn bench_domain_from_json(c: &mut Criterion) {
    let simple = r#"["status", "=", "published"]"#;
    let and_cond = r#"["&", ["status", "=", "published"], ["collection_id", "in", ["id1", "id2"]]]"#;
    let nested =
        r#"["|", ["&", ["status", "=", "published"], ["type", "=", "article"]], ["!", ["archived", "=", true]]]"#;

    let mut group = c.benchmark_group("domain_from_json");
    group.bench_function("simple_leaf", |b| b.iter(|| Domain::from_json(black_box(simple)).unwrap()));
    group.bench_function("and_conditions", |b| b.iter(|| Domain::from_json(black_box(and_cond)).unwrap()));
    group.bench_function("nested_complex", |b| b.iter(|| Domain::from_json(black_box(nested)).unwrap()));
    group.finish();
}

/// Domain::all() / Domain::any() 组合性能基准
fn bench_domain_compose(c: &mut Criterion) {
    let leaves: Vec<Domain> = (0..10)
        .map(|i| {
            let field = format!("field_{}", i);
            Domain::parse(&json!([field, "=", "value"])).unwrap()
        })
        .collect();

    let mut group = c.benchmark_group("domain_compose");
    group.bench_function("all_10_leaves", |b| {
        b.iter(|| Domain::all(black_box(leaves.clone())));
    });
    group.bench_function("any_10_leaves", |b| {
        b.iter(|| Domain::any(black_box(leaves.clone())));
    });
    group.finish();
}

/// 5+ 层深度嵌套 Domain 性能基准
fn bench_domain_deeply_nested(c: &mut Criterion) {
    let input = json!([
        "&",
        [
            "|",
            ["&", ["|", ["a", "=", "1"], ["b", "=", "2"]], ["&", ["c", "=", "3"], ["d", "=", "4"]]],
            ["!", ["&", ["|", ["e", "=", "5"], ["f", "=", "6"]], ["g", "=", "7"]]]
        ],
        ["|", ["&", ["h", "=", "8"], ["i", "=", "9"]], ["j", "=", "10"]]
    ]);

    c.bench_function("domain_deeply_nested_5_levels", |b| {
        let domain = Domain::parse(black_box(&input)).unwrap();
        b.iter(|| black_box(&domain).to_sql(None));
    });
}

/// 50+ OR 叶节点 Domain 性能基准
fn bench_domain_large_or(c: &mut Criterion) {
    let parsed_leaves: Vec<Domain> = (0..50)
        .map(|i| {
            let field = format!("f{}", i);
            Domain::parse(&json!([field, "=", "v"])).unwrap()
        })
        .collect();
    let large_or = Domain::Or(parsed_leaves);

    c.bench_function("domain_large_or_50_leaves", |b| {
        b.iter(|| black_box(&large_or).to_sql(None));
    });
}

/// Postgres 方言 SQL 编译性能基准
fn bench_domain_to_sql_postgres(c: &mut Criterion) {
    let simple = Domain::parse(&json!(["status", "=", "published"])).unwrap();
    let and_cond =
        Domain::parse(&json!(["&", ["status", "=", "published"], ["collection_id", "in", ["id1", "id2"]]])).unwrap();
    let nested = Domain::parse(&json!([
        "|",
        ["&", ["status", "=", "published"], ["type", "=", "article"]],
        ["!", ["archived", "=", true]]
    ]))
    .unwrap();

    let mut group = c.benchmark_group("domain_to_sql_postgres");
    group.bench_function("simple_leaf", |b| {
        b.iter(|| black_box(&simple).to_sql_with_dialect(None, Some(&Dialect::Postgres)));
    });
    group.bench_function("and_conditions", |b| {
        b.iter(|| black_box(&and_cond).to_sql_with_dialect(None, Some(&Dialect::Postgres)));
    });
    group.bench_function("nested_complex", |b| {
        b.iter(|| black_box(&nested).to_sql_with_dialect(None, Some(&Dialect::Postgres)));
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_domain_parse,
    bench_domain_to_sql,
    bench_domain_end_to_end,
    bench_domain_from_json,
    bench_domain_compose,
    bench_domain_deeply_nested,
    bench_domain_large_or,
    bench_domain_to_sql_postgres,
);
criterion_main!(benches);
