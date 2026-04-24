use moka::sync::Cache;
use std::time::Duration;

const DEFAULT_TTL_SECS: u64 = 300;
const DEFAULT_MAX_ENTRIES: usize = 1000;

/// 框架级多分区缓存
///
/// 内部分为四个独立的 moka `Cache` 分区：权限访问、用户（按 ID 和按邮箱）、模块设置。
/// 每个分区有独立的 TTL 和容量上限。
pub struct FrameworkCache<M, U = ()> {
    scope_access: Cache<String, M>,
    user_by_id: Cache<String, U>,
    user_by_email: Cache<String, U>,
    effective_setting: Cache<String, String>,
}

impl<M: Clone + Send + Sync + 'static, U: Clone + Send + Sync + 'static> Default for FrameworkCache<M, U> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: Clone + Send + Sync + 'static, U: Clone + Send + Sync + 'static> FrameworkCache<M, U> {
    /// 创建带默认 TTL 和容量上限的缓存实例
    pub fn new() -> Self {
        Self {
            scope_access: Cache::builder()
                .time_to_idle(Duration::from_secs(DEFAULT_TTL_SECS))
                .max_capacity(DEFAULT_MAX_ENTRIES as u64)
                .build(),
            user_by_id: Cache::builder()
                .time_to_idle(Duration::from_secs(DEFAULT_TTL_SECS))
                .max_capacity(500)
                .build(),
            user_by_email: Cache::builder()
                .time_to_idle(Duration::from_secs(DEFAULT_TTL_SECS))
                .max_capacity(500)
                .build(),
            effective_setting: Cache::builder()
                .time_to_idle(Duration::from_secs(60))
                .max_capacity(2000)
                .build(),
        }
    }

    /// 查询权限缓存
    pub fn get_scope_access(&self, scope_id: &str, user_id: &str) -> Option<M> {
        self.scope_access.get(&cache_key(scope_id, user_id))
    }

    /// 写入权限缓存
    pub fn put_scope_access(&self, scope_id: &str, user_id: &str, member: M) {
        self.scope_access.insert(cache_key(scope_id, user_id), member);
    }

    /// 失效指定 scope+user 的权限缓存
    pub fn invalidate_scope_access(&self, scope_id: &str, user_id: &str) {
        self.scope_access.invalidate(&cache_key(scope_id, user_id));
    }

    /// 失效整个 scope 下所有用户的权限缓存
    pub fn invalidate_scope(&self, _scope_id: &str) {
        self.scope_access.invalidate_all();
        self.scope_access.run_pending_tasks();
    }

    /// 按 ID 查询用户缓存
    pub fn get_user(&self, user_id: &str) -> Option<U> {
        self.user_by_id.get(user_id)
    }

    /// 同时写入 ID 和邮箱两个索引
    pub fn put_user(&self, id_key: &str, email_key: &str, user: U) {
        self.user_by_id.insert(id_key.to_string(), user.clone());
        self.user_by_email.insert(email_key.to_string(), user);
    }

    /// 按 ID 失效用户缓存
    pub fn invalidate_user(&self, user_id: &str) {
        self.user_by_id.invalidate(user_id);
    }

    /// 按邮箱查询用户缓存
    pub fn get_user_by_email(&self, email: &str) -> Option<U> {
        self.user_by_email.get(email)
    }

    /// 查询模块设置的生效值
    pub fn get_effective_setting(&self, module: &str, key: &str, scope_id: Option<&str>) -> Option<String> {
        self.effective_setting.get(&setting_cache_key(module, key, scope_id))
    }

    /// 写入模块设置缓存
    pub fn put_effective_setting(&self, module: &str, key: &str, scope_id: Option<&str>, value: String) {
        self.effective_setting.insert(setting_cache_key(module, key, scope_id), value);
    }

    /// 清空所有模块设置缓存
    pub fn invalidate_settings(&self) {
        self.effective_setting.invalidate_all();
    }

    /// 返回各分区的条目计数
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            scope_access_entries: self.scope_access.entry_count(),
            user_entries: self.user_by_id.entry_count(),
            setting_entries: self.effective_setting.entry_count(),
        }
    }
}

