//! Runs the shared report scenario against one disposable database.
//!
//! SQLite URLs are accepted. A PostgreSQL URL must contain `ferrioxid_test`,
//! so this binary does not open the populated `ferrioxid` database.

mod scenario;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::args()
        .nth(1)
        .ok_or("pass a sqlite URL or a disposable postgres URL containing ferrioxid_test")?;
    if url.starts_with("postgres://") || url.starts_with("postgresql://") {
        if !url.contains("ferrioxid_test") {
            return Err(
                "refusing a PostgreSQL URL that is not a disposable ferrioxid_test database".into(),
            );
        }
    } else if !url.starts_with("sqlite:") {
        return Err("database URL must start with sqlite: or postgres:".into());
    }
    let db = ferrioxid_db::connect(&url).await?;
    scenario::exercise(&db).await?;
    println!("grouped report ok");
    Ok(())
}
