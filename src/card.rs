use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use image::ImageReader;
use image::imageops::FilterType;
use image::imageops::overlay;
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use std::io::Cursor;
use std::sync::Arc;

use ab_glyph::{FontArc, PxScale};

use crate::avatar::fetch_avatar_from_url;
use crate::cache::Cache;
use crate::user::User;

const BACKGROUND_PATH: &str = "assets/image/quaver.jpg";
const CARD_WIDTH: u32 = 256;
const CARD_HEIGHT: u32 = 192;

#[derive(Debug)]
enum CardError {
    UserNotFound,
    UpstreamError,
}

pub async fn generate_card(
    Path(username): Path<String>,
    Extension(cache): Extension<Arc<Cache>>,
) -> Response {
    if let Some(cached_image) = cache.get(&username).await {
        return (
            [(axum::http::header::CONTENT_TYPE, "image/png")],
            cached_image,
        )
            .into_response();
    }

    let result = generate_card_image(&username).await;
    resolve_card(&username, result, &cache).await
}

// Caches the image on success and builds the appropriate HTTP response.
// Extracted so that the caching/status logic can be tested without network calls.
async fn resolve_card(
    username: &str,
    result: Result<Vec<u8>, CardError>,
    cache: &Cache,
) -> Response {
    match result {
        Ok(bytes) => {
            cache.set(username.to_string(), bytes.clone()).await;
            ([(axum::http::header::CONTENT_TYPE, "image/png")], bytes).into_response()
        }
        Err(CardError::UserNotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(CardError::UpstreamError) => StatusCode::BAD_GATEWAY.into_response(),
    }
}

async fn generate_card_image(username: &str) -> Result<Vec<u8>, CardError> {
    let user_id = match User::fetch_id(username).await {
        Ok(0) => return Err(CardError::UserNotFound),
        Ok(id) => id,
        Err(_) => return Err(CardError::UpstreamError),
    };

    let user_stat = match User::fetch_stat(user_id).await {
        Ok(u) => u,
        Err(_) => return Err(CardError::UpstreamError),
    };

    let avatar = fetch_avatar_from_url(&user_stat.avatar_url, (64, 64)).await;
    Ok(render_card(&user_stat, &avatar))
}

/// 將玩家資料與頭像渲染成 PNG 位元組（純函式，不依賴網路，便於測試）。
fn render_card(user_stat: &User, avatar: &DynamicImage) -> Vec<u8> {
    // 建立圖卡，使用背景圖
    let bg_img = ImageReader::open(BACKGROUND_PATH)
        .expect("無法打開背景圖")
        .decode()
        .expect("無法解析背景圖")
        .resize_exact(CARD_WIDTH, CARD_HEIGHT, FilterType::Lanczos3)
        .to_rgba8();

    let mut img = bg_img; // 用背景圖作為畫布

    overlay(&mut img, &avatar.to_rgba8(), 10, 10); // 將大頭照繪製到卡片左上角

    // 載入字型（請確保 assets/JetBrainsMono.ttf 存在）
    let font = FontArc::try_from_slice(include_bytes!("../assets/JetBrainsMono/JetBrainsMono.ttf"))
        .unwrap();
    let scale = PxScale::from(20.0);

    for (i, line) in build_lines(user_stat).iter().enumerate() {
        draw_line(&mut img, line, 10, 80 + i as i32 * 20, scale, &font); // 調整文字位置
    }

    // 輸出成 PNG
    let mut buffer = Cursor::new(Vec::new());
    let dynimg = DynamicImage::ImageRgba8(img);
    dynimg.write_to(&mut buffer, ImageFormat::Png).unwrap();

    buffer.into_inner()
}

/// 根據玩家資料組出卡片要顯示的文字行。
fn build_lines(user_stat: &User) -> Vec<String> {
    vec![
        format!("{} ({})", user_stat.name, user_stat.country),
        format!("Global Rank: #{}", user_stat.global_rank),
        format!("Country Rank: #{}", user_stat.country_rank),
        format!("rating: {:.2}", user_stat.rating),
        format!("Accuracy: {:.2}%", user_stat.accuracy),
    ]
}

// 繪製單行文字
fn draw_line(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    text: &str,
    x: i32,
    y: i32,
    scale: PxScale,
    font: &FontArc,
) {
    use imageproc::drawing::draw_text_mut;
    draw_text_mut(img, Rgba([255, 255, 255, 255]), x, y, scale, font, text);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn sample_user() -> User {
        User {
            name: "young".to_string(),
            country: "TW".to_string(),
            global_rank: 1234,
            country_rank: 56,
            rating: 712.345,
            accuracy: 98.765,
            avatar_url: "https://example.com/a.png".to_string(),
        }
    }

    #[test]
    fn test_build_lines_formats_all_fields() {
        let lines = build_lines(&sample_user());
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], "young (TW)");
        assert_eq!(lines[1], "Global Rank: #1234");
        assert_eq!(lines[2], "Country Rank: #56");
        // rating 與 accuracy 取小數點後兩位
        assert_eq!(lines[3], "rating: 712.35");
        assert_eq!(lines[4], "Accuracy: 98.77%");
    }

    #[test]
    fn test_render_card_produces_valid_png() {
        let avatar = DynamicImage::new_rgba8(64, 64);
        let bytes = render_card(&sample_user(), &avatar);

        assert!(!bytes.is_empty());
        // PNG 檔頭魔術位元組
        assert_eq!(
            &bytes[..8],
            &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
        );

        // 解碼回來應為設定的卡片尺寸
        let decoded = image::load_from_memory(&bytes).expect("輸出應為合法圖片");
        assert_eq!(decoded.width(), CARD_WIDTH);
        assert_eq!(decoded.height(), CARD_HEIGHT);
    }

    #[tokio::test]
    async fn test_missing_user_returns_404_and_not_cached() {
        let cache = Cache::new(Duration::from_secs(60));
        let response = resolve_card("ghost", Err(CardError::UserNotFound), &cache).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(
            cache.get("ghost").await.is_none(),
            "failed lookup must not be cached"
        );
    }
}
