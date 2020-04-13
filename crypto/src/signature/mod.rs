// Copyright 2019-2020 PolkaX Authors. Licensed under GPL-3.0.

/// Signature serialization/deserialization.
pub mod serde;

use std::convert::TryFrom;

use plum_address::{Address, Protocol};
use plum_hashing::blake2b_256;

use crate::errors::CryptoError;

/// The maximum length of signature.
pub const SIGNATURE_MAX_LENGTH: u32 = 200;

/// The signature type.
#[repr(u8)]
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub enum SignatureType {
    /// The `Secp256k1` signature.
    Secp256k1 = 1,
    /// The `BLS` signature.
    Bls = 2,
}

impl Default for SignatureType {
    fn default() -> Self {
        SignatureType::Bls
    }
}

impl TryFrom<u8> for SignatureType {
    type Error = CryptoError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(SignatureType::Secp256k1),
            2 => Ok(SignatureType::Bls),
            _ => Err(CryptoError::UnknownSignatureType(value)),
        }
    }
}

impl From<SignatureType> for u8 {
    fn from(ty: SignatureType) -> Self {
        match ty {
            SignatureType::Secp256k1 => 1,
            SignatureType::Bls => 2,
        }
    }
}

/// The general signature structure.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct Signature {
    /// The signature type.
    r#type: SignatureType,
    /// Tha actual signature bytes.
    data: Vec<u8>,
}

impl Signature {
    /// Create a signature with the given type and raw data
    pub fn new<T: Into<Vec<u8>>>(ty: SignatureType, data: T) -> Self {
        Self {
            r#type: ty,
            data: data.into(),
        }
    }

    /// Create a Secp256k1 signature with the given raw data.
    pub fn new_secp256k1<T: Into<Vec<u8>>>(data: T) -> Self {
        Self::new(SignatureType::Secp256k1, data)
    }

    /// Create a `BLS` signature with the given raw data.
    pub fn new_bls<T: Into<Vec<u8>>>(data: T) -> Self {
        Self::new(SignatureType::Bls, data)
    }

    /// Sign the message with the given signature type and private key.
    ///
    /// Return the signature related to the given signature type.
    pub fn sign<K, M>(ty: SignatureType, privkey: K, msg: M) -> Result<Self, CryptoError>
    where
        K: AsRef<[u8]>,
        M: AsRef<[u8]>,
    {
        match ty {
            SignatureType::Secp256k1 => Self::sign_secp256k1(privkey, msg),
            SignatureType::Bls => Self::sign_bls(privkey, msg),
        }
    }

    /// Sign the message with the given secp256k1 private key.
    ///
    /// Return the secp256k1 signature.
    pub fn sign_secp256k1<K, M>(privkey: K, msg: M) -> Result<Self, CryptoError>
    where
        K: AsRef<[u8]>,
        M: AsRef<[u8]>,
    {
        let seckey = secp256k1::SecretKey::parse_slice(privkey.as_ref())?;
        let hashed_msg = blake2b_256(msg); //  secp256k1::util::MESSAGE_SIZE == 32 bytes
        let message = secp256k1::Message::parse(&hashed_msg);
        let (signature, _recovery_id) = secp256k1::sign(&message, &seckey);
        Ok(Self {
            r#type: SignatureType::Secp256k1,
            data: signature.serialize().to_vec(),
        })
    }

    /// Sign the message with the given `BLS` private key.
    ///
    /// Return the `BLS` signature.
    pub fn sign_bls<K, M>(privkey: K, msg: M) -> Result<Self, CryptoError>
    where
        K: AsRef<[u8]>,
        M: AsRef<[u8]>,
    {
        use bls::Serialize;
        let privkey = bls::PrivateKey::from_bytes(privkey.as_ref())?;
        let signature = privkey.sign(msg);
        Ok(Self {
            r#type: SignatureType::Bls,
            data: signature.as_bytes(),
        })
    }

    /// Verify the signature with the given public key and message.
    pub fn verify<K, M>(&self, pubkey: K, msg: M) -> Result<bool, CryptoError>
    where
        K: AsRef<[u8]>,
        M: AsRef<[u8]>,
    {
        match self.r#type {
            SignatureType::Secp256k1 => self.verify_secp256k1(pubkey, msg),
            SignatureType::Bls => self.verify_bls(pubkey, msg),
        }
    }

    /// Verify the secp256k1 signature with the given secp256k1 public key and message.
    fn verify_secp256k1<K, M>(&self, pubkey: K, msg: M) -> Result<bool, CryptoError>
    where
        K: AsRef<[u8]>,
        M: AsRef<[u8]>,
    {
        let pubkey = secp256k1::PublicKey::parse_slice(pubkey.as_ref(), None)?;
        let hashed_msg = blake2b_256(msg);
        let msg = secp256k1::Message::parse(&hashed_msg); //  secp256k1::util::MESSAGE_SIZE == 32 bytes
        let signature = secp256k1::Signature::parse_slice(&self.data)?;
        Ok(secp256k1::verify(&msg, &signature, &pubkey))
    }

    /// Verify the `BLS` signature with the given `BLS` public key and message.
    fn verify_bls<K, M>(&self, pubkey: K, msg: M) -> Result<bool, CryptoError>
    where
        K: AsRef<[u8]>,
        M: AsRef<[u8]>,
    {
        use bls::Serialize;
        let pubkey = bls::PublicKey::from_bytes(pubkey.as_ref())?;
        // When signing with `BLS` privkey, the message will be hashed in `bls::PrivateKey::sign`,
        // so the message here needs to be hashed before the signature is verified.
        let hashed_msg = bls::hash(msg.as_ref());
        let signature = bls::Signature::from_bytes(&self.data)?;
        Ok(bls::verify(&signature, &[hashed_msg], &[pubkey]))
    }

    /// Return the signature type.
    pub fn r#type(&self) -> SignatureType {
        self.r#type
    }

    /// Return the actual signature bytes.
    pub fn as_bytes(&self) -> &[u8] {
        self.data.as_slice()
    }

    /// helper function to check signture type is same with address type
    pub fn check_address_type(&self, addr: &Address) -> Result<(), CryptoError> {
        let protocol = addr.protocol();
        match self.r#type {
            SignatureType::Secp256k1 => {
                if protocol != Protocol::Secp256k1 {
                    return Err(CryptoError::NotSameType(self.r#type, protocol));
                }
            }
            SignatureType::Bls => {
                if protocol != Protocol::Bls {
                    return Err(CryptoError::NotSameType(self.r#type, protocol));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Signature;
    use crate::key::{PrivateKey, PublicKey};

    #[test]
    fn sign_and_verify_secp256k1() {
        let privkey = PrivateKey::generate_secp256k1_privkey();
        let pubkey = PublicKey::from_privkey(&privkey);
        let (privkey, pubkey) = (privkey.into_vec(), pubkey.into_vec());
        let msg = "hello, world";
        let signature = Signature::sign_secp256k1(privkey, msg).unwrap();
        let res = signature.verify_secp256k1(pubkey, msg);
        assert_eq!(res, Ok(true))
    }

    #[test]
    fn sign_and_verify_bls() {
        let privkey = PrivateKey::generate_bls_privkey();
        let pubkey = PublicKey::from_privkey(&privkey);
        let (privkey, pubkey) = (privkey.into_vec(), pubkey.into_vec());
        let msg = "hello, world";
        let signature = Signature::sign_bls(privkey, msg).unwrap();
        let res = signature.verify_bls(pubkey, msg);
        assert_eq!(res, Ok(true))
    }
}
