use Quaver_Stats::user::User;

#[tokio::main]
async fn main() {
    let mut player: User = User::new();
    player.set_name("tyrcs".to_string());

    let id = User::fetch_id(&player.name).await.unwrap();
    let fetched = User::fetch_stat(id).await.unwrap();

    println!("{:#?}", fetched);
}
