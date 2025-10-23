use image::DynamicImage;
use image::io::Reader as ImageReader;
use image::imageops::FilterType;
use reqwest::Client;
use sha1::Sha1;
use sha1::Digest;
use std::io::Cursor;
use std::path::PathBuf;
use tokio::fs;

pub async fn fetch_avatar_from_url(avatar_url: &str, size: (u32, u32)) -> DynamicImage {
    // 準備快取路徑 ~/.cache/quaver_stats/avatars/<sha1>.bin
    let mut cache_dir = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("."));
    cache_dir.push("quaver_stats");
    cache_dir.push("avatars");
    fs::create_dir_all(&cache_dir).await.expect("無法建立快取資料夾");

    let mut hasher = Sha1::new();
    hasher.update(avatar_url.as_bytes());
    let digest = hasher.finalize();
    let key = digest.iter().map(|b| format!("{:02x}", b)).collect::<String>();
    let cache_path = cache_dir.join(format!("{}.bin", key));
    // 若快取存在就讀取快取
    if let Ok(bytes) = fs::read(&cache_path).await {
        let avatar = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .expect("無法解析大頭照格式（快取）")
            .decode()
            .expect("無法解碼大頭照（快取）")
            .resize_exact(size.0, size.1, FilterType::Lanczos3);
        return avatar;
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

    // 原子性寫入快取（先寫 tmp 再 rename）
    let tmp_path = cache_path.with_extension("tmp");
    fs::write(&tmp_path, &bytes_vec).await.expect("無法寫入快取暫存檔");
    fs::rename(&tmp_path, &cache_path).await.ok(); // 忽略 rename 錯誤（若已被其他工作寫入）

    // 解碼並回傳
    let avatar = ImageReader::new(Cursor::new(bytes_vec))
        .with_guessed_format()
        .expect("無法解析大頭照格式")
        .decode()
        .expect("無法解碼大頭照")
        .resize_exact(size.0, size.1, FilterType::Lanczos3);

    avatar
}