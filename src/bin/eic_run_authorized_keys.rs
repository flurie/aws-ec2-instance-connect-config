use tokio::time::{timeout, Duration};

#[tokio::main]
async fn main() {
    async fn long_future() {
        // do work here
    }

    let res = timeout(Duration::from_secs(5), long_future()).await;

    if res.is_err() {
        println!("operation timed out");
    }
}
