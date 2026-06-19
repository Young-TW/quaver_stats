use image::DynamicImage;
use image::ImageReader;
use image::imageops::FilterType;
use reqwest::Client;
use sha1::Digest;
use sha1::Sha1;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs;

/// 以頭像 URL 的 SHA1 計算磁碟快取的檔名 key（純函式，便於測試）。
fn cache_key(avatar_url: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(avatar_url.as_bytes());
    let digest = hasher.finalize();
    digest
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
}

/// 計算頭像快取的目錄路徑 `<cache_dir>/quaver_stats/avatars`。
fn avatar_cache_dir() -> PathBuf {
    let mut cache_dir = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("."));
    cache_dir.push("quaver_stats");
    cache_dir.push("avatars");
    cache_dir
}

/// 解碼位元組並縮放成指定大小（自動猜測圖片格式）。
///
/// 位元組來自不受信任的來源（CDN 可能回傳 HTML 錯誤頁、限流回應或損毀檔案），
/// 因此格式偵測與解碼失敗時不會 panic，而是回傳一張指定大小的透明佔位圖，
/// 讓整個請求得以繼續而非讓任務崩潰。
fn decode_and_resize(bytes: Vec<u8>, size: (u32, u32)) -> DynamicImage {
    let decoded = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.decode().ok());

    match decoded {
        Some(img) => img.resize_exact(size.0, size.1, FilterType::Lanczos3),
        None => DynamicImage::new_rgba8(size.0, size.1),
    }
}

/// Returns the avatar at `avatar_url` decoded and resized to `size`
/// (`(width, height)`).
///
/// The raw bytes are cached on disk under the per-user cache directory, keyed
/// by the SHA1 of `avatar_url`; a cache hit avoids the network request. On a
/// miss the image is downloaded and written to the cache before being returned.
///
/// Bytes that cannot be decoded as an image (e.g. a CDN error page) yield a
/// blank placeholder rather than panicking.
///
/// # Panics
///
/// Panics if the cache directory cannot be created or the download fails.
pub async fn fetch_avatar_from_url(avatar_url: &str, size: (u32, u32)) -> DynamicImage {
    // 準備快取路徑 ~/.cache/quaver_stats/avatars/<sha1>.bin
    let cache_dir = avatar_cache_dir();
    fs::create_dir_all(&cache_dir)
        .await
        .expect("無法建立快取資料夾");

    let cache_path = cache_dir.join(format!("{}.bin", cache_key(avatar_url)));

    // 若快取存在就讀取快取
    if let Ok(bytes) = fs::read(&cache_path).await {
        return decode_and_resize(bytes, size);
    }

    // 快取不存在：下載
    // 設定 timeout，避免永不回應的上游無限期占用 Tokio worker（issue #7）。
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("無法建立 HTTP client");
    let avatar_bytes = client
        .get(avatar_url)
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
    fs::write(&tmp_path, bytes)
        .await
        .expect("無法寫入快取暫存檔");
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
        assert_eq!(cache_key("abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
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

    /// 迴歸測試（issue #5）：CDN 回傳 HTML 錯誤頁、限流回應或損毀檔案時，
    /// 解碼路徑不應 panic。預期行為是回傳錯誤 / 安全處理，而非讓 Axum 任務崩潰。
    ///
    /// 目前的 `decode_and_resize` 對不受信任的位元組呼叫 `.expect(...)`，因此這個
    /// 測試在尚未修復前會因為 panic 而失敗（這是預期的）。
    #[test]
    fn test_decode_and_resize_invalid_bytes_does_not_panic() {
        // 模擬 Quaver CDN 回傳一個 HTML 錯誤頁而非圖片。
        let html_error_page = b"<html><body>503 Service Unavailable</body></html>".to_vec();

        let result = std::panic::catch_unwind(|| {
            decode_and_resize(html_error_page, (64, 64));
        });

        // 不應 panic：無效的位元組必須被安全處理，而不是 unwind 整個任務。
        assert!(
            result.is_ok(),
            "decode_and_resize 在收到非圖片位元組時 panic 了；預期應安全回傳錯誤而非崩潰"
        );
    }

    /// 迴歸測試（issue #7）：對外的 HTTP 請求必須設定 timeout。
    ///
    /// 模擬一個「永遠不回應」的上游：TCP 連線可以建立，但伺服器接受連線後
    /// 既不回傳任何資料也不關閉連線。若 `fetch_avatar_from_url` 使用的
    /// `reqwest` client 沒有設定 timeout（目前 `Client::new()` 即是如此），
    /// 這個請求會無限期占用 Tokio worker，永遠不會結束。
    ///
    /// 預期（修復後）行為：client 設定了 timeout（issue 建議 10 秒），因此
    /// 請求會在 timeout 視窗內結束（成功與否不重要，重點是「會結束」而非
    /// 卡死）。我們以一個比 timeout 更寬的外層 12 秒上限來判定：若內層
    /// 任務在 12 秒內結束（不論回傳或 panic），代表 timeout 生效；若外層
    /// 12 秒先到，代表沒有 timeout、請求卡死，測試失敗。
    ///
    /// 目前尚未設定 timeout，因此此測試會因為內層永不結束而失敗（這是預期的）。
    #[tokio::test]
    async fn test_fetch_avatar_has_http_timeout_on_stalled_upstream() {
        use std::time::Duration;

        // 永不回應的上游：接受連線後把 socket 留著，從不寫入任何回應。
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let mut held = Vec::new();
            loop {
                if let Ok((sock, _)) = listener.accept().await {
                    // 持有 socket 使連線保持開啟，但永不回應。
                    held.push(sock);
                }
            }
        });

        let url = format!("http://{}/never-responds.png", addr);

        // 在獨立的 task 中執行，這樣即使下載失敗時 panic（expect），
        // 也只會變成 JoinError 而不會讓整個測試 unwind；我們關心的是「是否在
        // timeout 視窗內結束」。
        let handle = tokio::spawn(async move { fetch_avatar_from_url(&url, (64, 64)).await });

        // 外層上限（12 秒）大於 issue 建議的 10 秒 timeout，給 client 的 timeout
        // 機會先觸發。
        let outcome = tokio::time::timeout(Duration::from_secs(12), handle).await;

        assert!(
            outcome.is_ok(),
            "對永不回應的上游發出的請求在 12 秒內沒有結束；表示外部 HTTP 請求沒有設定 timeout，會無限期占用 Tokio worker（issue #7）"
        );
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
