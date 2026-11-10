//! Ephemeral, object-scoped reception. Never persist or log bearer URLs.
use crate::error::{AppError, Result};
use crate::transfer::download::DownloadDestination;
use futures::StreamExt;
use sha2::{Digest, Sha256};
use std::{path::Path, time::Duration};
use url::Url;

const MAX_BYTES: u64 = 10 * 1024 * 1024 * 1024;
fn invalid(message: &str) -> AppError {
    AppError::InvalidContent(message.into())
}

fn validate_link(value: &str) -> Result<Url> {
    if value.len() > 16384 || value.chars().any(char::is_control) {
        return Err(invalid("Paste a valid S3 download link."));
    }
    let url = Url::parse(value).map_err(|_| invalid("Paste a valid S3 download link."))?;
    let host = url.host_str().unwrap_or_default();
    // Only AWS-owned regional S3 endpoints. No arbitrary endpoints or redirects.
    let labels: Vec<_> = host.split('.').collect();
    let regional = labels.len() >= 4
        && labels[labels.len() - 2..] == ["amazonaws", "com"]
        && labels[labels.len() - 4] == "s3"
        && valid_region(labels[labels.len() - 3]);
    let global = host == "s3.amazonaws.com" || host.ends_with(".s3.amazonaws.com");
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
        || !(regional || global)
        || url.path() == "/"
    {
        return Err(invalid(
            "Use an HTTPS AWS S3 download link supplied by the sender.",
        ));
    }
    let pairs: Vec<_> = url.query_pairs().collect();
    for required in [
        "X-Amz-Algorithm",
        "X-Amz-Credential",
        "X-Amz-Date",
        "X-Amz-Expires",
        "X-Amz-SignedHeaders",
        "X-Amz-Signature",
    ] {
        if pairs
            .iter()
            .filter(|(key, value)| key == required && !value.is_empty())
            .count()
            != 1
        {
            return Err(invalid(
                "This is not a signed S3 download link. Ask the sender for a new link.",
            ));
        }
    }
    let value = |name: &str| {
        pairs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_ref())
            .unwrap_or("")
    };
    if value("X-Amz-Algorithm") != "AWS4-HMAC-SHA256"
        || value("X-Amz-SignedHeaders") != "host"
        || !matches!(value("X-Amz-Expires").parse::<u64>(), Ok(1..=604800))
    {
        return Err(invalid(
            "This download link is not supported. Ask the sender for a new link.",
        ));
    }
    Ok(url)
}

fn valid_region(value: &str) -> bool {
    let parts: Vec<_> = value.split('-').collect();
    parts.len() >= 3
        && parts[0].len() == 2
        && parts.iter().all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
        && parts.last().unwrap().bytes().all(|c| c.is_ascii_digit())
}

fn validate_hash(value: Option<&str>) -> Result<Option<String>> {
    value
        .filter(|v| !v.is_empty())
        .map(|v| {
            if v.len() == 64 && v.bytes().all(|c| c.is_ascii_hexdigit()) {
                Ok(v.to_ascii_lowercase())
            } else {
                Err(invalid(
                    "SHA-256 must contain exactly 64 hexadecimal characters.",
                ))
            }
        })
        .transpose()
}

#[derive(serde::Serialize)]
pub struct Receipt {
    bytes: u64,
    sha256: String,
}

#[tauri::command]
pub async fn receive_handoff(
    link: String,
    local_path: String,
    expected_sha256: Option<String>,
) -> Result<Receipt> {
    let url = validate_link(link.trim())?;
    let expected = validate_hash(expected_sha256.as_deref())?;
    let destination = DownloadDestination::from_path(Path::new(&local_path), false)?;
    let mut pending = destination.begin()?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(1800))
        .build()
        .map_err(|_| invalid("Could not start the download. Please try again."))?;
    // Do not include reqwest errors: their Display representation includes the signed URL.
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|_| invalid("Could not reach S3. Check your connection and try again."))?;
    if !response.status().is_success() {
        return Err(invalid("Download was not accepted. The link may have expired; ask the sender for a fresh link."));
    }
    if response.content_length().is_some_and(|n| n > MAX_BYTES) {
        return Err(invalid("The file exceeds the 10 GB receive limit."));
    }
    let length = response.content_length();
    let mut stream = response.bytes_stream();
    let mut bytes = 0;
    let mut hash = Sha256::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| {
            invalid("Download was interrupted. Try again with the same or a fresh link.")
        })?;
        bytes += chunk.len() as u64;
        if bytes > MAX_BYTES {
            return Err(invalid("The file exceeds the 10 GB receive limit."));
        }
        hash.update(&chunk);
        pending.write_all(&chunk).await?;
    }
    if length.is_some_and(|n| n != bytes) {
        return Err(invalid("The download was incomplete. Please try again."));
    }
    let sha256 = format!("{:x}", hash.finalize());
    if expected.is_some_and(|value| value != sha256) {
        return Err(invalid(
            "The file checksum does not match. Ask the sender for a fresh link and checksum.",
        ));
    }
    pending.finish_writing().await?;
    pending.commit()?;
    Ok(Receipt { bytes, sha256 })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn signed(host: &str) -> String {
        format!("https://{host}/source.zip?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential=test&X-Amz-Date=20260922T000000Z&X-Amz-Expires=900&X-Amz-SignedHeaders=host&X-Amz-Signature=synthetic")
    }
    #[test]
    fn only_signed_s3_links_are_accepted() {
        for host in [
            "files.s3.ap-southeast-1.amazonaws.com",
            "s3.ap-southeast-1.amazonaws.com",
            "files.s3.amazonaws.com",
        ] {
            assert!(validate_link(&signed(host)).is_ok());
        }
        for host in [
            "localhost",
            "127.0.0.1",
            "169.254.169.254",
            "s3.amazonaws.com.evil.test",
            "example.com",
            "files.s3.evil.amazonaws.com",
            "user:password@files.s3.amazonaws.com",
            "files.s3.amazonaws.com:444",
        ] {
            assert!(validate_link(&signed(host)).is_err());
        }
        let valid = signed("files.s3.amazonaws.com");
        for input in [
            valid.replace("https:", "http:"),
            format!("{valid}#fragment"),
            format!("{valid}&X-Amz-Expires=1"),
            valid.replace("Expires=900", "Expires=0"),
            valid.replace("Expires=900", "Expires=604801"),
            "https://files.s3.amazonaws.com/file".into(),
            "x".repeat(16385),
        ] {
            assert!(validate_link(&input).is_err());
        }
    }
    #[test]
    fn checksum_validation() {
        assert_eq!(
            validate_hash(Some(&"A".repeat(64))).unwrap(),
            Some("a".repeat(64))
        );
        assert!(validate_hash(Some("not-a-hash")).is_err());
        assert_eq!(validate_hash(None).unwrap(), None);
    }
}
