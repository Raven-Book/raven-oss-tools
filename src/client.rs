use std::borrow::Cow;
use std::option::Option;
use std::path::{PathBuf};
use std::sync::{Arc, Mutex};
use aws_config::{BehaviorVersion, Region, SdkConfig};
use aws_sdk_s3::{Client, config};
use aws_sdk_s3::config::{Credentials, SharedCredentialsProvider};
use aws_sdk_s3::operation::list_objects_v2::ListObjectsV2Output;
use aws_sdk_s3::operation::put_object::PutObjectOutput;
use aws_sdk_s3::primitives::{ByteStream, DateTime};
use serde::{Deserialize, Serialize};
use tokio::fs::{DirBuilder, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::command::{CommandRegistry};
use crate::constant::TEMP_FOLDER;
use crate::crypt::encrypt_file;
use crate::handler;
use crate::parser::{CommandParser};
use crate::utils::{create_dir, DeleteFolder, get_parent_path, open_file};

#[derive(Debug)]
pub struct AliyunClient {
    client: Client,
    bucket: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Config {
    access_key_id: String,
    secret_access_key: String,
    region: String,
    endpoint_url: String,
    bucket: String,
}

pub struct AliyunOssCommandExecutor {
    client: Arc<Mutex<AliyunClient>>,
    registry: CommandRegistry,
}

impl Config {
    pub fn new_empty() -> Self {
        Config {
            access_key_id: "".into(),
            secret_access_key: "".into(),
            region: "".into(),
            endpoint_url: "".into(),
            bucket: "".into(),
        }
    }

    pub fn is_valid(&self) -> bool {
        !(self.access_key_id.is_empty()
            || self.secret_access_key.is_empty()
            || self.region.is_empty()
            || self.endpoint_url.is_empty()
            || self.bucket.is_empty())
    }
}

impl AliyunClient {
    pub async fn load_from_env() -> Option<Self> {
        let home_path = match home::home_dir() {
            Some(path) => path,
            None => {
                eprintln!("Impossible to get your home dir!");
                return None;
            }
        };

        let path_str = home_path.to_str().unwrap();

        let file_prefix_path = format!("{}/.config/rot/", path_str);
        let filename = "rot.json";
        DirBuilder::new()
            .recursive(true)
            .create(&file_prefix_path).await.expect("Couldn't create or open dir.");

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(format!("{}{}", &file_prefix_path, filename))
            .await
            .expect("Couldn't create or open file.");

        let mut text = String::new();
        file.read_to_string(&mut text).await.expect("Couldn't read file.");

        if text.is_empty() {
            let config_text = serde_json::to_string(&Config::new_empty()).expect("Couldn't serialize.");
            println!("is empty: {}", &config_text);
            file.write_all(config_text.as_bytes()).await.expect("TODO: panic message");
            return None;
        } else {
            let result = serde_json::from_str::<Config>(&text);
            let config: Option<Config> = match result {
                Ok(value) => {
                    Some(value)
                }
                Err(_) => {
                    let config_text = serde_json::to_string(&Config::new_empty()).expect("Couldn't serialize.");
                    file.write_all(config_text.as_bytes()).await.expect("TODO: panic message");
                    None
                }
            };

            if config.is_none() {
                println!("Configuration is missing.");
                return None;
            } else if let Some(value) = config {
                if value.is_valid() {
                    return Some(Self::new(
                        value.access_key_id,
                        value.secret_access_key,
                        value.endpoint_url,
                        value.region,
                        value.bucket,
                    ));
                }
            }
        }
        None
    }

    pub fn new(access_key_id: impl Into<String>,
               secret_access_key: impl Into<String>,
               endpoint_url: impl Into<String>,
               region: impl Into<Cow<'static, str>>,
               bucket: impl Into<String>,
    ) -> Self {
        let client = AliyunClient::build_aws_client(access_key_id, secret_access_key, endpoint_url, region);
        Self {
            client,
            bucket: bucket.into(),
        }
    }

    pub async fn list_obj(&self,
                          max_keys: Option<i32>,
                          prefix_path: Option<String>,
                          token: Option<String>) -> ListObjectsV2Output {
        let mut res = self.client.list_objects_v2()
            .bucket(&self.bucket);


        if let Some(value) = max_keys {
            res = res.max_keys(value)
        }

        if let Some(value) = prefix_path {
            res = res.prefix(value)
        }

        if let Some(value) = token {
            res = res.continuation_token(value)
        }

        let resp = res.send().await.expect("Request Error by list object.");

        resp
    }

    pub async fn upload_file(&self,
                             key: impl Into<String>,
                             input_path: PathBuf,
                             password: Option<impl Into<String>>,
                             expiry_seconds: Option<i64>) -> Result<PutObjectOutput, String> {
        let mut delete_path: Option<PathBuf> = None;

        let filename = match input_path.file_name() {
            Some(file_name) => file_name.to_string_lossy(),
            None => {
                return Err("couldn't get filename！".into());
            }
        };

        let content =
            if let Some(pwd) = password {

                let mut output_path = match get_parent_path(&input_path).await {
                    Ok(value) => value,
                    Err(e) => { return Err(e); }
                };

                output_path.push(TEMP_FOLDER);
                create_dir(&output_path).await;
                output_path.push(filename.to_string());

                encrypt_file(&input_path, &output_path, pwd).await.expect("failed to encrypt file.");
                let bs = ByteStream::from_path(&output_path).await.expect("not found file");
                output_path.pop();
                delete_path = Some(output_path);
                bs
            } else {
                ByteStream::from_path(&input_path).await.expect("not found file")
            };

        let mut prefix_key = key.into();

        if !(prefix_key.ends_with('/') || prefix_key.ends_with("\\")) {
            if prefix_key.len() > 1 {
                prefix_key.push('/');
            } else if prefix_key.len() == 1 {
                prefix_key.clear()
            }
        }


        let mut upload = self.client.put_object()
            .bucket(&self.bucket)
            .key(format!("{}{}", prefix_key, filename))
            .body(content);

        if let Some(value) = expiry_seconds {
            let expiry_time = DateTime::from_secs(value);
            upload = upload.expires(expiry_time);
        }

        let resp = match upload.send().await {
            Ok(value) => {
                delete_path.delete().await;
                value
            }
            Err(_) => {
                delete_path.delete().await;
                return Err("request error by put object".into());
            }
        };


        Ok(resp)
    }

    pub async fn download_file(&self, key: impl Into<String>, path: &PathBuf) {
        let resp = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await.unwrap();

        let data = resp.body.collect().await.unwrap();
        let bytes = data.into_bytes();

        let mut file = open_file(path).await;

        let _ = file.write(&*bytes).await.unwrap();
        file.flush().await.unwrap();
        drop(file);
    }

    fn build_aws_client(access_key_id: impl Into<String>,
                        secret_access_key: impl Into<String>,
                        endpoint_url: impl Into<String>,
                        region: impl Into<Cow<'static, str>>) -> Client {
        let sdk_config = SdkConfig::builder().credentials_provider(
            SharedCredentialsProvider::new(
                Credentials::new(
                    access_key_id,
                    secret_access_key,
                    None,
                    None,
                    "static",
                )
            ))
            .endpoint_url(endpoint_url)
            .region(Region::new(region))
            .behavior_version(BehaviorVersion::latest())
            .build();

        let s3_config_builder = config::Builder::from(&sdk_config);
        let client = Client::from_conf(s3_config_builder.build());
        client
    }
}


impl AliyunOssCommandExecutor {
    pub async fn new() -> Option<Self> {
        let client = AliyunClient::load_from_env().await;

        if client.is_none() {
            return None;
        }

        let mut executor = Self {
            client: Arc::new(Mutex::new(client.unwrap())),
            registry: CommandRegistry::new(),
        };
        executor.init();
        Some(executor)
    }

    pub async fn execute(&mut self, args: impl IntoIterator<Item=impl Into<String>>) -> Result<(), String> {
        let args = CommandParser::from_strings(args);
        self.registry.execute(args).await
    }

    pub fn init(&mut self) {
        self.registry.register("list", handler::get_obj_names(Arc::clone(&self.client)));
        self.registry.register("upload", handler::upload_file(Arc::clone(&self.client)));
        self.registry.register("download", handler::download_file(Arc::clone(&self.client)));
    }
}

#[cfg(test)]
mod test {
    use crate::client::{Config};

    #[test]
    fn test_config_serialize() {
        let config = Config::new_empty();
        let json = serde_json::to_string(&config).expect("Couldn't serialize config struct.");
        assert_eq!(json, "{\"access_key_id\":\"\",\"secret_access_key\":\"\",\"region\":\"\",\"endpoint_url\":\"\",\"bucket\":\"\"}")
    }
}





