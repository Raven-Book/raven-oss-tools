use std::env;
use raven_oss_tools::client::AliyunOssCommandExecutor;

#[tokio::main]
async fn main() -> Result<(), String>{
    let args: Vec<String> = env::args().collect();
    let mut client = match AliyunOssCommandExecutor::new().await {
        Some(value) => value,
        None => {
            println!("已在~/.config/rot/内初始化配置文件，请填写rot.json。");
            std::process::exit(0)
        }
    };
    client.execute(args).await
}
