use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use ingjoo_core::module::registry::{FieldType, ModelDescriptor, ModelRegistry};

/// 构造含 field_count 个字段的 ModelDescriptor（使用 builder 模式）
fn make_model(name: &str, field_count: usize) -> ModelDescriptor {
    let mut model = ModelDescriptor::new(name, &format!("t_{}", name));
    for i in 0..field_count {
        let field_name = format!("field_{}", i);
        if i == 0 {
            model = model.required_field(&field_name, FieldType::Text);
        } else {
            model = model.field(&field_name, if i % 2 == 0 { FieldType::Text } else { FieldType::Integer });
        }
    }
    model
}

/// 注册单个模型
fn bench_registry_register(c: &mut Criterion) {
    c.bench_function("registry_register_model", |b| {
        b.iter_batched(
            || (ModelRegistry::new(), make_model("bench_model", 5)),
            |(reg, model)| {
                reg.register(model);
                reg
            },
            BatchSize::SmallInput,
        )
    });
}

/// 在 100 个模型中按名查找
fn bench_registry_get(c: &mut Criterion) {
    let reg = ModelRegistry::new();
    for i in 0..100 {
        reg.register(make_model(&format!("model_{}", i), 5));
    }

    c.bench_function("registry_get_model", |b| b.iter(|| black_box(&reg).get("model_50")));
}

/// 列出全部 100 个模型
fn bench_registry_list(c: &mut Criterion) {
    let reg = ModelRegistry::new();
    for i in 0..100 {
        reg.register(make_model(&format!("model_{}", i), 5));
    }

    c.bench_function("registry_list_100_models", |b| b.iter(|| black_box(&reg).list()));
}

/// 注销模型
fn bench_registry_unregister(c: &mut Criterion) {
    c.bench_function("registry_unregister_model", |b| {
        b.iter_batched(
            || {
                let reg = ModelRegistry::new();
                reg.register(make_model("to_remove", 5));
                reg
            },
            |reg| {
                black_box(&reg).unregister("to_remove");
                reg
            },
            BatchSize::SmallInput,
        )
    });
}

/// 1000 模型注册 + 查找
fn bench_registry_large(c: &mut Criterion) {
    c.bench_function("registry_large_1000_models", |b| {
        b.iter_batched(
            ModelRegistry::new,
            |reg| {
                for i in 0..1000 {
                    reg.register(make_model(&format!("large_model_{}", i), 5));
                }
                reg.get("large_model_500");
                reg
            },
            BatchSize::SmallInput,
        )
    });
}

/// 50+ 字段的大模型
fn bench_registry_large_model(c: &mut Criterion) {
    c.bench_function("registry_large_model_50_fields", |b| {
        b.iter_batched(
            ModelRegistry::new,
            |reg| {
                reg.register(make_model("big_model", 50));
                reg.get("big_model");
                reg
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    benches,
    bench_registry_register,
    bench_registry_get,
    bench_registry_list,
    bench_registry_unregister,
    bench_registry_large,
    bench_registry_large_model,
);
criterion_main!(benches);
