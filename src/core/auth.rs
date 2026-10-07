use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupabaseClaims {
    pub sub: Option<String>,
    pub exp: Option<u64>,
    pub email: Option<String>,
    pub role: Option<String>,
    pub iss: Option<String>,
    pub app_metadata: Option<serde_json::Value>,
    pub user_metadata: Option<serde_json::Value>,
}

/// Defensively validates the structure of a JWT token issued by Supabase.
/// Verifies:
/// 1. 3-part dot-separated format (header.payload.signature).
/// 2. Decodable URL-safe Base64 payload.
/// 3. Time expiration (`exp` > current unix timestamp).
/// 4. Presence of user identifier (`sub`).
pub fn validate_supabase_token(token: &str) -> Result<SupabaseClaims, String> {
    let token_clean = token.trim();
    if token_clean.is_empty() {
        return Err("Authentication token not provided.".to_string());
    }

    let parts: Vec<&str> = token_clean.split('.').collect();
    if parts.len() != 3 {
        return Err("Invalid JWT token format.".to_string());
    }

    let payload_b64 = parts[1];
    let decoded_payload = base64_url_decode(payload_b64)
        .map_err(|e| format!("Failed to decode token payload: {}", e))?;

    let claims: SupabaseClaims = serde_json::from_slice(&decoded_payload)
        .map_err(|e| format!("Corrupted JWT payload: {}", e))?;

    // Check expiration
    if let Some(exp) = claims.exp {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if now > exp {
            return Err("Session expired. Please log in again to continue.".to_string());
        }
    } else {
        return Err("Token missing valid expiration field.".to_string());
    }

    // Check if it has a unique user identifier
    if claims.sub.is_none() || claims.sub.as_deref() == Some("") {
        return Err("Token not associated with a valid user.".to_string());
    }

    Ok(claims)
}

fn base64_url_decode(input: &str) -> Result<Vec<u8>, String> {
    let mut s = input.replace('-', "+").replace('_', "/");
    match s.len() % 4 {
        2 => s.push_str("=="),
        3 => s.push_str("="),
        _ => {}
    }

    base64_decode_raw(&s)
}

fn base64_decode_raw(s: &str) -> Result<Vec<u8>, String> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut lookup = [255u8; 256];
    for (i, &b) in TABLE.iter().enumerate() {
        lookup[b as usize] = i as u8;
    }

    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;

    for &b in bytes {
        if b == b'=' || b.is_ascii_whitespace() {
            continue;
        }
        let val = lookup[b as usize];
        if val == 255 {
            return Err(format!("Invalid character in base64: {}", b as char));
        }
        buffer = (buffer << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }

    Ok(out)
}
