use reqwest::Error;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub global_rank: u32,
    pub country_rank: u32,
    pub country: String,
    pub rating: f32,
    pub accuracy: f32,
}

impl User {
    pub fn new() -> Self {
        User {
            id: String::new(),
            name: String::new(),
            global_rank: 0,
            country_rank: 0,
            country: String::new(),
            rating: 0.0,
            accuracy: 0.0,
        }
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub async fn fetch_id(name: &str) -> Result<(), Error> {
        let url = format!("https://api.quavergame.com/v2/user/search/{}", name);

        Ok(())
    }

    pub async fn fetch_stat(id: &str) -> Result<(), Error> {
        let url = format!("https://api.quavergame.com/v2/user/{}", id);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_id() {
        let user = User::fetch_id("tyrcs").await.unwrap();
        assert_eq!(user.id, "48618");
        assert_eq!(user.name, "tyrcs");
        assert_eq!(user.country, "CN");
    }
}
