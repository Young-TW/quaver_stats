use reqwest::Error;
use serde::Deserialize;

/// A Quaver player's profile and 7K statistics, flattened from the API
/// response.
#[derive(Debug, Deserialize)]
pub struct User {
    /// The player's username.
    pub name: String,
    /// The player's global 7K rank.
    pub global_rank: u64,
    /// The player's country 7K rank.
    pub country_rank: u64,
    /// The player's country code.
    pub country: String,
    /// The player's overall 7K performance rating.
    pub rating: f64,
    /// The player's overall 7K accuracy, as a percentage.
    pub accuracy: f64,
    /// URL of the player's avatar image.
    pub avatar_url: String, // 新增欄位
}

// API 回應的反序列化結構，抽出至模組層級以便單獨測試解析邏輯
#[derive(Debug, Deserialize)]
struct UserSearchResponse {
    users: Vec<RawUser>,
}

#[derive(Debug, Deserialize)]
struct RawUser {
    id: u64,
}

#[derive(Debug, Deserialize)]
struct UserResponse {
    user: RawUserDetail,
}

#[derive(Debug, Deserialize)]
struct RawUserDetail {
    username: String,
    country: String,
    avatar_url: String, // 新增欄位
    #[serde(rename = "stats_keys7")]
    stats: RawStats,
}

#[derive(Debug, Deserialize)]
struct RawStats {
    ranks: Ranks,
    #[serde(rename = "overall_performance_rating")]
    performance: f64,
    #[serde(rename = "overall_accuracy")]
    accuracy: f64,
}

#[derive(Debug, Deserialize)]
struct Ranks {
    global: u64,
    country: u64,
}

impl User {
    /// 從搜尋 API 的 JSON 字串解析出使用者 ID（取第一筆，無資料則回傳 0）。
    /// 拆出純函式以便不依賴網路即可測試解析邏輯。
    fn parse_id(body: &str) -> Result<u64, serde_json::Error> {
        let result: UserSearchResponse = serde_json::from_str(body)?;
        Ok(result.users.first().map(|u| u.id).unwrap_or(0))
    }

    /// 從使用者 API 的 JSON 字串解析出 `User`。
    #[cfg(test)]
    fn parse_stat(body: &str) -> Result<User, serde_json::Error> {
        let result: UserResponse = serde_json::from_str(body)?;
        Ok(User::from_detail(result.user))
    }

    /// Looks up a player by `name` via the Quaver user-search API and returns
    /// the first matching user's ID.
    ///
    /// Returns `Ok(0)` when no user matches or the response cannot be parsed.
    /// Returns `Err` only if the HTTP request itself fails.
    pub async fn fetch_id(name: &str) -> Result<u64, Error> {
        let url = format!("https://api.quavergame.com/v2/user/search/{}", name);
        let body = reqwest::get(&url).await?.text().await?;
        // 解析失敗時視為查無使用者，回傳 0（與舊行為一致）
        Ok(Self::parse_id(&body).unwrap_or(0))
    }

    /// Fetches the player with the given `id` from the Quaver user API and
    /// builds a [`User`] from their 7K statistics.
    ///
    /// Returns `Err` if the HTTP request fails or the JSON response cannot be
    /// deserialized.
    pub async fn fetch_stat(id: u64) -> Result<User, Error> {
        let url = format!("https://api.quavergame.com/v2/user/{}", id);
        let response = reqwest::get(&url).await?;
        let result: UserResponse = response.json().await?;
        Ok(User::from_detail(result.user))
    }

    fn from_detail(detail: RawUserDetail) -> User {
        User {
            name: detail.username,
            country: detail.country,
            global_rank: detail.stats.ranks.global,
            country_rank: detail.stats.ranks.country,
            rating: detail.stats.performance,
            accuracy: detail.stats.accuracy,
            avatar_url: detail.avatar_url,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_id_picks_first_user() {
        let body = r#"{ "users": [ { "id": 48618 }, { "id": 99999 } ] }"#;
        assert_eq!(User::parse_id(body).unwrap(), 48618);
    }

    #[test]
    fn test_parse_id_empty_returns_zero() {
        let body = r#"{ "users": [] }"#;
        assert_eq!(User::parse_id(body).unwrap(), 0);
    }

    #[test]
    fn test_parse_id_invalid_json_errors() {
        assert!(User::parse_id("not json").is_err());
    }

    #[test]
    fn test_parse_stat_maps_all_fields() {
        let body = r#"{
            "user": {
                "username": "tyrcs",
                "country": "CN",
                "avatar_url": "https://example.com/a.png",
                "stats_keys7": {
                    "ranks": { "global": 1, "country": 2 },
                    "overall_performance_rating": 712.34,
                    "overall_accuracy": 98.76
                }
            }
        }"#;

        let user = User::parse_stat(body).unwrap();
        assert_eq!(user.name, "tyrcs");
        assert_eq!(user.country, "CN");
        assert_eq!(user.global_rank, 1);
        assert_eq!(user.country_rank, 2);
        assert_eq!(user.avatar_url, "https://example.com/a.png");
        assert!((user.rating - 712.34).abs() < f64::EPSILON);
        assert!((user.accuracy - 98.76).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_stat_missing_field_errors() {
        // 缺少 stats_keys7，應反序列化失敗
        let body = r#"{ "user": { "username": "x", "country": "US", "avatar_url": "" } }"#;
        assert!(User::parse_stat(body).is_err());
    }

    // 此測試會打真實的 Quaver API，預設忽略以避免 CI 受網路/外部資料影響。
    // 需要時可用 `cargo test -- --ignored` 執行。
    #[tokio::test]
    #[ignore]
    async fn test_fetch_user_stats() {
        let id = User::fetch_id("tyrcs").await.unwrap();
        assert_eq!(id, 48618);

        let user = User::fetch_stat(id).await.unwrap();
        assert_eq!(user.name, "tyrcs");
        assert_eq!(user.country, "CN");
        assert!(user.rating > 700.0);
        assert!(user.accuracy > 95.0);
    }
}
