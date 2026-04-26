//! 文档加载器 — 从文件系统加载文本文档

use async_trait::async_trait;
use ingjoo_core::extension::document::{Document, DocumentLoader};
use std::path::Path;

/// 文件系统文档加载器
///
/// 支持的文件格式：`.txt`, `.md`, `.json`, `.csv`
/// source 参数为目录路径，递归扫描所有支持的文件
pub struct FsDocumentLoader {
    max_file_size: u64,
}

impl FsDocumentLoader {
    pub fn new(max_file_size: u64) -> Self {
        Self { max_file_size }
    }

    pub fn with_defaults() -> Self {
        Self::new(10 * 1024 * 1024) // 10MB
    }

    fn is_supported(path: &Path) -> bool {
        path.extension().and_then(|e| e.to_str()).map(|e| matches!(e, "txt" | "md" | "json" | "csv")).unwrap_or(false)
    }
}

impl Default for FsDocumentLoader {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[async_trait]
impl DocumentLoader for FsDocumentLoader {
    async fn load(&self, source: &str) -> Result<Vec<Document>, anyhow::Error> {
        let path = Path::new(source);
        if !path.exists() {
            anyhow::bail!("路径不存在: {}", source);
        }

        if path.is_file() {
            return self.load_file(path).map(|d| vec![d]);
        }

        let mut documents = Vec::new();
        self.load_dir(path, &mut documents)?;
        Ok(documents)
    }
}

impl FsDocumentLoader {
    fn load_dir(&self, dir: &Path, documents: &mut Vec<Document>) -> Result<(), anyhow::Error> {
        let entries = std::fs::read_dir(dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.load_dir(&path, documents)?;
            } else if Self::is_supported(&path) {
                match self.load_file(&path) {
                    Ok(doc) => documents.push(doc),
                    Err(e) => {
                        tracing::warn!("跳过文件 {:?}: {}", path, e);
                    }
                }
            }
        }
        Ok(())
    }

    fn load_file(&self, path: &Path) -> Result<Document, anyhow::Error> {
        let metadata = std::fs::metadata(path)?;
        if metadata.len() > self.max_file_size {
            anyhow::bail!("文件过大: {} bytes (上限: {} bytes)", metadata.len(), self.max_file_size);
        }

        let content = std::fs::read_to_string(path)?;
        let id = path.to_str().ok_or_else(|| anyhow::anyhow!("无效路径编码"))?.to_string();

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_string();

        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();

        Ok(Document {
            id,
            content,
            metadata: serde_json::json!({
                "format": ext,
                "file_name": file_name,
                "size": metadata.len(),
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn default_constructs() {
        let loader = FsDocumentLoader::default();
        assert_eq!(loader.max_file_size, 10 * 1024 * 1024);
    }

    #[tokio::test]
    async fn load_single_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let mut f = std::fs::File::create(&file_path).unwrap();
        write!(f, "hello world").unwrap();

        let loader = FsDocumentLoader::with_defaults();
        let docs = loader.load(file_path.to_str().unwrap()).await.unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].content, "hello world");
        assert_eq!(docs[0].metadata["format"], "txt");
    }

    #[tokio::test]
    async fn load_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "content a").unwrap();
        std::fs::write(dir.path().join("b.md"), "content b").unwrap();
        std::fs::write(dir.path().join("c.bin"), "binary").unwrap();

        let loader = FsDocumentLoader::with_defaults();
        let docs = loader.load(dir.path().to_str().unwrap()).await.unwrap();
        assert_eq!(docs.len(), 2);
        let names: Vec<&str> = docs.iter().map(|d| d.metadata["file_name"].as_str().unwrap()).collect();
        assert!(names.contains(&"a.txt"));
        assert!(names.contains(&"b.md"));
        assert!(!names.contains(&"c.bin"));
    }

    #[tokio::test]
    async fn nonexistent_path_fails() {
        let loader = FsDocumentLoader::with_defaults();
        let result = loader.load("/nonexistent/path").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn max_file_size_enforced() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("big.txt");
        std::fs::write(&file_path, "x".repeat(100)).unwrap();

        let loader = FsDocumentLoader::new(10);
        let result = loader.load(file_path.to_str().unwrap()).await;
        assert!(result.is_err());
    }
}
