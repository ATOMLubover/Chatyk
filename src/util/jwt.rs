use jsonwebtoken::errors::Result;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

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
