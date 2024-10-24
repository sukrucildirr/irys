use crate::{Arbitrary, Signature};
use alloy_primitives::{bytes, Parity, U256 as RethU256};
use alloy_rlp::{Decodable, Encodable, Error as RlpError, RlpDecodable, RlpEncodable};
use arbitrary::Unstructured;
use base58::{FromBase58, ToBase58};
use eyre::Error;
use reth_codecs::Compact;
use reth_db_api::table::{Decode, Encode};
use reth_db_api::DatabaseError;
use serde::{
    de::{self, Error as _},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{ops::Index, slice::SliceIndex, str::FromStr};

use fixed_hash::construct_fixed_hash;
use uint::construct_uint;

//==============================================================================
// U256 Type
//------------------------------------------------------------------------------
construct_uint! {
    /// 256-bit unsigned integer.
    pub struct U256(4);
}

//==============================================================================
// H256 Type
//------------------------------------------------------------------------------
construct_fixed_hash! {
    /// A 256-bit hash type (32 bytes)
    pub struct H256(32);
}

// Manually implement Arbitrary for H256
impl<'a> Arbitrary<'a> for H256 {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(H256::random())
    }
}

impl Encode for H256 {
    type Encoded = [u8; 32];

    fn encode(self) -> Self::Encoded {
        self.0
    }
}

impl Decode for H256 {
    fn decode(value: &[u8]) -> Result<Self, DatabaseError> {
        Ok(Self::from_slice(
            value.try_into().map_err(|_| DatabaseError::Decode)?,
        ))
    }
}

impl Encodable for H256 {
    fn encode(&self, out: &mut dyn bytes::BufMut) {
        self.0.encode(out);
    }
    fn length(&self) -> usize {
        self.0.len()
    }
}

impl Decodable for H256 {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        if buf.len() < 32 {
            return Err(RlpError::Custom("not enough bytes to decode H256"));
        }
        Ok(H256::from_slice(buf))
    }
}
//==============================================================================
// IrysSignature
//------------------------------------------------------------------------------
#[derive(Clone, Eq, Debug, Arbitrary, RlpEncodable, RlpDecodable)]
pub struct IrysSignature {
    pub reth_signature: Signature,
}

impl PartialEq for IrysSignature {
    fn eq(&self, other: &Self) -> bool {
        self.reth_signature.r() == other.reth_signature.r()
            && self.reth_signature.s() == other.reth_signature.s()
            && self.reth_signature.v().y_parity() == other.reth_signature.v().y_parity()
    }
}

impl From<Signature> for IrysSignature {
    fn from(sig: Signature) -> Self {
        IrysSignature {
            reth_signature: sig, // Directly wrapping the Signature struct
        }
    }
}

impl Compact for IrysSignature {
    #[inline]
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: bytes::BufMut + AsMut<[u8]>,
    {
        self.reth_signature.to_compact(buf)
    }

    #[inline]
    fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]) {
        let compact = Signature::from_compact(buf, len);
        (
            IrysSignature {
                reth_signature: compact.0,
            },
            compact.1,
        )
    }
}

// Implement Serialize for H256
impl Serialize for IrysSignature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let bytes = self.reth_signature.as_bytes();
        serializer.serialize_str(bytes.to_base58().as_ref())
    }
}

// Implement Deserialize for H256
impl<'de> Deserialize<'de> for IrysSignature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // First, deserialize the base58-encoded string
        let s: String = Deserialize::deserialize(deserializer)?;

        // Decode the base58 string into bytes
        let bytes = FromBase58::from_base58(s.as_str())
            .map_err(|e| format!("Failed to decode from base58 {:?}", e))
            .expect("base58 should prase");

        // Ensure the byte array is exactly 65 bytes (r, s, and v values of the signature)
        if bytes.len() != 65 {
            return Err(de::Error::invalid_length(
                bytes.len(),
                &"expected 65 bytes for signature",
            ));
        }

        // Convert the byte array into a Signature struct using TryFrom
        let sig = Signature::try_from(bytes.as_slice()).map_err(de::Error::custom)?;

        // Return the IrysSignature by wrapping the Signature
        Ok(IrysSignature {
            reth_signature: sig,
        })
    }
}
//==============================================================================
// Address Base58
//------------------------------------------------------------------------------
pub mod address_base58_stringify {
    use alloy_primitives::Address;
    use base58::{FromBase58, ToBase58};
    use serde::{self, de, Deserialize, Deserializer, Serializer};

    #[allow(dead_code)]
    pub fn serialize<S>(value: &Address, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(value.0.to_base58().as_ref())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Address, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;

        // Decode the base58 string into bytes
        let bytes = FromBase58::from_base58(s.as_str())
            .map_err(|e| format!("Failed to decode from base58 {:?}", e))
            .expect("base58 should prase");

        // Ensure the byte array is exactly 65 bytes (r, s, and v values of the signature)
        if bytes.len() != 20 {
            return Err(de::Error::invalid_length(
                bytes.len(),
                &"expected 65 bytes for signature",
            ));
        }

        Ok(Address::from_slice(&bytes))
    }
}

//==============================================================================
// Option<u64>
//------------------------------------------------------------------------------
/// where u64 is represented as a string in the json
pub mod option_u64_stringify {
    use serde::{self, Deserialize, Deserializer, Serializer};
    use serde_json::Value;

