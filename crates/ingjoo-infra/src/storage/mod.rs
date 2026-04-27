use anyhow::Result;
use async_trait::async_trait;

/// 文件元数据
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: String,
    pub size: u64,
    pub mime_type: String,
}

/// 文件存储 trait，支持本地和 S3 等后端
#[async_trait]
pub trait FileStorage: Send + Sync {
    async fn save(&self, path: &str, data: &[u8]) -> Result<()>;
    async fn load(&self, path: &str) -> Result<Vec<u8>>;
    async fn delete(&self, path: &str) -> Result<()>;
    async fn exists(&self, path: &str) -> bool;
    async fn size(&self, path: &str) -> Result<u64>;
}

/// 本地文件系统存储
pub struct LocalStorage {
    base_dir: std::path::PathBuf,
}

impl LocalStorage {
    /// 创建本地存储，以 base_dir 为根目录
    pub fn new(base_dir: std::path::PathBuf) -> Self {
        Self { base_dir }
    }

    fn full_path(&self, relative: &str) -> std::path::PathBuf {
        self.base_dir.join(relative)
    }
}

#[async_trait]
impl FileStorage for LocalStorage {
    async fn save(&self, path: &str, data: &[u8]) -> Result<()> {
        let full = self.full_path(path);
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&full, data).await?;
        Ok(())
    }

    async fn load(&self, path: &str) -> Result<Vec<u8>> {
        let full = self.full_path(path);
        let data = tokio::fs::read(&full).await?;
        Ok(data)
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let full = self.full_path(path);
        if full.exists() {
            tokio::fs::remove_file(full).await?;
        }
        Ok(())
    }

    async fn exists(&self, path: &str) -> bool {
        self.full_path(path).exists()
    }

    async fn size(&self, path: &str) -> Result<u64> {
        let full = self.full_path(path);
        let meta = tokio::fs::metadata(full).await?;
        Ok(meta.len())
    }
}

#[cfg(feature = "s3")]
pub struct S3Storage {
    client: aws_sdk_s3::Client,
    bucket: String,
}

#[cfg(feature = "s3")]
impl S3Storage {
    pub async fn new(bucket: String) -> Result<Self> {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = aws_sdk_s3::Client::new(&config);
        Ok(Self { client, bucket })
    }

    pub async fn from_endpoint(endpoint: String, bucket: String, region: String) -> Result<Self> {
        use aws_sdk_s3::config::Region;
        let config = aws_config::from_env().region(Region::new(region)).endpoint_url(endpoint).load().await;
        let client = aws_sdk_s3::Client::new(&config);
        Ok(Self { client, bucket })
    }
}

#[cfg(feature = "s3")]
#[async_trait]
impl FileStorage for S3Storage {
    async fn save(&self, path: &str, data: &[u8]) -> Result<()> {
        self.client.put_object().bucket(&self.bucket).key(path).body(data.to_vec().into()).send().await?;
        Ok(())
    }

    async fn load(&self, path: &str) -> Result<Vec<u8>> {
        let resp = self.client.get_object().bucket(&self.bucket).key(path).send().await?;
        let bytes = resp.body.collect().await?.to_vec();
        Ok(bytes)
    }

    async fn delete(&self, path: &str) -> Result<()> {
        self.client.delete_object().bucket(&self.bucket).key(path).send().await?;
        Ok(())
    }

    async fn exists(&self, path: &str) -> bool {
        self.client.head_object().bucket(&self.bucket).key(path).send().await.is_ok()
    }

    async fn size(&self, path: &str) -> Result<u64> {
        let resp = self.client.head_object().bucket(&self.bucket).key(path).send().await?;
        Ok(resp.content_length().unwrap_or(0) as u64)
    }
}
