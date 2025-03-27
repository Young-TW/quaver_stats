use reqwest::Error;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct ApiResponse<T> {
    status: String,
    data: T,
}

#[derive(Deserialize, Debug)]
pub struct User {
    id: u64,
    username: String,
    // 你可以加上更多欄位，例如 country, avatar 等等
}

#[derive(Deserialize, Debug)]
pub struct UserStats {
    global_rank: u64,
    country_rank: u64,
    performance_points: f64,
    // 可以根據 API 回傳補上更多欄位
}

#[derive(Deserialize, Debug)]
struct UserDetail {
    user: User,
    stats: UserStats,
}

pub async fn get_user_id(user_name: &str) -> Result<Option<User>, reqwest::Error> {
    let url = format!("https://api.quavergame.com/v2/user/search/{}", user_name);
    let response = reqwest::get(&url).await?;
    let api_response: ApiResponse<Vec<User>> = response.json().await?;
    Ok(api_response.data.into_iter().next())
}

/// 根據使用者 ID 抓取該使用者的統計資料
pub async fn get_user_stat(user_id: &str) -> Result<UserStats, Error> {
    let url = format!("https://api.quavergame.com/v2/user/{}", user_id);
    let response = reqwest::get(&url).await?;
    let api_response: ApiResponse<UserDetail> = response.json().await?;
    Ok(api_response.data.stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_user_id() {
        let user_id = get_user_id("example_user").await.unwrap();
        assert!(user_id.is_some());
    }

    #[tokio::test]
    async fn test_get_user_stat() {
        let user_id = get_user_id("example_user").await.unwrap();
        let user_stat = get_user_stat(&user_id.unwrap().id.to_string())
            .await
            .unwrap();
        assert!(user_stat.global_rank > 0);
    }
}
