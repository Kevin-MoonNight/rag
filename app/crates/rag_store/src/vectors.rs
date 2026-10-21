//! Qdrant Edge：兩個 shard。
//!
//! - 文字的 shard：`text`（文字 chunk 的向量）和 `kw`（關鍵字檢索用的稀疏向量，
//!   詞的稀有程度 IDF 由資料庫在查詢時計算）。
//! - 圖片的 shard：`image`（頁面圖片或圖片檔的向量）。
//!
//! 分成兩個是因為同一個 shard 裡每一筆資料都會替每一種向量留位置：文字和圖片放一起時，
//! 每個文字 chunk 會多佔一份空的圖片向量（磁碟 3 KB、記憶體 0.8 KB），反過來也一樣。
//!
//! 完整的 float32 向量放磁碟，記憶體裡只放 int8 量化後的版本；查詢時先用量化版粗篩，再讀完整向量重新計分。

use crate::VECTOR_SIZE;
use anyhow::{anyhow, Context, Result};
use qdrant_edge::{
    EdgeConfig, EdgeShard, NamedQuery, PointId, PointOperations, PointStruct, PointStructPersisted,
    PointInsertOperations, QuantizationSearchParams, QueryEnum, ScrollRequestBuilder, SearchParams, SearchRequestBuilder, SparseVector,
    UpdateOperation, Vector, VectorInternal, VectorStructInternal, Vectors, WithPayloadInterface, WithVector,
};
use rag_core::{Hit, ImagePoint, SparseVec, TextPoint};
use serde_json::json;
use std::path::Path;

pub const TEXT: &str = "text";
pub const IMAGE: &str = "image";
pub const KEYWORDS: &str = "kw";

fn err(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow!("向量資料庫：{e}")
}

#[derive(Clone, Copy)]
pub enum ShardKind {
    Text,
    Image,
}

/// 記憶體裡放的量化向量。只在建立新索引時決定，之後沿用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Quantization {
    /// 每個數字 1 位元組。2026-10-15 以前建的索引都是這種。
    Int8,
    /// 每個數字 1 位元，記憶體是 int8 的八分之一。
    OneBit,
    /// 1 位元，但查詢向量用 8 位元比對（不對稱量化），準確一些、慢一些。新索引的預設（實驗 84）：
    /// 10 萬個 chunk 時儲存層 106 MB → 28 MB，粗篩 5 倍時前 5 名和 int8 只差 1 題。
    #[default]
    OneBitQuery8,
}

impl Quantization {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "int8" => Some(Self::Int8),
            "1bit" => Some(Self::OneBit),
            "1bit-q8" => Some(Self::OneBitQuery8),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Int8 => "int8",
            Self::OneBit => "1bit",
            Self::OneBitQuery8 => "1bit-q8",
        }
    }

    /// 粗篩取幾倍的候選再用完整向量重新計分。1 位元比較粗，要多取一些（實驗 84：5 倍是準確度和
    /// 記憶體不足時速度的甜蜜點；倍數越多越接近完整搜尋，但每次查詢要從磁碟讀回的完整向量也越多）。
    pub fn default_oversampling(self) -> f64 {
        match self {
            Self::Int8 => 2.0,
            Self::OneBit | Self::OneBitQuery8 => 5.0,
        }
    }

    /// 已經存在的 shard 建立時用的量化方式（讀它的設定檔）；還沒建立或讀不出來就回傳 `None`。
    pub fn of_shard(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path.join("edge_config.json")).ok()?;
        let value: serde_json::Value = serde_json::from_str(&text).ok()?;
        let config = value.get("vectors")?.as_object()?.values().next()?.get("quantization_config")?;
        if config.get("scalar").is_some() {
            Some(Self::Int8)
        } else if let Some(binary) = config.get("binary") {
            let query8 = binary.get("query_encoding").and_then(|e| e.as_str()) == Some("scalar8bits");
            Some(if query8 { Self::OneBitQuery8 } else { Self::OneBit })
        } else {
            None
        }
    }
}

