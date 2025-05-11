use ring::aead::NONCE_LEN;

pub(crate) const NONCE: [u8; 12] = [200u8; NONCE_LEN];

pub(crate) const AAD: &[u8; 36] = b"cfaf0256-beec-4495-9175-b9800dd2e2d7";

pub(crate) const SALT: &[u8; 36] = b"5462d05a-cbf4-465a-956f-2b98770beabb";

pub(crate) const CHUNK_SIZE: usize = 1024 * 1024 * 5;

pub(crate) const CHUNK_SIZE_WITH_TAG: usize = CHUNK_SIZE + 16;

// pub(crate) const TEMP_FOLDER: &str = "raven-oss-tmp";