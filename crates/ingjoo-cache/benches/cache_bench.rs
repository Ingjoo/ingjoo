use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ingjoo_cache::moka_cache::FrameworkCache;

/// 测量用简单类型 — 满足 M: Clone + Send + Sync + 'static
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct BenchMember {
    role: String,
}

/// 测量用简单类型 — 满足 U: Clone + Send + Sync + 'static
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct BenchUser {
    id: String,
    email: String,
}

/// Put + Get 单次往返
fn bench_cache_put_get(c: &mut Criterion) {
    let cache: FrameworkCache<BenchMember, BenchUser> = FrameworkCache::new();

    c.bench_function("cache_put_get_scope_access", |b| {
        b.iter(|| {
            cache.put_scope_access("scope1", "user1", BenchMember { role: "admin".to_string() });
            let _ = black_box(cache.get_scope_access("scope1", "user1"));
        })
    });
}

/// 预填充 1000 条后批量读取命中率
fn bench_cache_hit(c: &mut Criterion) {
    let cache: FrameworkCache<BenchMember, BenchUser> = FrameworkCache::new();
    for i in 0..1000 {
        cache.put_scope_access("scope", &format!("user_{}", i), BenchMember { role: format!("role_{}", i) });
    }

    c.bench_function("cache_hit_1000_entries", |b| {
        b.iter(|| {
            for i in 0..100 {
                let _ = black_box(cache.get_scope_access("scope", &format!("user_{}", i)));
            }
        })
    });
}

/// 批量 invalidate
fn bench_cache_invalidate(c: &mut Criterion) {
    c.bench_function("cache_invalidate_scope_all", |b| {
        b.iter_batched(
            || {
                let cache: FrameworkCache<BenchMember, BenchUser> = FrameworkCache::new();
                for i in 0..100 {
                    cache.put_scope_access("scope", &format!("user_{}", i), BenchMember { role: "reader".to_string() });
                }
                cache
            },
            |cache| {
                cache.invalidate_scope("scope");
                cache
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

/// 用户缓存: put + get_by_id + get_by_email
fn bench_cache_user(c: &mut Criterion) {
    let cache: FrameworkCache<BenchMember, BenchUser> = FrameworkCache::new();

    c.bench_function("cache_put_get_user", |b| {
        b.iter(|| {
            cache.put_user("u1", "u1@test.com", BenchUser { id: "u1".to_string(), email: "u1@test.com".to_string() });
            let _ = black_box(cache.get_user("u1"));
            let _ = black_box(cache.get_user_by_email("u1@test.com"));
        })
    });
}

criterion_group!(benches, bench_cache_put_get, bench_cache_hit, bench_cache_invalidate, bench_cache_user,);
criterion_main!(benches);
