use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct SemanticEngine {
    model: Mutex<TextEmbedding>,
    model_name: String,
}

impl SemanticEngine {
    /// Initialize with auto-detection of model directory and environment endpoint
    pub fn new(custom_model_dir: Option<PathBuf>, use_large_m3: bool) -> Result<Self, Box<dyn std::error::Error>> {

        // Step 2: Determine model storage path (Priority: custom > PHARMRAG_MODELS > ./models/ (if exists) > ~/.cache/pharmrag/models/)
        let model_dir = if let Some(dir) = custom_model_dir {
            dir
        } else if let Ok(env_models) = std::env::var("PHARMRAG_MODELS").or_else(|_| std::env::var("PHARM_RAG_MODELS")) {
            PathBuf::from(env_models)
        } else if std::path::Path::new("./models").exists() {
            PathBuf::from("./models")
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            let legacy_cache = PathBuf::from(&home).join(".cache").join("pharm-rag").join("models");
            if legacy_cache.exists() {
                legacy_cache
            } else {
                let cache_base = std::env::var("XDG_CACHE_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from(home).join(".cache"));
                cache_base.join("pharmrag").join("models")
            }
        };

        let (model_type, name) = if use_large_m3 {
            (EmbeddingModel::BGEM3, "BGE-M3 (Multilingual)")
        } else {
            (EmbeddingModel::BGESmallZHV15, "BGE-Small-ZH-v1.5")
        };

        eprintln!("🧠 Initializing semantic engine: {}...", name);
        eprintln!("📁 Model cache directory: {}", model_dir.display());

        let options = TextInitOptions::new(model_type)
            .with_cache_dir(model_dir)
            .with_show_download_progress(false);

        let model = TextEmbedding::try_new(options)?;

        Ok(Self {
            model: Mutex::new(model),
            model_name: name.to_string(),
        })
    }

    /// Compute vector embedding for a single query text
    pub fn embed_query(&self, text: &str) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let mut model = self.model.lock().map_err(|e| e.to_string())?;
        let embeddings = model.embed(vec![text.to_string()], None)?;
        if let Some(first) = embeddings.into_iter().next() {
            Ok(first)
        } else {
            Err("Empty embedding generated".into())
        }
    }

    /// Compute batch vector embeddings for clauses
    pub fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
        let mut model = self.model.lock().map_err(|e| e.to_string())?;
        let embeddings = model.embed(texts.to_vec(), None)?;
        Ok(embeddings)
    }

    pub fn model_name(&self) -> &str {
        &self.model_name
    }
}

/// Convert Vec<f32> to Little-Endian bytes for SQLite BLOB
pub fn vector_to_bytes(vec: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vec.len() * 4);
    for val in vec {
        bytes.extend_from_slice(&val.to_le_bytes());
    }
    bytes
}

/// Convert Little-Endian bytes back to Vec<f32>
pub fn bytes_to_vector(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
        .collect()
}

/// Compute cosine similarity between two normalized vectors
pub fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
    if v1.len() != v2.len() || v1.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0;
    let mut norm1 = 0.0;
    let mut norm2 = 0.0;
    for i in 0..v1.len() {
        dot += v1[i] * v2[i];
        norm1 += v1[i] * v1[i];
        norm2 += v2[i] * v2[i];
    }
    if norm1 <= 0.0 || norm2 <= 0.0 {
        return 0.0;
    }
    dot / (norm1.sqrt() * norm2.sqrt())
}
