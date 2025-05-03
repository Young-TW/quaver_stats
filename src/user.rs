use reqwest::Error;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct User {
    pub name: String,
    pub global_rank: u64,
    pub country_rank: u64,
    pub country: String,
    pub rating: f64,
    pub accuracy: f64,
    pub avatar_url: String, // 新增欄位
}

impl User {
    pub async fn fetch_id(name: &str) -> Result<u64, Error> {
        #[derive(Debug, Deserialize)]
        struct UserSearchResponse {
            users: Vec<RawUser>,
        }

        #[derive(Debug, Deserialize)]
        struct RawUser {
            id: u64,
        }

        let url = format!("https://api.quavergame.com/v2/user/search/{}", name);
        let response = reqwest::get(&url).await?;
        let result: UserSearchResponse = response.json().await?;
        Ok(result.users.first().map(|u| u.id).unwrap_or(0))
    }

    pub async fn fetch_stat(id: u64) -> Result<User, Error> {
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

        let url = format!("https://api.quavergame.com/v2/user/{}", id);
        let response = reqwest::get(&url).await?;
        let result: UserResponse = response.json().await?;

        Ok(User {
            name: result.user.username,
            country: result.user.country,
            global_rank: result.user.stats.ranks.global,
            country_rank: result.user.stats.ranks.country,
            rating: result.user.stats.performance,
            accuracy: result.user.stats.accuracy,
            avatar_url: result.user.avatar_url, // 提取 avatar_url
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
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
