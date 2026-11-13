#[tokio::main]
async fn main() {
    let code = myshop::management::execute().await;
    std::process::exit(i32::from(code));
}
