use reqwest::Error;
use serde::Deserialize;

/// Selects which key mode's statistics to display on the card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Keys4,
    Keys7,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Keys4 => "4k",
            Mode::Keys7 => "7k",
        }
    }
}

/// A Quaver player's profile and statistics, flattened from the API response.
#[derive(Debug, Deserialize)]
pub struct User {
    /// The player's username.
    pub name: String,
    /// The player's global rank for the selected mode.
    pub global_rank: u64,
    /// The player's country rank for the selected mode.
    pub country_rank: u64,
    /// The player's country code.
    pub country: String,
    /// The player's overall performance rating for the selected mode.
    pub rating: f64,
    /// The player's overall accuracy for the selected mode, as a percentage.
    pub accuracy: f64,
    /// URL of the player's avatar image.
    pub avatar_url: String,
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

#[derive(Debug)]
enum ParseIdError {
    NotFound,
    Json,
}

impl From<serde_json::Error> for ParseIdError {
    fn from(_: serde_json::Error) -> Self {
        ParseIdError::Json
    }
}

#[derive(Debug, Deserialize)]
struct UserResponse {
    user: RawUserDetail,
}

#[derive(Debug, Deserialize)]
struct RawUserDetail {
    username: String,
    country: String,
    avatar_url: String,
    #[serde(rename = "stats_keys4")]
    stats4: RawStats,
    #[serde(rename = "stats_keys7")]
    stats7: RawStats,
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
    /// 從搜尋 API 的 JSON 字串解析出使用者 ID（取第一筆）。
    /// 無匹配使用者回傳 Err(ParseIdError::NotFound)；JSON 格式錯誤回傳 Err(ParseIdError::Json)。
    fn parse_id(body: &str) -> Result<u64, ParseIdError> {
        let result: UserSearchResponse = serde_json::from_str(body)?;
        result
            .users
            .first()
            .map(|u| u.id)
            .ok_or(ParseIdError::NotFound)
    }

    /// 從使用者 API 的 JSON 字串解析出 `User`。
    #[cfg(test)]
    fn parse_stat(body: &str, mode: Mode) -> Result<User, serde_json::Error> {
        let result: UserResponse = serde_json::from_str(body)?;
        Ok(User::from_detail(result.user, mode))
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
    /// builds a [`User`] from the statistics for the requested [`Mode`].
    ///
    /// Returns `Err` if the HTTP request fails or the JSON response cannot be
    /// deserialized.
    pub async fn fetch_stat(id: u64, mode: Mode) -> Result<User, Error> {
        let url = format!("https://api.quavergame.com/v2/user/{}", id);
        let response = reqwest::get(&url).await?;
        let result: UserResponse = response.json().await?;
        Ok(User::from_detail(result.user, mode))
    }

    fn from_detail(detail: RawUserDetail, mode: Mode) -> User {
        let stats = match mode {
            Mode::Keys4 => detail.stats4,
            Mode::Keys7 => detail.stats7,
        };
        User {
            name: detail.username,
            country: detail.country,
            global_rank: stats.ranks.global,
            country_rank: stats.ranks.country,
            rating: stats.performance,
            accuracy: stats.accuracy,
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
    fn test_parse_id_empty_returns_not_found() {
        let body = r#"{ "users": [] }"#;
        assert!(User::parse_id(body).is_err());
    }

    #[test]
    fn test_parse_id_invalid_json_errors() {
        assert!(User::parse_id("not json").is_err());
    }

    fn both_stats_body() -> &'static str {
        r#"{
            "user": {
                "username": "tyrcs",
                "country": "CN",
                "avatar_url": "https://example.com/a.png",
                "stats_keys4": {
                    "ranks": { "global": 10, "country": 3 },
                    "overall_performance_rating": 500.00,
                    "overall_accuracy": 97.00
                },
                "stats_keys7": {
                    "ranks": { "global": 1, "country": 2 },
                    "overall_performance_rating": 712.34,
                    "overall_accuracy": 98.76
                }
            }
        }"#
    }

    #[test]
    fn test_parse_stat_maps_all_fields() {
        let user = User::parse_stat(both_stats_body(), Mode::Keys7).unwrap();
        assert_eq!(user.name, "tyrcs");
        assert_eq!(user.country, "CN");
        assert_eq!(user.global_rank, 1);
        assert_eq!(user.country_rank, 2);
        assert_eq!(user.avatar_url, "https://example.com/a.png");
        assert!((user.rating - 712.34).abs() < f64::EPSILON);
        assert!((user.accuracy - 98.76).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_stat_uses_4k_stats_when_mode_keys4() {
        let user = User::parse_stat(both_stats_body(), Mode::Keys4).unwrap();
        assert_eq!(user.global_rank, 10);
        assert_eq!(user.country_rank, 3);
        assert!((user.rating - 500.00).abs() < f64::EPSILON);
        assert!((user.accuracy - 97.00).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_stat_missing_field_errors() {
        // 缺少 stats_keys4 與 stats_keys7，應反序列化失敗
        let body = r#"{ "user": { "username": "x", "country": "US", "avatar_url": "" } }"#;
        assert!(User::parse_stat(body, Mode::Keys7).is_err());
    }

    // Regression for issue #3: parse_id silently returned Ok(0) when the
    // search result contained no users, causing generate_card_image to call
    // fetch_stat(0) and make a wasted HTTP round-trip.
    // Expected: parse_id must NOT return Ok(0) for an empty users array —
    // it must signal "not found" distinctly (e.g. Err, or Ok(None) once the
    // return type is updated to Result<Option<u64>, _>).
    #[test]
    fn test_parse_id_empty_users_signals_not_found() {
        let body = r#"{"users":[]}"#;
        let result = User::parse_id(body);
        assert!(
            result.map_or(true, |id| id != 0),
            "parse_id returned Ok(0) for empty users array; see issue #3"
        );
    }

    // 此測試會打真實的 Quaver API，預設忽略以避免 CI 受網路/外部資料影響。
    // 需要時可用 `cargo test -- --ignored` 執行。
    #[tokio::test]
    #[ignore]
    async fn test_fetch_user_stats() {
        let id = User::fetch_id("tyrcs").await.unwrap();
        assert_eq!(id, 48618);

        let user = User::fetch_stat(id, Mode::Keys7).await.unwrap();
        assert_eq!(user.name, "tyrcs");
        assert_eq!(user.country, "CN");
        assert!(user.rating > 700.0);
        assert!(user.accuracy > 95.0);
    }
}
