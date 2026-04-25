//! ID 类型定义 — 通过 `define_id!` 宏生成强类型的字符串 ID

/// 定义强类型字符串 ID 的宏
///
/// 生成的类型实现 `Display`、`From<String>`、`From<&str>`、`AsRef<str>`、`Deref<Target=str>`
/// 以及 `sqlx::Type`（transparent）和 `serde` 序列化。
#[macro_export]
macro_rules! define_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, PartialEq, Eq, Hash,
            PartialOrd, Ord,
            ::serde::Serialize, ::serde::Deserialize,
            ::sqlx::Type,
        )]
        #[sqlx(transparent)]
        #[repr(transparent)]
        pub struct $name(pub String);

        impl $name {
            /// 从任意字符串创建 ID
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self { Self(s) }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self { Self(s.to_string()) }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str { &self.0 }
        }

        impl std::ops::Deref for $name {
            type Target = str;
            fn deref(&self) -> &Self::Target { &self.0 }
        }

        impl From<$name> for String {
            fn from(id: $name) -> String { id.0 }
        }
    };
}

// 用户 ID
define_id!(UserId);
// 用户组 ID
define_id!(GroupId);