    #[allow(dead_code)]
    pub fn serialize<S>(value: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(number) => serializer.serialize_str(&number.to_string()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt_val: Option<Value> = Option::deserialize(deserializer)?;

        match opt_val {
            Some(Value::String(s)) => s.parse::<u64>().map(Some).map_err(serde::de::Error::custom),
            Some(_) => Err(serde::de::Error::custom("Invalid type")),
            None => Ok(None),
        }
    }
}

//==============================================================================
// U256
//------------------------------------------------------------------------------
impl Default for IrysSignature {
    fn default() -> Self {
        IrysSignature {
            reth_signature: Signature::new(
                RethU256::default(),
                RethU256::default(),
                Parity::Eip155(0), // Assuming 0 as default parity
            ),
        }
    }
}
/// Implement Serialize for U256
impl Serialize for U256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}

/// Implement Deserialize for U256
impl<'de> Deserialize<'de> for U256 {
    fn deserialize<D>(deserializer: D) -> Result<U256, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        U256::from_dec_str(&s).map_err(serde::de::Error::custom)
    }
}

//==============================================================================
// H256
//------------------------------------------------------------------------------
impl H256 {
    pub fn to_vec(self) -> Vec<u8> {
        self.0.to_vec()
    }
}

// Implement Serialize for H256
impl Serialize for H256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_bytes().to_base58().as_ref())
    }
}

// Implement Deserialize for H256
impl<'de> Deserialize<'de> for H256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        DecodeHash::from(&s).map_err(D::Error::custom)
    }
}

/// Decode from encoded base58 string into H256 bytes.
pub trait DecodeHash: Sized {
    fn from(base58_string: &str) -> Result<Self, String>;
    fn empty() -> Self;
}

impl DecodeHash for H256 {
    fn from(base58_string: &str) -> Result<Self, String> {
        FromBase58::from_base58(base58_string)
            .map_err(|e| format!("Failed to decode from base58 {:?}", e))
            .map(|bytes| H256::from_slice(bytes.as_slice()))
    }

    fn empty() -> Self {
        H256::zero()
    }
}

impl Compact for H256 {
    #[inline]
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: bytes::BufMut + AsMut<[u8]>,
    {
        self.0.to_compact(buf)
    }

    #[inline]
    fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]) {
        // Disambiguate and call the correct H256::from method
        let (v, remaining_buf) = <[u8; 32]>::from_compact(buf, len);
        // Fully qualify this call to avoid calling DecodeHash::from
        (<H256 as From<[u8; 32]>>::from(v), remaining_buf)
    }
}

//==============================================================================
// Base64 Type
//------------------------------------------------------------------------------
/// A struct of [`Vec<u8>`] used for all `base64_url` encoded fields. This is
/// used for large fields like proof chunk data.

#[derive(Default, Debug, Clone, Eq, PartialEq, Compact, Arbitrary)]
pub struct Base64(pub Vec<u8>);

impl std::fmt::Display for Base64 {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let string = base64_url::encode(&self.0);
        write!(f, "{}", string)
    }
}

/// Converts a base64url encoded string to a Base64 struct.
impl FromStr for Base64 {
    type Err = base64_url::base64::DecodeError;
    fn from_str(str: &str) -> Result<Self, base64_url::base64::DecodeError> {
        let result = base64_url::decode(str)?;
        Ok(Self(result))
    }
}

impl Base64 {
    pub fn from_utf8_str(str: &str) -> Result<Self, Error> {
        Ok(Self(str.as_bytes().to_vec()))
    }
    pub fn to_utf8_string(&self) -> Result<String, Error> {
        Ok(String::from_utf8(self.0.clone())?)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn split_at(&self, mid: usize) -> (&[u8], &[u8]) {
        self.0.split_at(mid)
    }
}

impl Serialize for Base64 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&format!("{}", &self))
    }
}

impl<'de> Deserialize<'de> for Base64 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Vis;
        impl serde::de::Visitor<'_> for Vis {
            type Value = Base64;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a base64 string")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                base64_url::decode(v)
                    .map(Base64)
                    .map_err(|_| de::Error::custom("failed to decode base64 string"))
            }
        }
        deserializer.deserialize_str(Vis)
    }
}

//==============================================================================
// H256List Type
//------------------------------------------------------------------------------
/// A struct of [`Vec<H256>`] used for lists of [`Base64`] encoded hashes
#[derive(Debug, Default, Clone, Eq, PartialEq, Compact, Arbitrary)]
pub struct H256List(pub Vec<H256>);

impl H256List {
    // Constructor for an empty H256List
    pub fn new() -> Self {
        H256List(Vec::new())
    }

    // Constructor for an initialized H256List
    pub fn with_capacity(capacity: usize) -> Self {
        H256List(Vec::with_capacity(capacity))
    }

    pub fn push(&mut self, value: H256) {
        self.0.push(value)
    }

    pub fn reverse(&mut self) {
        self.0.reverse()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, H256> {
        self.0.iter()
    }

    pub fn get(&self, index: usize) -> Option<&<usize as SliceIndex<[H256]>>::Output> {
        self.0.get(index)
    }
}

impl Index<usize> for H256List {
    type Output = H256;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl PartialEq<Vec<H256>> for H256List {
    fn eq(&self, other: &Vec<H256>) -> bool {
        &self.0 == other
    }
}

impl PartialEq<H256List> for Vec<H256> {
    fn eq(&self, other: &H256List) -> bool {
        self == &other.0
    }
}

// Implement Serialize for H256 base64url encoded Array
impl Serialize for H256List {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize self.0 (Vec<Base64>) directly
        self.0.serialize(serializer)
    }
}

// Implement Deserialize for H256 base64url encoded Array
impl<'de> Deserialize<'de> for H256List {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Deserialize a Vec<Base64> and then wrap it in Base64Array
        Vec::<H256>::deserialize(deserializer).map(H256List)
    }
}