/// 各分区缓存条目计数
pub struct CacheStats {
    pub scope_access_entries: u64,
    pub user_entries: u64,
    pub setting_entries: u64,
}

fn cache_key(a: &str, b: &str) -> String {
    format!("{}:{}", a, b)
}

fn setting_cache_key(module: &str, key: &str, scope_id: Option<&str>) -> String {
    match scope_id {
        Some(id) => format!("{}:{}:{}", module, key, id),
        None => format!("{}:{}:", module, key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug)]
    struct TestUser {
        id: String,
        email: String,
    }

    fn test_user(id: &str, email: &str) -> TestUser {
        TestUser { id: id.to_string(), email: email.to_string() }
    }

    #[derive(Clone, Debug)]
    struct TestMember {
        #[allow(dead_code)]
        scope_id: String,
        #[allow(dead_code)]
        user_id: String,
        role: String,
    }

    fn test_member(scope: &str, uid: &str, role: &str) -> TestMember {
        TestMember { scope_id: scope.to_string(), user_id: uid.to_string(), role: role.to_string() }
    }

    #[test]
    fn scope_access_cache_round_trip() {
        let cache: FrameworkCache<TestMember, TestUser> = FrameworkCache::new();
        assert!(cache.get_scope_access("col1", "u1").is_none());

        cache.put_scope_access("col1", "u1", test_member("col1", "u1", "admin"));
        let m = cache.get_scope_access("col1", "u1").unwrap();
        assert_eq!(m.role, "admin");

        cache.invalidate_scope_access("col1", "u1");
        assert!(cache.get_scope_access("col1", "u1").is_none());
    }

    #[test]
    fn invalidate_scope_clears_all_users() {
        let cache: FrameworkCache<TestMember, TestUser> = FrameworkCache::new();
        cache.put_scope_access("col1", "u1", test_member("col1", "u1", "admin"));
        cache.put_scope_access("col1", "u2", test_member("col1", "u2", "viewer"));

        cache.invalidate_scope("col1");

        assert!(cache.get_scope_access("col1", "u1").is_none());
        assert!(cache.get_scope_access("col1", "u2").is_none());
    }

    #[test]
    fn user_cache_by_id_and_email() {
        let cache: FrameworkCache<TestMember, TestUser> = FrameworkCache::new();
        let user = test_user("u1", "a@b.com");

        cache.put_user("u1", "a@b.com", user);

        assert_eq!(cache.get_user("u1").unwrap().email, "a@b.com");
        assert_eq!(cache.get_user_by_email("a@b.com").unwrap().id, "u1");

        cache.invalidate_user("u1");
        assert!(cache.get_user("u1").is_none());
    }

    #[test]
    fn setting_cache_round_trip() {
        let cache: FrameworkCache<TestMember, TestUser> = FrameworkCache::new();

        assert!(cache.get_effective_setting("wiki", "theme", None).is_none());

        cache.put_effective_setting("wiki", "theme", None, "dark".to_string());
        assert_eq!(cache.get_effective_setting("wiki", "theme", None).unwrap(), "dark");

        cache.put_effective_setting("wiki", "theme", Some("col1"), "light".to_string());
        assert_eq!(cache.get_effective_setting("wiki", "theme", Some("col1")).unwrap(), "light");
        assert_eq!(cache.get_effective_setting("wiki", "theme", None).unwrap(), "dark");

        cache.invalidate_settings();
        assert!(cache.get_effective_setting("wiki", "theme", None).is_none());
    }

    #[test]
    fn cache_stats() {
        let cache: FrameworkCache<TestMember, TestUser> = FrameworkCache::new();
        cache.put_user("u1", "a@b.com", test_user("u1", "a@b.com"));
        cache.put_scope_access("c1", "u1", test_member("c1", "u1", "admin"));
        cache.put_effective_setting("m", "k", None, "v".to_string());

        cache.scope_access.run_pending_tasks();
        cache.user_by_id.run_pending_tasks();
        cache.effective_setting.run_pending_tasks();

        let stats = cache.stats();
        assert!(stats.user_entries >= 1);
        assert!(stats.scope_access_entries >= 1);
        assert!(stats.setting_entries >= 1);
    }
}
