use std::env;
use std::path::{Path, PathBuf};
use async_trait::async_trait;
use tokio::fs::{DirBuilder, File, OpenOptions, remove_dir_all};
use tokio::process::Command;


pub fn sanitize_path_prefix(path: &str) -> &str {
    if path.is_empty() {
        return path;
    }

    let mut chars = path.chars();
    let mut index = 0;

    while let Some(chr) = chars.next() {
        if chr == '/' || chr == '\\' {
            index += 1;
        } else {
            break;
        }
    }

    &path[index..]
}

pub fn ensure_absolute_path(input_path: &str) -> PathBuf {
    let path = Path::new(input_path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        let current_dir = env::current_dir().expect("failed to get current directory");
        current_dir.join(path)
    }
}


pub async fn get_parent_path(path: impl Into<PathBuf>) -> Result<PathBuf, String> {
    let file_path = path.into();

    let _file = File::open(&file_path).await.expect("无法获取文件路径！");

    if let Some(path) = file_path.parent() {
        return Ok(path.into());
    }

    Err("无法获取文件路径！".into())
}

pub async fn open_file(path: impl AsRef<Path>) -> File {
    OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(path)
        .await
        .expect("couldn't open file")
}
pub async fn create_dir(path: impl AsRef<Path>) {
    let path_ref = path.as_ref();
    if !path_ref.exists(){
        DirBuilder::new()
            .recursive(true)
            .create(path)
            .await
            .expect("couldn't create or open dir");
    }
}

#[async_trait]
pub trait HidePath {
    async fn hide_path(&self);
}

#[async_trait]
impl HidePath for PathBuf {
    async fn hide_path(&self) {
        let path_text = self.to_str()
            .expect("Couldn't found path");
        let path_buf = PathBuf::from(&path_text);
        if std::env::consts::OS == "windows" {
            let _ = match Command::new("attrib")
                .args(&["+H", &path_text])
                .status()
                .await {
                Ok(_) => {}
                Err(_) => {}
            };
        } else {
            let filename = path_buf.file_name()
                .expect( "not found file_name")
                .to_string_lossy();
            if !filename.starts_with(".") {
                let mut new_path_buf = path_buf.clone();
                new_path_buf.pop();
                new_path_buf.push(format!("{}{}", ".", filename));

                tokio::fs::rename(path_buf, new_path_buf)
                    .await
                    .expect("couldn't rename file");
            }
        }
    }
}

#[async_trait]
pub trait DeleteFolder {
    async fn delete(&self);
}

#[async_trait]
impl DeleteFolder for Option<PathBuf> {
    async fn delete(&self) {
        if let Some(value) = self {
            match remove_dir_all(value).await {
                Ok(_) => {}
                Err(e) => { eprintln!("{}", e.to_string()) }
            }
        }
    }
}

#[async_trait]
impl DeleteFolder for PathBuf {
    async fn delete(&self) {
        match remove_dir_all(self).await {
            Ok(_) => {}
            Err(e) => { eprintln!("{}", e.to_string()) }
        }
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;
    use crate::utils::{create_dir, HidePath, sanitize_path_prefix};

    #[test]
    fn test_sanitize() {
        let parsed_text = "Book/Literature Books";
        let raw_text = "////Book/Literature Books";

        assert_eq!(sanitize_path_prefix(raw_text), parsed_text)
    }

    #[tokio::test]
    async fn test_hide_path() {
        let path_text = "./target/test";
        create_dir(path_text).await;
        let path_buf = PathBuf::from(path_text);
        path_buf.hide_path().await;
    }

}