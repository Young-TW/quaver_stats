use image::{DynamicImage, ImageReader};
use image::imageops::FilterType;
use reqwest::Client;

pub async fn fetch_avatar_from_url(avatar_url: &str, size: (u32, u32)) -> DynamicImage {
    // 下載大頭照
    let client = Client::new();
    let avatar_bytes = client.get(avatar_url)
        .send()
        .await
        .expect("無法下載大頭照")
        .bytes()
        .await
        .expect("無法讀取大頭照");

    // 將大頭照轉換為 DynamicImage 並調整大小
    let avatar = ImageReader::new(std::io::Cursor::new(avatar_bytes))
        .with_guessed_format()
        .expect("無法解析大頭照格式")
        .decode()
        .expect("無法解碼大頭照")
        .resize_exact(size.0, size.1, FilterType::Lanczos3);

    avatar
}