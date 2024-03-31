use std::num::NonZeroU32;
use std::path::Path;
use ring::aead::{Aad, AES_256_GCM, LessSafeKey, Nonce, UnboundKey};
use ring::error::Unspecified;
use ring::pbkdf2;
use tokio::fs::File;
use tokio::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::constant::{AAD, CHUNK_SIZE, NONCE, SALT};

pub fn derive_key(password: &[u8], salt: &[u8]) -> Result<[u8; 32], Unspecified> {
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


async fn process_file(input_path:  impl AsRef<Path>,
                      output_path:  impl AsRef<Path>,
                      chunk_size: usize,
                      password: impl Into<String>,
                      operation: fn(&LessSafeKey, Nonce, &[u8]) -> Vec<u8>) -> io::Result<()> {
    let mut input_file = File::open(input_path).await?;
    let mut output_file = File::create(output_path).await?;
    let less_safe_key = setup_key(password);

    while let Some(buffer) = read_chunk(&mut input_file, chunk_size).await? {
        let nonce = Nonce::try_assume_unique_for_key(&NONCE).unwrap();
        let processed_data = operation(&less_safe_key, nonce, &buffer);
        output_file.write_all(&processed_data).await?;
    }

    Ok(())
}

async fn read_chunk(file: &mut File, chunk_size: usize) -> io::Result<Option<Vec<u8>>> {
    let mut buffer = vec![0; chunk_size];
    let bytes_read = file.read(&mut buffer).await?;
    if bytes_read == 0 {
        return Ok(None);
    }
    buffer.truncate(bytes_read);
    Ok(Some(buffer))
}

fn setup_key(password: impl Into<String>) -> LessSafeKey {
    let password_str = password.into();
    let key = derive_key(password_str.as_bytes(), SALT).unwrap();
    let unbound_key = UnboundKey::new(&AES_256_GCM, &key).expect("AES_256_GCM key setup failed");
    LessSafeKey::new(unbound_key)
}

pub async fn encrypt_file(input_path: impl AsRef<Path>,
                          output_path: impl AsRef<Path>,
                          password: impl Into<String>) -> io::Result<()> {
    process_file(input_path,
                 output_path,
                 CHUNK_SIZE,
                 password,
                 |less_safe_key, nonce, buffer: &[u8]| {
                     let mut in_out = buffer.to_vec();
                     let aad = Aad::from(AAD);
                     let _ = &less_safe_key.seal_in_place_append_tag(nonce, aad, &mut in_out).unwrap();
                     in_out
                 }).await
}

pub async fn decrypt_file(input_path: impl AsRef<Path>,
                          output_path: impl AsRef<Path>,
                          password: impl Into<String>) -> io::Result<()> {
    process_file(input_path,
                 output_path,
                 CHUNK_SIZE + AES_256_GCM.tag_len(),
                 password,
                 |less_safe_key, nonce, buffer: &[u8]| {
                     let mut in_out = buffer.to_vec();
                     let aad = Aad::from(AAD);
                     let decrypted_data = &less_safe_key.open_in_place(nonce, aad, &mut in_out).unwrap();
                     decrypted_data.to_vec()
                 }).await
}

pub fn _encrypt(secret: &[u8], payload: &[u8]) -> Result<Vec<u8>, Unspecified> {
    let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &secret).unwrap());
    let nonce = Nonce::try_assume_unique_for_key(&NONCE).unwrap();
    let aad = Aad::from(AAD);

    let mut in_out = payload.to_vec();
    key.seal_in_place_append_tag(nonce, aad, &mut in_out).unwrap();

    Ok(in_out)
}

pub fn _decrypt(secret: &[u8], payload: &[u8]) -> Result<Vec<u8>, Unspecified> {
    let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &secret).unwrap());
    let nonce = Nonce::try_assume_unique_for_key(&NONCE).unwrap();
    let aad = Aad::from(AAD);

    let mut in_out = payload.to_vec();
    key.open_in_place(nonce, aad, &mut in_out).unwrap();

    Ok(in_out)
}

#[cfg(test)]
mod test {
    use tokio::fs::{DirBuilder, File, OpenOptions};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use crate::crypt::{_decrypt, decrypt_file, derive_key, _encrypt, encrypt_file};

    #[test]
    fn test_crypt() {
        let password = b"PASSWORD";
        let salt = b"SALT";
        let secret = derive_key(password, salt).unwrap();
        let payload = "Hello World!";
        let payload_u8 = payload.as_bytes();

        let encrypt_data = _encrypt(&secret, payload_u8).unwrap();
        let decrypt_data = _decrypt(&secret, &encrypt_data).unwrap();

        println!("tag_len = {}", decrypt_data.len() - payload_u8.len());
        assert_eq!(payload.as_bytes(), &decrypt_data[..payload.len()])
    }

    #[tokio::test]
    async fn test_crypt_file() {
        let password = "RAVEN_BOOK";
        let encrypt_input_path = "target/test/example.txt";
        let encrypt_output_path = "target/test/example.enc";
        let decrypt_output_path = "target/test/dec_example.txt";

        DirBuilder::new()
            .recursive(true)
            .create("target/test").await.unwrap();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(encrypt_input_path).await.unwrap();
        file.write_all("HELLO WORLD!".as_bytes()).await.unwrap();
        file.flush().await.unwrap();

        encrypt_file(encrypt_input_path, encrypt_output_path, password).await.unwrap();
        decrypt_file(encrypt_output_path, decrypt_output_path, password).await.unwrap();

        let mut raw_file = File::open(encrypt_input_path).await.unwrap();
        let mut decrypt_file = File::open(decrypt_output_path).await.unwrap();

        let mut raw_text = String::new();
        let mut decrypt_text = String::new();
        raw_file.read_to_string(&mut raw_text).await.unwrap();
        decrypt_file.read_to_string(&mut decrypt_text).await.unwrap();

        assert_eq!(raw_text, decrypt_text)
    }
}