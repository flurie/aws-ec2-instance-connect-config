use aws_config::imds::client::Client;

#[tokio::main]
async fn main() {
    let client = Client::builder().build();
    let ami_id = client
        .get("/latest/meta-data/ami-id")
        .await
        .expect("failure communicating with IMDS");
    println!("{:?}", ami_id)
}
