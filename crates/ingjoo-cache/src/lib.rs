//! 莺竹框架缓存模块
//!
//! 基于 [moka](https://docs.rs/moka) 的同步缓存实现，
//! 提供权限、用户、设置等热点数据的 TTL 缓存。

pub mod moka_cache;

pub use moka_cache::{CacheStats, FrameworkCache};