fn config(kind: ShardKind, quantization: Quantization) -> Result<EdgeConfig> {
    let quantization = match quantization {
        Quantization::Int8 => json!({ "scalar": { "type": "int8", "memory": "pinned" } }),
        Quantization::OneBit => json!({ "binary": { "memory": "pinned" } }),
        Quantization::OneBitQuery8 => json!({ "binary": { "memory": "pinned", "query_encoding": "scalar8bits" } }),
    };
    let dense = json!({
        "size": VECTOR_SIZE,
        "distance": "Dot",            // 向量都已正規化，內積就是餘弦相似度
        "on_disk": true,              // 完整向量放磁碟
        "quantization_config": quantization,
    });
    // 寫入紀錄檔（WAL）每個檔案預設先佔 32 MB，空的 shard 就有約 100 MB；改成 1 MB。
    // 比這個大的一筆寫入會自己用一個夠大的檔案，不受這個上限限制。
    let wal = json!({ "segment_capacity": 1024 * 1024, "segment_queue_len": 0, "retain_closed": 1 });
    let value = match kind {
        ShardKind::Text => json!({
            "on_disk_payload": true,
            "vectors": { TEXT: dense },
            "sparse_vectors": { KEYWORDS: { "modifier": "idf", "on_disk": true } },
            "wal_options": wal,
        }),
        ShardKind::Image => json!({ "on_disk_payload": true, "vectors": { IMAGE: dense }, "wal_options": wal }),
    };
    serde_json::from_value(value).context("向量資料庫的設定格式不對")
}

pub fn open_shard(path: &Path, kind: ShardKind, quantization: Quantization) -> Result<EdgeShard> {
    if path.join("edge_config.json").exists() {
        EdgeShard::load(path, None).map_err(err)
    } else {
        std::fs::create_dir_all(path)?;
        EdgeShard::new(path, config(kind, quantization)?).map_err(err)
    }
}

/// 把一個 shard 的全部資料（向量、關鍵字、附帶資料）原樣寫進另一個 shard。實驗換量化方式時用，不必重算向量。
pub fn copy_points(from: &EdgeShard, to: &EdgeShard) -> Result<usize> {
    let mut offset = None;
    let mut count = 0;
    loop {
        let mut builder = ScrollRequestBuilder::new()
            .limit(256)
            .with_payload(WithPayloadInterface::Bool(true))
            .with_vector(WithVector::Bool(true));
        if let Some(o) = offset {
            builder = builder.offset(o);
        }
        let (records, next) = from.scroll(builder.build()).map_err(err)?;
        let points: Vec<PointStructPersisted> = records
            .into_iter()
            .filter_map(|r| {
                let payload = r.payload.map(|p| serde_json::to_value(p).unwrap_or_default()).unwrap_or_else(|| json!({}));
                Some(PointStruct::new(r.id, Vectors::from(r.vector?), payload).into())
            })
            .collect();
        count += points.len();
        upsert(to, points)?;
        match next {
            Some(n) => offset = Some(n),
            None => break,
        }
    }
    to.flush().map_err(err)?;
    Ok(count)
}

/// 放大索引（實驗用）：shard 裡原有的每一點各複製 `copies` 份。`map`（原編號, 原 file_id, 第幾份）回傳
/// （新編號, 新 file_id），回傳 `None` 的不複製。稠密向量每份加一點隨機擾動再正規化（`noise` 0.1 時和原向量的
/// 餘弦相似度約 0.995），不讓同樣的向量重複出現；稀疏向量（關鍵字）照抄。每寫入約 5,000 點整理一次，壓低記憶體。
pub fn replicate_points(shard: &EdgeShard, copies: usize, noise: f32, map: &dyn Fn(u64, i64, usize) -> Option<(u64, i64)>) -> Result<usize> {
    // 先讀出全部原有的點，再寫入新的點（邊讀邊寫會讀到剛寫入的點）
    let mut originals = Vec::new();
    let mut offset = None;
    loop {
        let mut builder = ScrollRequestBuilder::new()
            .limit(256)
            .with_payload(WithPayloadInterface::Bool(true))
            .with_vector(WithVector::Bool(true));
        if let Some(o) = offset {
            builder = builder.offset(o);
        }
        let (records, next) = shard.scroll(builder.build()).map_err(err)?;
        originals.extend(records);
        match next {
            Some(n) => offset = Some(n),
            None => break,
        }
    }
    let mut count = 0;
    let mut since_optimize = 0;
    let mut batch: Vec<PointStructPersisted> = Vec::new();
    for c in 1..=copies {
        for r in &originals {
            let (PointId::NumId(id), Some(VectorStructInternal::Named(named))) = (r.id, r.vector.as_ref()) else { continue };
            let file_id = r.payload.as_ref().and_then(|p| p.0.get("file_id")).and_then(|v| v.as_i64()).unwrap_or(-1);
            let Some((new_id, new_file)) = map(id, file_id, c) else { continue };
            let mut seed = new_id | 1;
            let mut named = named.clone();
            for v in named.values_mut() {
                if let VectorInternal::Dense(d) = v {
                    perturb(d, noise, &mut seed);
                }
            }
            batch.push(PointStruct::new(PointId::NumId(new_id), Vectors::from(VectorStructInternal::Named(named)), json!({ "file_id": new_file })).into());
            if batch.len() >= 256 {
                count += batch.len();
                since_optimize += batch.len();
                upsert(shard, std::mem::take(&mut batch))?;
                if since_optimize >= 5_000 {
                    shard.optimize().map_err(err)?;
                    shard.flush().map_err(err)?;
                    since_optimize = 0;
                }
            }
        }
    }
    count += batch.len();
    upsert(shard, batch)?;
    shard.flush().map_err(err)?;
    Ok(count)
}

