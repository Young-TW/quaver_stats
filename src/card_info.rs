pub struct CardInfo {
    username: String,
    user_global_rank: u32,
    user_country_rank: u32,
    country: String,
    user_rating: f32,
    user_accuracy: f32,
}

impl CardInfo {
    pub fn new(
        username: String,
        global_rank: u32,
        country_rank: u32,
        country: String,
        rating: f32,
        accuracy: f32,
    ) -> Self {
        CardInfo {
            username,
            user_global_rank: global_rank,
            user_country_rank: country_rank,
            country,
            user_rating: rating,
            user_accuracy: accuracy,
        }
    }
}
