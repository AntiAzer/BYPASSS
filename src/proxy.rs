use reqwest::{Client, Proxy};

pub async fn check_proxy(addr: &str) -> bool {
    let client = Client::builder().proxy(Proxy::all(format!("http://{}", addr)).unwrap()).build().unwrap();
    match client.get("http://example.com").send().await {
        Ok(response) => {
            if response.status() == 200 {
                println!("[+] working proxy - {:?}", addr);
                return true;
            }
        }

        Err(_) => {
            return false;
        }
    }
    false
}