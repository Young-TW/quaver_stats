use image::DynamicImage;
use image::ImageReader;
use image::imageops::FilterType;
use reqwest::Client;
use sha1::Sha1;
use sha1::Digest;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tokio::fs;

/// 以頭像 URL 的 SHA1 計算磁碟快取的檔名 key（純函式，便於測試）。
fn cache_key(avatar_url: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(avatar_url.as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{:02x}", b)).collect::<String>()
}

/// 計算頭像快取的目錄路徑 `<cache_dir>/quaver_stats/avatars`。
fn avatar_cache_dir() -> PathBuf {
    let mut cache_dir = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("."));
    cache_dir.push("quaver_stats");
    cache_dir.push("avatars");
    cache_dir
}

/// 解碼位元組並縮放成指定大小（自動猜測圖片格式）。
fn decode_and_resize(bytes: Vec<u8>, size: (u32, u32)) -> DynamicImage {
    ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .expect("無法解析大頭照格式")
        .decode()
        .expect("無法解碼大頭照")
        .resize_exact(size.0, size.1, FilterType::Lanczos3)
}

pub async fn fetch_avatar_from_url(avatar_url: &str, size: (u32, u32)) -> DynamicImage {
    // 準備快取路徑 ~/.cache/quaver_stats/avatars/<sha1>.bin
    let cache_dir = avatar_cache_dir();
    fs::create_dir_all(&cache_dir).await.expect("無法建立快取資料夾");

    let cache_path = cache_dir.join(format!("{}.bin", cache_key(avatar_url)));

    // 若快取存在就讀取快取
    if let Ok(bytes) = fs::read(&cache_path).await {
        return decode_and_resize(bytes, size);
    }

    // 快取不存在：下載
    let client = Client::new();
    let avatar_bytes = client.get(avatar_url)
        .send()
        .await
        .expect("無法下載大頭照")
        .bytes()
        .await
        .expect("無法讀取大頭照");

    let bytes_vec = avatar_bytes.to_vec();

    write_cache(&cache_path, &bytes_vec).await;

    decode_and_resize(bytes_vec, size)
}

/// 原子性寫入快取（先寫 tmp 再 rename），忽略 rename 競態錯誤。
async fn write_cache(cache_path: &Path, bytes: &[u8]) {
    let tmp_path = cache_path.with_extension("tmp");
    fs::write(&tmp_path, bytes).await.expect("無法寫入快取暫存檔");
    fs::rename(&tmp_path, cache_path).await.ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_is_sha1_hex() {
        // 已知 SHA1("https://example.com/avatar.png")
        let key = cache_key("https://example.com/avatar.png");
        assert_eq!(key.len(), 40); // SHA1 = 20 bytes = 40 hex chars
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_cache_key_is_deterministic_and_distinct() {
        let a1 = cache_key("https://a.example/x.png");
        let a2 = cache_key("https://a.example/x.png");
        let b = cache_key("https://b.example/y.png");
        assert_eq!(a1, a2);
        assert_ne!(a1, b);
    }

    #[test]
    fn test_cache_key_known_value() {
        // 對照值來自標準 SHA1
        assert_eq!(
            cache_key("abc"),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }

    #[test]
    fn test_avatar_cache_dir_ends_with_expected_path() {
        let dir = avatar_cache_dir();
        assert!(dir.ends_with("quaver_stats/avatars"));
    }

    #[tokio::test]
    async fn test_write_cache_creates_file_atomically() {
        let tmp = std::env::temp_dir().join(format!("quaver_test_{}.bin", cache_key("write-test")));
        let _ = fs::remove_file(&tmp).await;

        write_cache(&tmp, b"hello").await;
        assert_eq!(fs::read(&tmp).await.unwrap(), b"hello");
        // 不應殘留 tmp 暫存檔
        assert!(!tmp.with_extension("tmp").exists());

        let _ = fs::remove_file(&tmp).await;
    }

    #[test]
    fn test_decode_and_resize_scales_image() {
        // 產生一張 10x10 的 PNG 後解碼縮放至 4x4
        let src = DynamicImage::new_rgba8(10, 10);
        let mut buf = Cursor::new(Vec::new());
        src.write_to(&mut buf, image::ImageFormat::Png).unwrap();

        let resized = decode_and_resize(buf.into_inner(), (4, 4));
        assert_eq!(resized.width(), 4);
        assert_eq!(resized.height(), 4);
    }
}
