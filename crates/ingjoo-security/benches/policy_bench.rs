use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use ingjoo_core::query::domain::Domain;
use ingjoo_security::{AccessOp, ModelAccess, RecordRule, SecurityPolicy};

fn build_policy() -> SecurityPolicy {
    let mut policy = SecurityPolicy::new();
    for model in &["entry", "collection", "source", "project", "tag"] {
        policy.add_model_access(ModelAccess {
            model: model.to_string(),
            role: "admin".to_string(),
            read: true,
            write: true,
            create: true,
            delete: true,
            import: true,
            export: true,
        });
        policy.add_model_access(ModelAccess {
            model: model.to_string(),
            role: "viewer".to_string(),
            read: true,
            write: false,
            create: false,
            delete: false,
            import: false,
            export: true,
        });
        policy.add_model_access(ModelAccess {
            model: model.to_string(),
            role: "user".to_string(),
            read: true,
            write: true,
            create: true,
            delete: false,
            import: true,
            export: true,
        });
    }
    for model in &["entry", "source"] {
        policy.add_record_rule(RecordRule {
            model: model.to_string(),
            role: "viewer".to_string(),
            domain: Domain::from_json(r#"["status", "=", "published"]"#).unwrap(),
            perm_read: true,
            perm_write: false,
            perm_create: false,
            perm_delete: false,
        });
        policy.add_record_rule(RecordRule {
            model: model.to_string(),
            role: "user".to_string(),
            domain: Domain::from_json(r#"["collection_id", "in", ["c1", "c2"]]"#).unwrap(),
            perm_read: true,
            perm_write: true,
            perm_create: false,
            perm_delete: false,
        });
    }
    policy
}

/// Layer 1: check_access 模型级权限检查
fn bench_check_access(c: &mut Criterion) {
    let policy = build_policy();
    c.bench_function("security_check_access_allowed", |b| {
        b.iter(|| policy.check_access(black_box("entry"), black_box("admin"), AccessOp::Read));
    });
    c.bench_function("security_check_access_denied", |b| {
        b.iter(|| policy.check_access(black_box("entry"), black_box("viewer"), AccessOp::Write));
    });
    c.bench_function("security_check_access_no_rule", |b| {
        b.iter(|| policy.check_access(black_box("nonexistent"), black_box("user"), AccessOp::Read));
    });
}

/// Layer 2: record_filter 记录级过滤
fn bench_record_filter(c: &mut Criterion) {
    let policy = build_policy();
    c.bench_function("security_record_filter_viewer", |b| {
        b.iter(|| policy.record_filter(black_box("entry"), black_box("viewer"), "u1", &AccessOp::Read));
    });
    c.bench_function("security_record_filter_admin", |b| {
        b.iter(|| policy.record_filter(black_box("entry"), black_box("admin"), "u1", &AccessOp::Read));
    });
    c.bench_function("security_record_filter_no_rule", |b| {
        b.iter(|| policy.record_filter(black_box("tag"), black_box("viewer"), "u1", &AccessOp::Read));
    });
}

/// Layer 3: collection_isolation 集合隔离
fn bench_collection_isolation(c: &mut Criterion) {
    let policy = build_policy();
    let ids_1 = vec!["c1".to_string()];
    let ids_10: Vec<String> = (0..10).map(|i| format!("c{}", i)).collect();
    let ids_100: Vec<String> = (0..100).map(|i| format!("c{}", i)).collect();

    let mut group = c.benchmark_group("security_collection_isolation");
    group.bench_function("1_id", |b| {
        b.iter(|| policy.collection_isolation(black_box(&ids_1), None));
    });
    group.bench_function("10_ids", |b| {
        b.iter(|| policy.collection_isolation(black_box(&ids_10), None));
    });
    group.bench_function("100_ids", |b| {
        b.iter(|| policy.collection_isolation(black_box(&ids_100), None));
    });
    group.finish();
}

/// 策略构建性能
fn bench_policy_build(c: &mut Criterion) {
    c.bench_function("security_policy_build", |b| {
        b.iter_batched(
            SecurityPolicy::new,
            |mut policy| {
                for model in &["entry", "collection", "source"] {
                    policy.add_model_access(ModelAccess {
                        model: model.to_string(),
                        role: "admin".to_string(),
                        read: true,
                        write: true,
                        create: true,
                        delete: true,
                        import: true,
                        export: true,
                    });
                }
                policy
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    benches,
    bench_check_access,
    bench_record_filter,
    bench_collection_isolation,
    bench_policy_build,
);
criterion_main!(benches);
