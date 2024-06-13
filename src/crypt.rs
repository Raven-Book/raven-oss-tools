use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use ring::aead::{Aad, AES_256_GCM, LessSafeKey, Nonce, UnboundKey};
use ring::aead::chacha20_poly1305_openssh::TAG_LEN;
use ring::error::Unspecified;
use ring::pbkdf2;
use tokio::fs::File;
use tokio::io;
use tokio::io::AsyncWriteExt;
use crate::constant::{AAD, CHUNK_SIZE, CHUNK_SIZE_WITH_TAG, NONCE, SALT};
use crate::println_in_test;
use crate::utils::{FileChunkIterator, UnwrapOrExit};

async fn process_file(input_path: impl AsRef<Path>,
                      output_path: impl AsRef<Path>,
                      chunk_size: usize,
                      password: impl Into<String>,
                      operation: fn(&LessSafeKey, &[u8]) -> Vec<u8>) -> io::Result<()> {
    let mut iter = FileChunkIterator::new(File::open(input_path)
                                              .await
                                              .unwrap_or_exit("文件读取失败"), chunk_size)
        .await
        .unwrap_or_exit("FileChunkIterator 创建失败");
    let mut output_file = File::create(output_path).await?;
    let less_safe_key = setup_key(password);
    while let Some(buffer) = iter.read_chunk()
        .await
        .unwrap_or_exit("文件读取失败") {

        println_in_test!("文件大小: {}; 待读取: {}; 当前次数: {};"
            ,iter.get_original_file_size(),
            iter.get_file_size(),
            iter.get_original_file_size().div_ceil(iter.get_chunk_size()) - iter.get_file_size().div_ceil(iter.get_chunk_size()));

        let processed_data = operation(&less_safe_key, &buffer);
        output_file.write_all(&processed_data).await?;
    }
    Ok(())
}

pub async fn decrypt_file(input_path: impl AsRef<Path>,
                      output_path: impl AsRef<Path>,
                      password: impl Into<String>) {
    process_file(input_path,
                 output_path,
                 CHUNK_SIZE_WITH_TAG,
                 password,
                 |less_safe_key, buffer: &[u8]| {
                     let result = decrypt(&*buffer, less_safe_key).unwrap_or_exit("解密时失败");
                     result[..result.len() - TAG_LEN].to_vec()
                 }).await
        .unwrap_or_exit("文件解密失败");
}

pub async fn encrypt_file(input_path: impl AsRef<Path>,
                      output_path: impl AsRef<Path>,
                      password: impl Into<String>) {
    process_file(input_path,
                 output_path,
                 CHUNK_SIZE,
                 password,
                 |less_safe_key, buffer: &[u8]| {
                     encrypt(&*buffer, less_safe_key).unwrap_or_exit("文件加密时失败")
                 }).await
        .unwrap_or_exit("文件加密失败");
}

fn derive_key(password: &[u8], salt: &[u8]) -> Result<[u8; 32], Unspecified> {
    let iterations = NonZeroU32::new(100_000).unwrap();
    let mut key = [0u8; 32];

    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        iterations,
        salt,
        password,
        &mut key,
    );

    Ok(key)
}

pub fn setup_key(password: impl Into<String>) -> LessSafeKey {
    let password_str = password.into();
    let key = derive_key(password_str.as_bytes(), SALT).unwrap();
    let unbound_key = UnboundKey::new(&AES_256_GCM, &key).expect("AES_256_GCM key setup failed");
    LessSafeKey::new(unbound_key)
}

pub fn encrypt(payload: &[u8], less_safe_key: &LessSafeKey) -> Result<Vec<u8>, Unspecified> {
    let nonce = Nonce::try_assume_unique_for_key(&NONCE).unwrap();
    let aad = Aad::from(AAD);
    let mut in_out = payload.to_vec();
    less_safe_key.seal_in_place_append_tag(nonce, aad, &mut in_out).unwrap_or_exit("加密失败");
    Ok(in_out)
}

pub fn decrypt(payload: &[u8], less_safe_key: &LessSafeKey) -> Result<Vec<u8>, Unspecified> {
    let nonce = Nonce::try_assume_unique_for_key(&NONCE).unwrap();
    let aad = Aad::from(AAD);
    let mut in_out = payload.to_vec();
    less_safe_key.open_in_place(nonce, aad, &mut in_out).unwrap_or_exit("解密失败");
    Ok(in_out)
}

pub fn get_crypt_file_name(path: impl Into<PathBuf>, is_encrypt: bool) -> Result<String, &'static str> {
    let path = path.into();
    let filename = if is_encrypt {
        let mut tmp = path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        tmp.push_str(".enc");
        tmp
    } else {
        let tmp = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        tmp
    };
    Ok(filename)
}


#[cfg(test)]
mod test {
    use ring::aead::{AES_256_GCM, LessSafeKey, UnboundKey};
    use tokio::fs::{DirBuilder, File, OpenOptions};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use crate::constant::CHUNK_SIZE;
    use crate::crypt::{decrypt, decrypt_file, derive_key, encrypt, encrypt_file};

    #[test]
    fn test_crypt() {
        let password = b"PASSWORD";
        let salt = b"SALT";
        let secret = derive_key(password, salt).unwrap();
        let payload = "Hello World!";
        let payload_u8 = payload.as_bytes();

        let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &secret).unwrap());

        let encrypt_data = encrypt(payload_u8, &key).unwrap();
        let decrypt_data = decrypt(&encrypt_data, &key).unwrap();

        assert_eq!(payload.as_bytes(), &decrypt_data[..payload.len()])
    }

    #[tokio::test]
    async fn test_crypt_file() {
        const PASSWORD: &str = "RAVEN_BOOK";
        const ENCRYPT_INPUT_PATH: &str = "target/test/example.txt";
        const ENCRYPT_OUTPUT_PATH: &str = "target/test/example.enc";
        const DECRYPT_OUTPUT_PATH: &str = "target/test/dec_example.txt";
        const CONTENT: &str = "HELLO WORLD!";
        const CONTENT_LENGTH: usize = CONTENT.len();
        const REPETITIONS: usize = CHUNK_SIZE * 2 / CONTENT_LENGTH;

        DirBuilder::new()
            .recursive(true)
            .create("target/test").await.unwrap();

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(ENCRYPT_INPUT_PATH).await.unwrap();

        for _ in 0..REPETITIONS {
            file.write_all(CONTENT.as_bytes()).await.unwrap();
        }

        file.flush().await.unwrap();

        encrypt_file(ENCRYPT_INPUT_PATH, ENCRYPT_OUTPUT_PATH, PASSWORD).await;
        decrypt_file(ENCRYPT_OUTPUT_PATH, DECRYPT_OUTPUT_PATH, PASSWORD).await;

        let mut raw_file = File::open(ENCRYPT_INPUT_PATH).await.unwrap();
        let mut decrypt_file = File::open(DECRYPT_OUTPUT_PATH).await.unwrap();

        let mut raw_text = String::new();
        let mut decrypt_text = String::new();
        raw_file.read_to_string(&mut raw_text).await.unwrap();
        decrypt_file.read_to_string(&mut decrypt_text).await.unwrap();

        assert_eq!(raw_text, decrypt_text)
    }
}