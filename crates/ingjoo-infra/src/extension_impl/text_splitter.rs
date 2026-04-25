//! 文档分割器 — 字符级滑动窗口分割

use ingjoo_core::extension::document::{Document, TextSplitter};

/// 字符级文档分割器，使用滑动窗口策略将长文档拆分为固定大小的片段
pub struct CharTextSplitter {
    chunk_size: usize,
    chunk_overlap: usize,
}

impl CharTextSplitter {
    pub fn new(chunk_size: usize, chunk_overlap: usize) -> Self {
        Self {
            chunk_size,
            chunk_overlap,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(1000, 200)
    }
}

impl Default for CharTextSplitter {
    fn default() -> Self {
        Self::with_defaults()
    }
}

impl TextSplitter for CharTextSplitter {
    fn split(&self, documents: &[Document]) -> Vec<Document> {
        let mut result = Vec::new();

        for doc in documents {
            let content = &doc.content;
            if content.len() <= self.chunk_size {
                result.push(doc.clone());
                continue;
            }

            let step = self.chunk_size.saturating_sub(self.chunk_overlap);
            if step == 0 {
                result.push(doc.clone());
                continue;
            }

            let mut start = 0;
            let mut idx = 0;
            while start < content.len() {
                let end = (start + self.chunk_size).min(content.len());
                let chunk = &content[start..end];

                let mut metadata = doc.metadata.clone();
                if let Some(obj) = metadata.as_object_mut() {
                    obj.insert(
                        "chunk_index".to_string(),
                        serde_json::json!(idx),
                    );
                    obj.insert(
                        "parent_id".to_string(),
                        serde_json::json!(doc.id),
                    );
                }

                result.push(Document {
                    id: format!("{}#{}", doc.id, idx),
                    content: chunk.to_string(),
                    metadata,
                });

                start += step;
                idx += 1;
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_doc(content: &str) -> Document {
        Document {
            id: "test".to_string(),
            content: content.to_string(),
            metadata: serde_json::json!({}),
        }
    }

    #[test]
    fn short_document_unchanged() {
        let splitter = CharTextSplitter::new(100, 20);
        let doc = make_doc("hello world");
        let result = splitter.split(&[doc.clone()]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "hello world");
        assert_eq!(result[0].id, "test");
    }

    #[test]
    fn long_document_split_into_chunks() {
        let splitter = CharTextSplitter::new(10, 3);
        let doc = make_doc("abcdefghijklmnopqrstuvwxyz");
        let result = splitter.split(&[doc]);
        assert!(result.len() > 1);
        assert_eq!(result[0].content.len(), 10);
        for chunk in &result {
            assert!(chunk.content.len() <= 10);
        }
    }

    #[test]
    fn chunk_overlap() {
        let splitter = CharTextSplitter::new(10, 3);
        let doc = make_doc("abcdefghijklmnopqrstuvwxyz");
        let result = splitter.split(&[doc]);
        if result.len() >= 2 {
            let overlap = &result[0].content[7..];
            assert!(result[1].content.starts_with(overlap));
        }
    }

    #[test]
    fn chunk_metadata_includes_index() {
        let splitter = CharTextSplitter::new(5, 0);
        let doc = make_doc("abcdefghij");
        let result = splitter.split(&[doc]);
        assert!(result.len() >= 2);
        assert_eq!(result[0].metadata["chunk_index"], 0);
        assert_eq!(result[1].metadata["chunk_index"], 1);
        assert_eq!(result[0].metadata["parent_id"], "test");
    }

    #[test]
    fn empty_documents() {
        let splitter = CharTextSplitter::new(100, 20);
        let result = splitter.split(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn default_splitter() {
        let splitter = CharTextSplitter::default();
        assert_eq!(splitter.chunk_size, 1000);
        assert_eq!(splitter.chunk_overlap, 200);
    }
}
