//! 資料點的編號。
//!
//! 編號由「檔案內容的雜湊值 + 類型 + 頁碼 + 序號」算出來，同一份內容重新建索引會得到同樣的編號，
//! 所以中斷後重做同一批只會覆蓋原有的資料，不會產生重複。

use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointKind {
    Text,
    Image,
}

/// `content_hash` 是檔案內容的雜湊值（十六進位字串）；`index` 是這個檔案裡同類型資料的序號。
pub fn point_id(content_hash: &str, kind: PointKind, page: u32, index: u32) -> u64 {
    let mut h = Sha256::new();
    h.update(content_hash.as_bytes());
    h.update([match kind {
        PointKind::Text => b't',
        PointKind::Image => b'i',
    }]);
    h.update(page.to_le_bytes());
    h.update(index.to_le_bytes());
    let digest = h.finalize();
    u64::from_le_bytes(digest[..8].try_into().unwrap())
}

/// 檔案內容的雜湊值（SHA-256，十六進位）。
pub fn content_hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_and_distinct() {
        let h = content_hash(b"abc");
        assert_eq!(h, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let a = point_id(&h, PointKind::Text, 1, 0);
        assert_eq!(a, point_id(&h, PointKind::Text, 1, 0));
        assert_ne!(a, point_id(&h, PointKind::Image, 1, 0));
        assert_ne!(a, point_id(&h, PointKind::Text, 2, 0));
        assert_ne!(a, point_id(&h, PointKind::Text, 1, 1));
        assert_ne!(a, point_id(&content_hash(b"abd"), PointKind::Text, 1, 0));
    }
}
