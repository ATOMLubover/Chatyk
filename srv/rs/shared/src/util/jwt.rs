use chrono::Utc;
use jsonwebtoken::errors::Result;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JwtClaims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Clone)]
pub struct JwtCodec {
    pub encoding_key: EncodingKey,
    pub decoding_key: DecodingKey,
    pub exp: usize,
}

impl std::fmt::Debug for JwtCodec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtCodec")
            .field("encoding_key", &"EncodingKey { .. }")
            .field("decoding_key", &"DecodingKey { .. }")
            .finish()
    }
}

impl JwtCodec {
    pub fn new(encoding_key: &[u8], decoding_key: &[u8], expiration: usize) -> Self {
        JwtCodec {
            encoding_key: EncodingKey::from_secret(encoding_key),
            decoding_key: DecodingKey::from_secret(decoding_key),
            exp: expiration,
        }
    }

    pub fn encode<T>(&self, subject: T) -> Result<String>
    where
        T: Serialize,
    {
        let claims = serde_json::to_value(subject)?;

        let claims = JwtClaims {
            sub: claims.to_string(),
            exp: self.exp,
            iat: Utc::now().timestamp() as usize,
        };

        return encode_jwt(claims, &self.encoding_key);
    }

    pub fn decode<T>(&self, token: &str) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let claims: JwtClaims = decode_jwt(token, &self.decoding_key)?;

        let subject = serde_json::from_str(&claims.sub)?;

        return Ok(subject);
    }
}

pub fn encode_jwt<T>(claims: T, encoding_key: &EncodingKey) -> Result<String>
where
    T: Serialize,
{
    let token = jsonwebtoken::encode(&Header::default(), &claims, encoding_key)?;

    return Ok(token);
}

pub fn decode_jwt<T>(token: &str, decoding_key: &DecodingKey) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let validation = Validation::default();

    let token_data = jsonwebtoken::decode::<T>(token, decoding_key, &validation)?;

    return Ok(token_data.claims);
}
