use std::path::{PathBuf};
use async_trait::async_trait;
use tokio::fs::{File, remove_dir_all};

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

pub async fn get_file_directory(path: impl Into<PathBuf>) -> Result<PathBuf, String> {
    let file_path = path.into();

    let _file = File::open(&file_path).await.expect("无法获取文件路径！");

    if let Some(path) = file_path.parent() {
        return Ok(path.into());
    }

    Err("无法获取文件路径！".into())
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

#[cfg(test)]
mod test {
    use crate::utils::{sanitize_path_prefix};

    #[test]
    fn test_sanitize() {
        let parsed_text = "Book/Literature Books";
        let raw_text = "////Book/Literature Books";

        assert_eq!(sanitize_path_prefix(raw_text), parsed_text)
    }

    // #[tokio::test]
    // async fn test_delete_path() {
    //     let path = Some(PathBuf::from("D:/.raven_tmp/example.txt"));
    //     path.delete().await;
    // }
}