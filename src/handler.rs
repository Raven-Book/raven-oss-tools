use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use crate::client::AliyunClient;
use crate::command::CommandHandler;
use crate::parser::Arguments;
use crate::utils::sanitize_path_prefix;

pub fn upload_file(client: Arc<Mutex<AliyunClient>>) -> CommandHandler {
    Box::new(move |args: Arguments| -> Pin<Box<dyn Future<Output=Result<(), String>>>> {
        let client_clone = Arc::clone(&client);
        Box::pin(async move {
            if args.positional.len() < 1 {
                return Err("请输入正确的文件路径！".into());
            }

            let file_path = args.positional.get(0).unwrap();
            let mut upload_path = String::from("");
            let mut expiry_seconds: Option<i64> = None;
            let mut password: Option<String> = None;

            if let Some(value) = args.optional.get("u") {
                upload_path.push_str(sanitize_path_prefix(value));
            }

            if let Some(value) = args.optional.get("p") {
                password = Some(value.into())
            }

            if let Some(value) = args.optional.get("t") {
                expiry_seconds = Some(match value.parse() {
                    Ok(n) => n,
                    Err(_) => {
                        return Err(format!("无法将 `-t` 参数的值 '{}' 解析为整数，请确保你提供的是一个有效的整数值。", value));
                    }
                });
            }

            let resp = client_clone.lock().unwrap().upload_file(file_path,
                                                                upload_path,
                                                                password,
                                                                expiry_seconds).await.expect("文件上传失败");
            if let Some(e_tag) = resp.e_tag() {
                println!("文件上传成功！ETag: {}。", e_tag);
            } else {
                eprintln!("文件上传失败！");
            }
            Ok(())
        })
    })
}

pub fn get_obj_names(client: Arc<Mutex<AliyunClient>>) -> CommandHandler {
    Box::new(move |args: Arguments| -> Pin<Box<dyn Future<Output=Result<(), String>>>> {
        let client_clone = Arc::clone(&client);
        Box::pin(async move {
            let mut prefix_path: Option<String> = None;
            let mut max_keys: Option<i32> = None;

            if let Some(value) = args.optional.get("u") {
                prefix_path = Some(value.into());
            }

            if let Some(value) = args.optional.get("m") {
                max_keys = Some(match value.parse() {
                    Ok(n) => n,
                    Err(_) => {
                        return Err(format!("无法将 `-m` 参数的值 '{}' 解析为整数，请确保你提供的是一个有效的整数值。", value));
                    }
                });
            }

            let resp = client_clone.lock().unwrap().list_obj(max_keys, prefix_path, None).await;
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
                    return Ok(());
                }
            }
            Ok(())
        })
    })
}