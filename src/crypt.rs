use std::num::NonZeroU32;
use ring::aead::{Aad, AES_256_GCM, LessSafeKey, Nonce, UnboundKey};
use ring::error::Unspecified;
use ring::pbkdf2;
use crate::constant::{AAD, NONCE, SALT};

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
    less_safe_key.seal_in_place_append_tag(nonce, aad, &mut in_out).unwrap();
    Ok(in_out)
}

pub fn decrypt(payload: &[u8], less_safe_key: &LessSafeKey) -> Result<Vec<u8>, Unspecified> {
    let nonce = Nonce::try_assume_unique_for_key(&NONCE).unwrap();
    let aad = Aad::from(AAD);
    let mut in_out = payload.to_vec();
    less_safe_key.open_in_place(nonce, aad, &mut in_out).unwrap();
    Ok(in_out)
}

#[cfg(test)]
mod test {
    use ring::aead::{AES_256_GCM, LessSafeKey, UnboundKey};
    use crate::crypt::{decrypt, derive_key, encrypt};

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

}