/// 加上均勻分布的隨機擾動（xorshift，結果可重現）再正規化成單位長度。
fn perturb(v: &mut [f32], noise: f32, seed: &mut u64) {
    if noise <= 0.0 || v.is_empty() {
        return;
    }
    // 每個分量 ±noise × √(3 / 維度)：擾動向量的長度約等於 noise
    let scale = noise * (3.0 / v.len() as f32).sqrt();
    for x in v.iter_mut() {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        let u = (*seed >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0;
        *x += u * scale;
    }
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        v.iter_mut().for_each(|x| *x /= norm);
    }
}

fn upsert(shard: &EdgeShard, points: Vec<PointStructPersisted>) -> Result<()> {
    if points.is_empty() {
        return Ok(());
    }
    shard.update(UpdateOperation::PointOperation(PointOperations::UpsertPoints(PointInsertOperations::PointsList(points)))).map_err(err)
}

pub fn upsert_text(shard: &EdgeShard, points: &[TextPoint]) -> Result<()> {
    let mut out = Vec::with_capacity(points.len());
    for p in points {
        let sparse = Vector::new_sparse(p.sparse.indices.clone(), p.sparse.values.clone()).map_err(err)?;
        let vectors = Vectors::new_named([(TEXT, Vector::new_dense(p.dense.clone())), (KEYWORDS, sparse)]);
        out.push(PointStruct::new(PointId::NumId(p.id), vectors, json!({ "file_id": p.file_id })).into());
    }
    upsert(shard, out)
}

pub fn upsert_images(shard: &EdgeShard, points: &[ImagePoint]) -> Result<()> {
    let out = points
        .iter()
        .map(|p| {
            let vectors = Vectors::new_named([(IMAGE, Vector::new_dense(p.dense.clone()))]);
            PointStruct::new(PointId::NumId(p.id), vectors, json!({ "file_id": p.file_id })).into()
        })
        .collect();
    upsert(shard, out)
}

pub fn delete_points(shard: &EdgeShard, ids: &[u64]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let ids = ids.iter().map(|id| PointId::NumId(*id)).collect();
    shard.update(UpdateOperation::PointOperation(PointOperations::DeletePoints { ids })).map_err(err)
}

pub fn flush(shard: &EdgeShard) -> Result<()> {
    shard.flush().map_err(err)
}

pub fn optimize(shard: &EdgeShard) -> Result<bool> {
    shard.optimize().map_err(err)
}

fn hits(points: Vec<qdrant_edge::ScoredPoint>) -> Vec<Hit> {
    points
        .into_iter()
        .filter_map(|p| match p.id {
            PointId::NumId(id) => Some(Hit { id, score: p.score }),
            PointId::Uuid(_) => None,
        })
        .collect()
}

pub fn search_dense(shard: &EdgeShard, name: &str, query: &[f32], limit: usize, oversampling: f64) -> Result<Vec<Hit>> {
    let query = QueryEnum::Nearest(NamedQuery { query: VectorInternal::Dense(query.to_vec()), using: Some(name.to_string()) });
    // 量化向量粗篩取 oversampling 倍的量，再用磁碟上的完整向量重新計分
    let params = SearchParams {
        quantization: Some(QuantizationSearchParams { ignore: false, rescore: Some(true), oversampling: Some(oversampling) }),
        ..Default::default()
    };
    let request = SearchRequestBuilder::new(query, limit).params(params).build();
    Ok(hits(shard.search(request).map_err(err)?))
}

pub fn search_sparse(shard: &EdgeShard, query: &SparseVec, limit: usize) -> Result<Vec<Hit>> {
    if query.indices.is_empty() {
        return Ok(Vec::new());
    }
    let sparse = SparseVector::new(query.indices.clone(), query.values.clone()).map_err(err)?;
    let query = QueryEnum::Nearest(NamedQuery { query: VectorInternal::Sparse(sparse), using: Some(KEYWORDS.to_string()) });
    let request = SearchRequestBuilder::new(query, limit).build();
    Ok(hits(shard.search(request).map_err(err)?))
}
