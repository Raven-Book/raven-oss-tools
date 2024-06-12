use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use clap::{Parser, Subcommand};
use ring::aead::chacha20_poly1305_openssh::TAG_LEN;
use raven_oss_tools::client::{AliyunClient};
use raven_oss_tools::crypt::{decrypt, encrypt, setup_key};
use raven_oss_tools::utils::{append_slash, create_dir, ensure_absolute_path, sanitize_prefix_path, UnwrapOrExit};

#[derive(Parser)]
#[command(version, author, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    rot: Option<Rot>,
}

#[derive(Subcommand)]
enum Rot {
    Upload {
        path: String,
        #[arg(short)]
        password: Option<String>,
        #[arg(short, long)]
        prefix_path: Option<String>,
    },
    Download {
        remote_path: String,
        local_path: Option<String>,
        #[arg(short)]
        password: Option<String>,
    },
    Ls {
        #[arg(short, long)]
        prefix_path: Option<String>,
        #[arg(short, long)]
        max_length: Option<i32>,
    },
}

pub(crate) struct RotDownload {
    remote_path: String,
    local_path: Option<String>,
    password: Option<String>,
}

pub(crate) struct RotUpload {
    path: String,
    password: Option<String>,
    prefix_path: Option<String>,
}

pub(crate) struct RotList {
    prefix_path: Option<String>,
    max_length: Option<i32>,
}

async fn download_file(rot_download: RotDownload, client: Arc<Mutex<AliyunClient>>) {
    let key_path = PathBuf::from(&rot_download.remote_path);

    let filename = key_path.file_name()
        .expect("failed to get filename")
        .to_string_lossy()
        .to_string();


    let mut download_path =
        if let Some(o) = rot_download.local_path {
            ensure_absolute_path(&o)
                .unwrap_or_exit("下载时出现异常")
        } else {
            env::current_dir().expect("failed to get file")
        };
    create_dir(&download_path)
        .await
        .unwrap_or_exit("创建文件夹时出现异常");
    download_path.push(&filename);


    let has_password = !rot_download.password.is_none();
    if has_password {
        let less_safe_key = Arc::new(setup_key(&rot_download.password.unwrap()));
        let less_safe_key_clone = Arc::clone(&less_safe_key);
        client.lock()
            .unwrap_or_exit("获取 client 失败")
            .download_file(&rot_download.remote_path,
                           &download_path,
                           Some(Box::new(
                               move |buffer: &Vec<u8>| {
                                   let result = decrypt(&*buffer, &less_safe_key_clone).unwrap_or_exit("解密时失败");
                                   result[..result.len() - TAG_LEN].to_vec()
                               }))).await;
    } else {
        client.lock()
            .unwrap_or_exit("获取 client 失败")
            .download_file(&rot_download.remote_path, &download_path, None).await;
    }

    println!("文件下载成功！所在路径：{}。", download_path.to_string_lossy());
}

async fn upload_file(rot_upload: RotUpload, client: Arc<Mutex<AliyunClient>>) {
    let local_path = ensure_absolute_path(&rot_upload.path).unwrap_or_exit("无效的路径");

    let mut prefix_key: String = String::new();

    if let Some(value) = rot_upload.prefix_path {
        prefix_key.push_str(sanitize_prefix_path(&value));
    }

    append_slash(&mut prefix_key);

    let filename = local_path
        .file_name()
        .unwrap_or_exit("无法获取文件名")
        .to_string_lossy();

    let key = format!("{}{}", prefix_key, filename);

    let has_password = !rot_upload.password.is_none();
    let resp = if has_password {
        let less_safe_key = Arc::new(setup_key(&rot_upload.password.unwrap()));
        let less_safe_key_clone = Arc::clone(&less_safe_key);
        client.lock().unwrap().upload_file(
            key,
            local_path,
            Some(Box::new(
                move |buffer: &Vec<u8>| {
                    encrypt(&*buffer, &less_safe_key_clone).unwrap_or_exit("文件加密时失败")
                })),
        )
            .await
            .expect("failed to upload file")
    } else {
        client.lock().unwrap().upload_file(
            key,
            local_path,
            None,
        )
            .await
            .expect("failed to upload file")
    };


    if let Some(e_tag) = resp.e_tag() {
        println!("文件上传成功！ETag: {}。", e_tag);
    } else {
        println!("文件上传失败！");
    }
}

async fn list(rot_list: RotList, client: Arc<Mutex<AliyunClient>>) {
    let mut prefix_path: Option<String> = None;

    if let Some(value) = rot_list.prefix_path {
        prefix_path = Some(sanitize_prefix_path(&value).to_string())
    }

    let resp = client.lock().unwrap().list_obj(rot_list.max_length, prefix_path, None).await;
    match resp.contents {
        Some(objs) => {
            for (index, obj) in objs.iter().enumerate() {
                if let Some(key) = &obj.key {
                    println!("{}: {:?}", index + 1, key);
                }
            }
        }
        None => {
            println!("该路径下不存在文件！");
        }
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Some(rot) = cli.rot {
        let client = match AliyunClient::load_from_env().await {
            Some(value) => value,
            None => {
                println!("已在~/.config/rot/内初始化配置文件，请填写rot.json。");
                std::process::exit(0)
            }
        };

        let client_arc = Arc::new(Mutex::new(client));

        match rot {
            Rot::Download { remote_path, local_path, password } => {
                download_file(RotDownload {
                    remote_path,
                    local_path,
                    password,
                }, client_arc.clone()).await;
            }
            Rot::Upload { path, password, prefix_path } => {
                upload_file(RotUpload {
                    path,
                    password,
                    prefix_path,
                }, client_arc.clone()).await;
            }
            Rot::Ls { prefix_path, max_length} => {
                list(RotList {
                    prefix_path,
                    max_length,
                }, client_arc.clone()).await;
            }
        }
    }
}
