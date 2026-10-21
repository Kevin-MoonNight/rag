//! RAG 核心：雲端和個人電腦共用的部分。
//!
//! 這個套件不碰檔案格式、不載入模型、不直接讀寫資料庫。輸入是文件轉換套件產生的中間格式（[`Block`]），
//! 模型和儲存透過 [`Embedder`]、[`Store`] 兩個介面接進來。

pub mod answer;
pub mod chunk;
pub mod ids;
pub mod index;
pub mod math;
pub mod merge;
pub mod search;
pub mod text;
pub mod types;

pub use answer::{AnswerOptions, Passage};
pub use chunk::{chunk_blocks, Chunk, ChunkOptions};
pub use index::{index_document, reembed_document, IndexOptions, IndexReport};
pub use merge::{take_turns, Merged, Source};
pub use search::{search, search_keywords, SearchOptions, SearchResult};
pub use types::{Block, BlockKind, Embedder, Hit, ImagePoint, SparseVec, Store, TextPoint};
