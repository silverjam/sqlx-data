use sqlx_data::{Pool, Result, dml, repo};

#[repo]
trait TodoAppRepo {
    #[dml("SELECT 1")]
    async fn ping(&self, pool: &Pool) -> Result<i32>;
}
pub struct MyPoolApp {}

impl TodoAppRepo for MyPoolApp {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = Pool::connect(":memory:").await?;

    let todo_repo = MyPoolApp {};
    let ping = todo_repo.ping(&pool).await?;
    println!("Hello, world! {}", ping);
    Ok(())
}
