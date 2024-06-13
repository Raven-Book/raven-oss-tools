use std::{env, io};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use async_trait::async_trait;
use tokio::fs::{DirBuilder, File, OpenOptions, remove_dir_all};
use tokio::io::AsyncReadExt;
use tokio::process::Command;

#[cfg(test)]
#[macro_export]
macro_rules! println_in_test {
    ($($arg:tt)*) => (println!($($arg)*));
}

#[cfg(not(test))]
#[macro_export]
macro_rules! println_in_test {
    ($($arg:tt)*) => {};
}

pub fn sanitize_prefix_path(path: &str) -> &str {
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

pub fn append_slash(path: &mut String) {
    if !(path.ends_with('/') || path.ends_with('\\')) && path.len() > 0{
        path.push('/');
    }
}

pub fn ensure_absolute_path(input_path: &str) -> Result<PathBuf, String> {
    let path = Path::new(input_path);
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        let current_dir = match env::current_dir() {
            Ok(p) => p.join(path),
            Err(err) => return Err(err.to_string())
        };
        Ok(current_dir)
    }
}


pub async fn get_parent_path(path: impl Into<PathBuf>) -> Result<PathBuf, String> {
    let file_path = path.into();

    if let Some(path) = file_path.parent() {
        return Ok(path.into());
    }

    Err("无法获取文件路径！".into())
}

pub async fn create_file(path: impl AsRef<Path>) -> io::Result<File> {
    OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(path)
        .await
}

pub async fn create_dir(path: impl AsRef<Path>) -> io::Result<()> {
    let path_ref = path.as_ref();
    if !path_ref.exists() {
        return DirBuilder::new()
            .recursive(true)
            .create(path)
            .await;
    }
    Ok(())
}

pub struct FileChunkIterator {
    file: File,
    file_size: usize,
    chunk_size: usize,
    original_file_size: usize,
}

impl FileChunkIterator {
    pub async fn new(file: File, chunk_size: usize) -> io::Result<Self> {
        let metadata = file.metadata().await?;
        let file_size = metadata.len() as usize;
        let original_file_size = file_size;
        Ok(Self {
            file,
            file_size,
            chunk_size,
            original_file_size,
        })
    }
    pub async fn read_chunk(&mut self) -> tokio::io::Result<Option<Vec<u8>>> {
        if self.file_size == 0 {
            return Ok(None);
        }

        let to_read =
            if self.file_size >= self.chunk_size {
                self.chunk_size
            } else {
                self.file_size
            };

        let mut buffer = vec![0; to_read];

        let bytes_read = self.file.read_exact(&mut buffer).await?;
        self.file_size -= to_read;

        if bytes_read == 0 {
            return Ok(None);
        }

        Ok(Some(buffer))
    }

    pub fn get_chunk_size(&self) -> usize {
        self.chunk_size
    }

    pub fn get_file_size(&self) -> usize {
        self.file_size
    }

    pub fn get_original_file_size(&self) -> usize {
        self.original_file_size
    }
}

pub trait UnwrapOrExit<T> {
    fn unwrap_or_exit(self, error_message: impl Into<String>) -> T;
}

impl<T> UnwrapOrExit<T> for Option<T> {
    fn unwrap_or_exit(self, error_message: impl Into<String>) -> T {
        let error_message_text = error_message.into();
        match self {
            Some(value) => value,
            None => {
                println!("{}", error_message_text);
                std::process::exit(1);
            }
        }
    }
}

impl<T, E> UnwrapOrExit<T> for Result<T, E>
    where
        E: std::fmt::Debug
{
    fn unwrap_or_exit(self, error_message: impl Into<String>) -> T {
        let error_message_text = error_message.into();
        match self {
            Ok(value) => value,
            Err(err) => {
                println!("{}: {:?}", error_message_text, err);
                std::process::exit(1);
            }
        }
    }
}


#[async_trait]
pub(crate) trait HidePath {
    async fn hide_path(&self) -> io::Result<()>;
}

#[async_trait]
impl HidePath for PathBuf {
    async fn hide_path(&self) -> io::Result<()> {
        let path_text = self.to_string_lossy().to_string();
        let path_buf = self.clone();
        if env::consts::OS == "windows" {
            let _ = match Command::new("attrib")
                .args(&["+H", &path_text])
                .status()
                .await {
                Ok(_) => {}
                Err(err) => { return Err(err); }
            };
        } else {
            let filename = match path_buf.file_name() {
                Some(f) => f.to_string_lossy(),
                None => {
                    return Err(string_to_error("not found filename"));
                }
            };
            if !filename.starts_with(".") {
                let mut new_path_buf = path_buf.clone();
                new_path_buf.pop();
                new_path_buf.push(format!("{}{}", ".", filename));

                if let Err(err) = tokio::fs::rename(path_buf, new_path_buf).await {
                    return Err(err);
                }
            }
        }

        Ok(())
    }
}

fn string_to_error(s: impl Into<String>) -> io::Error {
    let text = s.into();
    io::Error::new(ErrorKind::Other, text)
}

#[async_trait]
pub(crate) trait DeleteFolder {
    async fn delete(&self) -> io::Result<()>;
}

#[async_trait]
impl DeleteFolder for Option<PathBuf> {
    async fn delete(&self) -> io::Result<()> {
        if let Some(value) = self {
            if let Err(err) = remove_dir_all(value).await {
                return Err(err);
            }
        }
        Ok(())
    }
}

#[async_trait]
impl DeleteFolder for PathBuf {
    async fn delete(&self) -> io::Result<()> {
        if let Err(err) = remove_dir_all(self).await {
            return Err(err);
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;
    use crate::utils::{create_dir, HidePath, sanitize_prefix_path};

    #[test]
    fn test_sanitize() {
        let parsed_text = "Book/Literature Books";
        let raw_text = "////Book/Literature Books";

        assert_eq!(sanitize_prefix_path(raw_text), parsed_text)
    }

    #[tokio::test]
    async fn test_hide_path() {
        let path_text = "./target/test";
        create_dir(path_text).await.unwrap();
        let path_buf = PathBuf::from(path_text);
        path_buf.hide_path().await.unwrap();
    }
}