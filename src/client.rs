use std::borrow::Cow;
use std::option::Option;
use std::path::{PathBuf};
use aws_config::{BehaviorVersion, Region, SdkConfig};
use aws_sdk_s3::{Client, config};
use aws_sdk_s3::config::{Credentials, SharedCredentialsProvider};
use aws_sdk_s3::operation::complete_multipart_upload::{CompleteMultipartUploadOutput};
use aws_sdk_s3::operation::list_objects_v2::ListObjectsV2Output;
use aws_sdk_s3::primitives::{ByteStream};
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use serde::{Deserialize, Serialize};
use tokio::fs::{DirBuilder, OpenOptions, File};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::constant::{CHUNK_SIZE, CHUNK_SIZE_WITH_TAG};
use crate::utils::{create_file, FileChunkIterator, UnwrapOrExit};

pub(crate) type Operation = Box<dyn Fn(&Vec<u8>) -> Vec<u8>>;

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

impl Config {
    pub fn new_empty() -> Self {
        Config {
            access_key_id: String::new(),
            secret_access_key: String::new(),
            region: String::new(),
            endpoint_url: String::new(),
            bucket: String::new(),
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
            file
                .write_all(config_text.as_bytes())
                .await
                .expect("TODO: panic message");
            return None;
        }

        let result = serde_json::from_str::<Config>(&text);
        let config: Option<Config> = match result {
            Ok(value) => {
                Some(value)
            }
            Err(_) => {
                let config_text = serde_json::to_string(&Config::new_empty()).expect("Couldn't serialize.");
                file
                    .write_all(config_text.as_bytes())
                    .await
                    .expect("TODO: panic message");
                None
            }
        };

        if let Some(value) = config {
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
                             operation: Option<Operation>) -> Result<CompleteMultipartUploadOutput, String> {
        let mut part_number = 0;
        let mut upload_parts = Vec::new();
        let key_text = key.into();

        let filename = match input_path.file_name() {
            Some(f) => f.to_string_lossy(),
            None => {
                return Err("failed to get filename".into());
            }
        };


        let file = File::open(&input_path)
            .await
            .unwrap_or_exit(format!("无法打开文件`{}`", filename));

        let multipart_res = self.client
            .create_multipart_upload()
            .bucket(&self.bucket)
            .key(&key_text)
            .send()
            .await.unwrap_or_exit("上传时出现错误");

        let upload_id = multipart_res.upload_id().unwrap_or_exit("获取 Upload Id 失败");
        let mut iter = FileChunkIterator::new(file, CHUNK_SIZE)
            .await
            .unwrap_or_exit("FileChunkIterator 创建失败");

        while let Some(buffer) = iter.read_chunk()
            .await
            .unwrap_or_exit("文件读取失败") {
            let write_buffer =
                if let Some(operation_fn) = &operation {
                    operation_fn(&buffer)
                } else {
                    buffer
                };

            let stream = ByteStream::from(write_buffer);
            part_number += 1;

            let upload_part_res = self
                .client
                .upload_part()
                .bucket(&self.bucket)
                .key(&key_text)
                .upload_id(upload_id)
                .body(stream)
                .part_number(part_number)
                .send()
                .await
                .unwrap_or_exit("上传时出现错误");

            let completer_part = CompletedPart::builder()
                .e_tag(upload_part_res.e_tag.unwrap_or_default())
                .part_number(part_number)
                .build();

            upload_parts.push(completer_part);
        }

        let completed_multipart_upload =
            CompletedMultipartUpload::builder()
                .set_parts(Some(upload_parts))
                .build();

        let completed_multipart_upload_res = self
            .client
            .complete_multipart_upload()
            .bucket(&self.bucket)
            .key(&key_text)
            .multipart_upload(completed_multipart_upload)
            .upload_id(upload_id)
            .send()
            .await
            .unwrap_or_exit("合并文件时出现异常");

        Ok(completed_multipart_upload_res)
    }

    pub async fn download_file(&self,
                               key: impl Into<String>,
                               path: &PathBuf,
                               operation: Option<Operation>)
    {
        let resp = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await.unwrap();

        let mut file = create_file(path)
            .await
            .unwrap_or_exit("文件读取失败");

        let content_len = resp
            .content_length()
            .unwrap_or_exit("无法获取文件大小，请检查网络连接");

        let mut byte_stream_async_reader = resp.body.into_async_read();
        let mut content_len_usize: usize = content_len
            .try_into()
            .unwrap_or_exit("文件长度非法");
        loop {
            if content_len_usize > CHUNK_SIZE_WITH_TAG {
                let mut buffer = vec![0; CHUNK_SIZE_WITH_TAG];
                let _ = byte_stream_async_reader
                    .read_exact(&mut buffer)
                    .await
                    .unwrap_or_exit("下载时出现异常");

                let write_buffer =
                    if let Some(operation_fn) = &operation {
                        operation_fn(&buffer)
                    } else {
                        buffer
                    };

                file.write_all(&write_buffer)
                    .await
                    .unwrap_or_exit("下载时出现异常");
                content_len_usize -= CHUNK_SIZE_WITH_TAG;
                continue;
            } else {
                let mut buffer = vec![0; content_len_usize];
                let _ = byte_stream_async_reader
                    .read_exact(&mut buffer)
                    .await
                    .unwrap_or_exit("下载时出现异常");

                let write_buffer =
                    if let Some(operation_fn) = &operation {
                        operation_fn(&buffer)
                    } else {
                        buffer
                    };

                file.write_all(&write_buffer)
                    .await
                    .unwrap_or_exit("下载时出现异常");
                break;
            }
        }
        file.flush().await.unwrap_or_exit("下载时出现异常");
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





