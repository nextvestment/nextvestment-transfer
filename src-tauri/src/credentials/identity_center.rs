//! Native IAM Identity Center device authorization. Tokens never cross IPC and
//! never enter the profile metadata file or the AWS CLI cache.
use super::{CredentialType, KeychainStorage, Profile};
use crate::commands::profiles::ProfileState;
use crate::error::{AppError, Result};
use aws_credential_types::{
    provider::{error::CredentialsError, future, ProvideCredentials},
    Credentials,
};
use aws_sdk_ssooidc::error::ProvideErrorMetadata;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::State;
use tauri_plugin_shell::ShellExt;
use tokio::sync::{Mutex, RwLock};

const SIGN_IN: &str =
    "Your AWS session has expired or was signed out. Sign in to AWS again in this profile.";
const NETWORK: &str = "AWS sign-in could not complete. Check your network and IAM Identity Center settings, then retry.";
const MAX_SESSIONS: usize = 4;
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn invalid(message: &str) -> AppError {
    AppError::InvalidCredentials(message.into())
}

#[derive(Clone, Serialize, Deserialize)]
struct VaultToken {
    token: String,
    expires_at: u64,
    start_url: String,
    sso_region: String,
    account_id: Option<String>,
    role_name: Option<String>,
}
#[derive(Clone)]
struct Pending {
    start_url: String,
    region: String,
    client_id: String,
    client_secret: String,
    device_code: String,
    verification_uri: String,
    expires_at: u64,
    interval: u64,
    next_poll: u64,
    token: Option<VaultToken>,
    polling: bool,
}
static SESSIONS: OnceLock<Mutex<HashMap<String, Pending>>> = OnceLock::new();
fn sessions() -> &'static Mutex<HashMap<String, Pending>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Restrict AWS clients to real commercial AWS regions and tenant access portals.
/// No arbitrary endpoint, credentials, query strings, or localhost addresses.
fn validate_portal(start_url: &str, region: &str) -> Result<String> {
    let parts: Vec<_> = region.split('-').collect();
    if parts.len() < 3
        || region.len() > 32
        || !matches!(
            parts[0],
            "af" | "ap" | "ca" | "eu" | "il" | "me" | "mx" | "sa" | "us"
        )
        || !region
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        || parts.last().is_none_or(|p| p.parse::<u8>().is_err())
        || region.starts_with("us-gov-")
    {
        return Err(invalid(
            "Enter a supported AWS commercial region, such as ap-southeast-1.",
        ));
    }
    let url = url::Url::parse(start_url.trim())
        .map_err(|_| invalid("Enter your HTTPS AWS access portal URL."))?;
    let host = url.host_str().unwrap_or_default();
    let tenant = host.strip_suffix(".awsapps.com").unwrap_or_default();
    if url.scheme() != "https"
        || tenant.is_empty()
        || tenant.contains('.')
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "/start" | "/start/")
    {
        return Err(invalid("Use the AWS access portal URL https://your-company.awsapps.com/start, without query parameters."));
    }
    Ok(format!("https://{host}/start"))
}
fn validate_verification_url(value: &str, region: &str) -> Result<()> {
    let url = url::Url::parse(value).map_err(|_| invalid(NETWORK))?;
    let expected = format!("device.sso.{region}.amazonaws.com");
    if url.scheme() != "https"
        || url.host_str() != Some(expected.as_str())
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(invalid(NETWORK));
    }
    Ok(())
}
fn oidc(region: &str) -> aws_sdk_ssooidc::Client {
    aws_sdk_ssooidc::Client::from_conf(
        aws_sdk_ssooidc::config::Builder::new()
            .behavior_version_latest()
            .region(aws_config::Region::new(region.to_owned()))
            .timeout_config(
                aws_smithy_types::timeout::TimeoutConfig::builder()
                    .operation_timeout(Duration::from_secs(20))
                    .build(),
            )
            .build(),
    )
}
fn sso(region: &str) -> aws_sdk_sso::Client {
    aws_sdk_sso::Client::from_conf(
        aws_sdk_sso::config::Builder::new()
            .behavior_version_latest()
            .region(aws_config::Region::new(region.to_owned()))
            .timeout_config(
                aws_smithy_types::timeout::TimeoutConfig::builder()
                    .operation_timeout(Duration::from_secs(20))
                    .build(),
            )
            .build(),
    )
}
fn cleanup(map: &mut HashMap<String, Pending>) {
    map.retain(|_, item| item.expires_at > now());
}
fn token_for_session(map: &HashMap<String, Pending>, id: &str) -> Result<VaultToken> {
    let item = map
        .get(id)
        .filter(|s| s.expires_at > now())
        .ok_or_else(|| invalid(SIGN_IN))?;
    let token = item
        .token
        .as_ref()
        .ok_or_else(|| invalid("Finish browser sign-in first."))?;
    if token.expires_at <= now() + 30 {
        return Err(invalid(SIGN_IN));
    }
    Ok(token.clone())
}
fn read_token(key: &str) -> Result<VaultToken> {
    read_token_from(&KeychainStorage::new(), key)
}
fn read_token_from(vault: &KeychainStorage, key: &str) -> Result<VaultToken> {
    if !key.starts_with("identity-center-") || uuid::Uuid::parse_str(&key[16..]).is_err() {
        return Err(invalid(SIGN_IN));
    }
    let json = vault.get(key)?.ok_or_else(|| invalid(SIGN_IN))?;
    let token: VaultToken = serde_json::from_str(&json).map_err(|_| invalid(SIGN_IN))?;
    if token.expires_at <= now() + 30 {
        return Err(invalid(SIGN_IN));
    }
    Ok(token)
}
fn vault_key() -> String {
    format!("identity-center-{}", uuid::Uuid::new_v4())
}

#[derive(Serialize)]
pub struct LoginStart {
    session_id: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}
#[derive(Serialize)]
pub struct LoginPoll {
    status: &'static str,
    interval: u64,
}
#[derive(Serialize)]
pub struct Account {
    account_id: String,
    account_name: String,
    email_address: Option<String>,
}
#[derive(Serialize)]
pub struct Role {
    account_id: String,
    role_name: String,
}

#[tauri::command]
pub async fn start_identity_center_login(
    start_url: String,
    sso_region: String,
) -> Result<LoginStart> {
    let start_url = validate_portal(&start_url, &sso_region)?;
    let mut map = sessions().lock().await;
    cleanup(&mut map);
    if map.len() >= MAX_SESSIONS {
        return Err(invalid(
            "Finish or cancel another AWS sign-in before starting a new one.",
        ));
    }
    drop(map);
    let _permit = LOOKUPS
        .try_acquire()
        .map_err(|_| invalid("Another AWS lookup is in progress. Please retry shortly."))?;
    let client = oidc(&sso_region);
    let registration = client
        .register_client()
        .client_name("Nextvestment Transfer")
        .client_type("public")
        .scopes("sso:account:access")
        .send()
        .await
        .map_err(|_| invalid(NETWORK))?;
    let client_id = registration
        .client_id()
        .ok_or_else(|| invalid(NETWORK))?
        .to_owned();
    let client_secret = registration
        .client_secret()
        .ok_or_else(|| invalid(NETWORK))?
        .to_owned();
    let auth = client
        .start_device_authorization()
        .client_id(&client_id)
        .client_secret(&client_secret)
        .start_url(&start_url)
        .send()
        .await
        .map_err(|_| invalid(NETWORK))?;
    let uri = auth
        .verification_uri_complete()
        .or(auth.verification_uri())
        .ok_or_else(|| invalid(NETWORK))?
        .to_owned();
    validate_verification_url(&uri, &sso_region)?;
    let interval = (auth.interval().max(1) as u64).max(5);
    let expires_in = (auth.expires_in().max(1) as u64).min(900);
    let session_id = uuid::Uuid::new_v4().to_string();
    let user_code = auth.user_code().ok_or_else(|| invalid(NETWORK))?.to_owned();
    let mut map = sessions().lock().await;
    cleanup(&mut map);
    if map.len() >= MAX_SESSIONS {
        return Err(invalid("Finish or cancel another AWS sign-in first."));
    }
    map.insert(
        session_id.clone(),
        Pending {
            start_url,
            region: sso_region,
            client_id,
            client_secret,
            device_code: auth
                .device_code()
                .ok_or_else(|| invalid(NETWORK))?
                .to_owned(),
            verification_uri: uri.clone(),
            expires_at: now() + expires_in,
            interval,
            next_poll: now() + interval,
            token: None,
            polling: false,
        },
    );
    Ok(LoginStart {
        session_id,
        user_code,
        verification_uri: uri,
        expires_in,
        interval,
    })
}
#[tauri::command]
pub async fn open_identity_center_login(session_id: String, app: tauri::AppHandle) -> Result<()> {
    let map = sessions().lock().await;
    let item = map
        .get(&session_id)
        .filter(|s| s.expires_at > now())
        .ok_or_else(|| invalid(SIGN_IN))?;
    validate_verification_url(&item.verification_uri, &item.region)?;
    #[allow(deprecated)]
    app.shell().open(&item.verification_uri, None).map_err(|_| {
        invalid("Could not open the browser. Copy the displayed AWS sign-in URL into your browser.")
    })
}
#[tauri::command]
pub async fn poll_identity_center_login(session_id: String) -> Result<LoginPoll> {
    let region = sessions()
        .lock()
        .await
        .get(&session_id)
        .map(|s| s.region.clone())
        .ok_or_else(|| invalid(SIGN_IN))?;
    poll_with_client(session_id, &oidc(&region)).await
}
async fn poll_with_client(
    session_id: String,
    client: &aws_sdk_ssooidc::Client,
) -> Result<LoginPoll> {
    let mut map = sessions().lock().await;
    cleanup(&mut map);
    let item = map.get_mut(&session_id).ok_or_else(|| invalid(SIGN_IN))?;
    if item.token.is_some() {
        return Ok(LoginPoll {
            status: "authorized",
            interval: item.interval,
        });
    }
    if item.polling || now() < item.next_poll {
        return Ok(LoginPoll {
            status: "pending",
            interval: item.interval,
        });
    }
    item.next_poll = now() + item.interval;
    item.polling = true;
    let request = item.clone();
    drop(map);
    let item = &request;
    let response = client
        .create_token()
        .client_id(&item.client_id)
        .client_secret(&item.client_secret)
        .device_code(&item.device_code)
        .grant_type("urn:ietf:params:oauth:grant-type:device_code")
        .send()
        .await;
    let mut map = sessions().lock().await;
    let item = map
        .get_mut(&session_id)
        .filter(|s| s.expires_at > now())
        .ok_or_else(|| invalid(SIGN_IN))?;
    item.polling = false;
    match response {
        Ok(output) => {
            let token = VaultToken {
                token: output
                    .access_token()
                    .ok_or_else(|| invalid(NETWORK))?
                    .to_owned(),
                expires_at: now() + (output.expires_in().max(1) as u64),
                start_url: item.start_url.clone(),
                sso_region: item.region.clone(),
                account_id: None,
                role_name: None,
            };
            // Pending authorization stays process-local. Only a saved, account/role-bound
            // session is written to the OS vault, so cancelling or a crash leaves no token orphan.
            item.token = Some(token);
            item.client_secret.clear();
            item.device_code.clear();
            // Bound account/role selection to fifteen minutes; the persisted token keeps its actual lifetime.
            item.expires_at = now() + 900;
            Ok(LoginPoll {
                status: "authorized",
                interval: item.interval,
            })
        }
        Err(error) => match error.as_service_error().and_then(|e| e.code()) {
            Some("AuthorizationPendingException") => Ok(LoginPoll {
                status: "pending",
                interval: item.interval,
            }),
            Some("SlowDownException") => {
                item.interval = (item.interval + 5).min(60);
                item.next_poll = now() + item.interval;
                Ok(LoginPoll {
                    status: "pending",
                    interval: item.interval,
                })
            }
            Some("AccessDeniedException" | "ExpiredTokenException") => {
                map.remove(&session_id);
                Err(invalid(
                    "AWS sign-in was declined or expired. Start again when ready.",
                ))
            }
            _ => Err(invalid(NETWORK)),
        },
    }
}
async fn accounts_inner(token: &VaultToken) -> Result<Vec<Account>> {
    let client = sso(&token.sso_region);
    let mut next = None;
    let mut found = Vec::new();
    for _ in 0..100 {
        let output = client
            .list_accounts()
            .access_token(&token.token)
            .set_next_token(next)
            .max_results(100)
            .send()
            .await
            .map_err(|_| invalid(SIGN_IN))?;
        found.extend(output.account_list().iter().filter_map(|a| {
            Some(Account {
                account_id: a.account_id()?.into(),
                account_name: a.account_name().unwrap_or("AWS account").into(),
                email_address: a.email_address().map(str::to_owned),
            })
        }));
        next = output.next_token().map(str::to_owned);
        if next.is_none() {
            return Ok(found);
        }
    }
    Err(invalid(
        "Too many AWS accounts to list. Ask your administrator to reduce assignments.",
    ))
}
async fn roles_inner(token: &VaultToken, account_id: &str) -> Result<Vec<Role>> {
    if account_id.len() != 12 || !account_id.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid("Select a valid AWS account."));
    }
    let client = sso(&token.sso_region);
    let mut next = None;
    let mut found = Vec::new();
    for _ in 0..100 {
        let output = client
            .list_account_roles()
            .access_token(&token.token)
            .account_id(account_id)
            .set_next_token(next)
            .max_results(100)
            .send()
            .await
            .map_err(|_| invalid(SIGN_IN))?;
        found.extend(output.role_list().iter().filter_map(|r| {
            Some(Role {
                account_id: account_id.into(),
                role_name: r.role_name()?.into(),
            })
        }));
        next = output.next_token().map(str::to_owned);
        if next.is_none() {
            return Ok(found);
        }
    }
    Err(invalid(
        "Too many AWS roles to list. Ask your administrator to reduce assignments.",
    ))
}
static LOOKUPS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);
async fn accounts(token: &VaultToken) -> Result<Vec<Account>> {
    let _permit = LOOKUPS
        .try_acquire()
        .map_err(|_| invalid("Another AWS lookup is in progress. Please retry shortly."))?;
    tokio::time::timeout(Duration::from_secs(60), accounts_inner(token))
        .await
        .map_err(|_| invalid(NETWORK))?
}
async fn roles(token: &VaultToken, account_id: &str) -> Result<Vec<Role>> {
    let _permit = LOOKUPS
        .try_acquire()
        .map_err(|_| invalid("Another AWS lookup is in progress. Please retry shortly."))?;
    tokio::time::timeout(Duration::from_secs(60), roles_inner(token, account_id))
        .await
        .map_err(|_| invalid(NETWORK))?
}
#[tauri::command]
pub async fn list_identity_center_accounts(session_id: String) -> Result<Vec<Account>> {
    let token = {
        let map = sessions().lock().await;
        token_for_session(&map, &session_id)?
    };
    accounts(&token).await
}
#[tauri::command]
pub async fn list_identity_center_roles(
    session_id: String,
    account_id: String,
) -> Result<Vec<Role>> {
    let token = {
        let map = sessions().lock().await;
        token_for_session(&map, &session_id)?
    };
    roles(&token, &account_id).await
}
#[tauri::command]
pub async fn save_identity_center_profile(
    session_id: String,
    account_id: String,
    role_name: String,
    name: String,
    region: String,
    profile_id: Option<String>,
    state: State<'_, ProfileState>,
) -> Result<Profile> {
    if name.trim().is_empty() || name.len() > 128 {
        return Err(invalid("Enter a profile name of up to 128 characters."));
    }
    let mut token = {
        let map = sessions().lock().await;
        token_for_session(&map, &session_id)?
    };
    validate_portal(&token.start_url, &region)?;
    if !roles(&token, &account_id)
        .await?
        .iter()
        .any(|r| r.role_name == role_name)
    {
        return Err(invalid("This AWS role is not assigned to your account."));
    }
    let mut map = sessions().lock().await;
    // Cancellation or expiry during AWS role lookup must prevent profile persistence.
    token_for_session(&map, &session_id)?;
    token.account_id = Some(account_id.clone());
    token.role_name = Some(role_name.clone());
    let session_ref = vault_key();
    KeychainStorage::new().store(
        &session_ref,
        &serde_json::to_string(&token).map_err(|_| invalid(NETWORK))?,
    )?;
    let profile = Profile::new(
        name.trim().into(),
        CredentialType::IdentityCenter {
            start_url: token.start_url,
            sso_region: token.sso_region,
            account_id,
            role_name,
            session_ref: session_ref.clone(),
        },
        Some(region),
    );
    let mut manager = state.write().await;
    let result = match profile_id {
        Some(id) => manager.update_profile(&id, profile).await,
        None => manager.add_profile(profile).await,
    };
    if result.is_err() {
        let _ = KeychainStorage::new().delete(&session_ref);
        return result;
    }
    map.remove(&session_id);
    result
}
#[tauri::command]
pub async fn cancel_identity_center_login(session_id: String) -> Result<()> {
    sessions().lock().await.remove(&session_id);
    Ok(())
}
#[tauri::command]
pub async fn sign_out_identity_center(
    profile_id: String,
    state: State<'_, ProfileState>,
    clients: State<'_, Arc<RwLock<crate::s3::S3ClientManager>>>,
) -> Result<()> {
    let profile = state.read().await.get_profile(&profile_id).await?;
    if let CredentialType::IdentityCenter {
        session_ref,
        sso_region,
        ..
    } = profile.credential_type
    {
        let token = read_token(&session_ref).ok();
        KeychainStorage::new().delete(&session_ref)?;
        clients.write().await.clear_cache();
        if let Some(token) = token {
            let _ = sso(&sso_region)
                .logout()
                .access_token(token.token)
                .send()
                .await;
        }
    }
    Ok(())
}
fn bound_token(profile: &Profile) -> Result<VaultToken> {
    bound_token_from(&KeychainStorage::new(), profile)
}
fn bound_token_from(vault: &KeychainStorage, profile: &Profile) -> Result<VaultToken> {
    if let CredentialType::IdentityCenter {
        start_url,
        sso_region,
        account_id,
        role_name,
        session_ref,
    } = &profile.credential_type
    {
        validate_portal(start_url, sso_region)?;
        let token = read_token_from(vault, session_ref)?;
        if token.start_url != *start_url
            || token.sso_region != *sso_region
            || token.account_id.as_ref() != Some(account_id)
            || token.role_name.as_ref() != Some(role_name)
        {
            return Err(invalid("AWS profile settings changed. Sign in again to authorize the selected account and role."));
        }
        Ok(token)
    } else {
        Err(invalid(SIGN_IN))
    }
}
pub(crate) fn validate_profile_binding(profile: &Profile) -> Result<()> {
    if matches!(
        profile.credential_type,
        CredentialType::IdentityCenter { .. }
    ) {
        bound_token(profile)?;
    }
    Ok(())
}
#[derive(Debug)]
pub struct IdentityCenterProvider {
    profile: Profile,
}
impl IdentityCenterProvider {
    pub fn new(profile: Profile) -> Self {
        Self { profile }
    }
}
impl ProvideCredentials for IdentityCenterProvider {
    fn provide_credentials<'a>(&'a self) -> future::ProvideCredentials<'a>
    where
        Self: 'a,
    {
        future::ProvideCredentials::new(async move {
            let token = bound_token(&self.profile)
                .map_err(|e| CredentialsError::provider_error(e.to_string()))?;
            fetch_role_credentials(&sso(&token.sso_region), token).await
        })
    }
}

async fn fetch_role_credentials(
    client: &aws_sdk_sso::Client,
    token: VaultToken,
) -> std::result::Result<Credentials, CredentialsError> {
    let output = client
        .get_role_credentials()
        .access_token(token.token)
        .account_id(token.account_id.unwrap_or_default())
        .role_name(token.role_name.unwrap_or_default())
        .send()
        .await
        .map_err(|_| CredentialsError::provider_error(SIGN_IN))?;
    let role = output
        .role_credentials()
        .ok_or_else(|| CredentialsError::provider_error(SIGN_IN))?;
    let access = role
        .access_key_id()
        .ok_or_else(|| CredentialsError::provider_error(SIGN_IN))?;
    let secret = role
        .secret_access_key()
        .ok_or_else(|| CredentialsError::provider_error(SIGN_IN))?;
    let session = role
        .session_token()
        .ok_or_else(|| CredentialsError::provider_error(SIGN_IN))?;
    if role.expiration() <= (now() * 1000) as i64 {
        return Err(CredentialsError::provider_error(SIGN_IN));
    }
    let expiration = UNIX_EPOCH + Duration::from_millis(role.expiration() as u64);
    Ok(Credentials::new(
        access,
        secret,
        Some(session.into()),
        Some(expiration),
        "identity-center-native",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portal_rejects_endpoint_and_credential_injection() {
        for url in [
            "http://company.awsapps.com/start",
            "https://company.awsapps.com.attacker.test/start",
            "https://company.awsapps.com/start?token=secret",
            "https://user@company.awsapps.com/start",
            "https://company.awsapps.com:8443/start",
            "https://localhost/start",
            "https://company.awsapps.com/other",
        ] {
            assert!(validate_portal(url, "ap-southeast-1").is_err(), "{url}");
        }
        assert_eq!(
            validate_portal("https://company.awsapps.com/start/", "ap-southeast-1").unwrap(),
            "https://company.awsapps.com/start"
        );
        for region in [
            "localhost",
            "../evil",
            "us-east-1/",
            "us-gov-west-1",
            "cn-north-1",
        ] {
            assert!(validate_portal("https://company.awsapps.com/start", region).is_err());
        }
    }
    #[test]
    fn verification_link_must_be_the_matching_regional_aws_service() {
        assert!(validate_verification_url(
            "https://device.sso.ap-southeast-1.amazonaws.com/?user_code=ABCD",
            "ap-southeast-1"
        )
        .is_ok());
        assert!(validate_verification_url(
            "https://device.sso.us-east-1.amazonaws.com/",
            "ap-southeast-1"
        )
        .is_err());
        assert!(validate_verification_url(
            "https://device.sso.ap-southeast-1.amazonaws.com.attacker.test/",
            "ap-southeast-1"
        )
        .is_err());
    }
    #[test]
    fn identity_metadata_and_debug_never_contain_token() {
        let p = Profile::new(
            "Work".into(),
            CredentialType::IdentityCenter {
                start_url: "https://company.awsapps.com/start".into(),
                sso_region: "ap-southeast-1".into(),
                account_id: "123456789012".into(),
                role_name: "Transfer".into(),
                session_ref: vault_key(),
            },
            None,
        );
        assert!(!serde_json::to_string(&p).unwrap().contains("access_token"));
        assert!(!format!("{:?}", p.credential_type).contains("session_ref"));
        assert!(read_token("arbitrary-vault-entry").is_err());
    }
    fn sample_token() -> VaultToken {
        VaultToken {
            token: "synthetic-device-access-token".into(),
            expires_at: now() + 3600,
            start_url: "https://company.awsapps.com/start".into(),
            sso_region: "ap-southeast-1".into(),
            account_id: Some("123456789012".into()),
            role_name: Some("Transfer".into()),
        }
    }
    fn sample_profile(key: String) -> Profile {
        Profile::new(
            "Work".into(),
            CredentialType::IdentityCenter {
                start_url: "https://company.awsapps.com/start".into(),
                sso_region: "ap-southeast-1".into(),
                account_id: "123456789012".into(),
                role_name: "Transfer".into(),
                session_ref: key,
            },
            None,
        )
    }
    #[test]
    fn vault_session_is_expiring_revocable_and_bound_to_all_identity_settings() {
        let vault = KeychainStorage::for_test();
        let key = vault_key();
        let mut token = sample_token();
        vault
            .store(&key, &serde_json::to_string(&token).unwrap())
            .unwrap();
        let profile = sample_profile(key.clone());
        assert!(bound_token_from(&vault, &profile).is_ok());
        for field in ["start_url", "sso_region", "account_id", "role_name"] {
            let mut json = serde_json::to_value(&profile).unwrap();
            let replacement = match field {
                "start_url" => "https://another.awsapps.com/start",
                "sso_region" => "us-east-1",
                "account_id" => "999999999999",
                _ => "Administrator",
            };
            json["credential_type"][field] = serde_json::Value::String(replacement.into());
            let changed: Profile = serde_json::from_value(json).unwrap();
            let error = bound_token_from(&vault, &changed)
                .err()
                .unwrap()
                .to_string();
            assert!(error.contains("settings changed"));
            assert!(!error.contains(&token.token));
        }
        token.expires_at = now() - 1;
        vault.delete(&key).unwrap();
        vault
            .store(&key, &serde_json::to_string(&token).unwrap())
            .unwrap();
        assert!(read_token_from(&vault, &key)
            .err()
            .unwrap()
            .to_string()
            .contains("Sign in"));
        vault.delete(&key).unwrap();
        assert!(read_token_from(&vault, &key).is_err());
    }
    #[tokio::test]
    async fn cancellation_and_expiry_prevent_late_profile_save() {
        let id = uuid::Uuid::new_v4().to_string();
        let item = Pending {
            start_url: "https://company.awsapps.com/start".into(),
            region: "ap-southeast-1".into(),
            client_id: "synthetic-client".into(),
            client_secret: String::new(),
            device_code: String::new(),
            verification_uri: String::new(),
            expires_at: now() + 60,
            interval: 5,
            next_poll: 0,
            token: Some(sample_token()),
            polling: false,
        };
        sessions().lock().await.insert(id.clone(), item.clone());
        assert!(token_for_session(&*sessions().lock().await, &id).is_ok());
        cancel_identity_center_login(id.clone()).await.unwrap();
        assert!(token_for_session(&*sessions().lock().await, &id).is_err());
        let mut expired = item;
        expired.expires_at = now() - 1;
        let mut isolated = HashMap::from([(id.clone(), expired)]);
        cleanup(&mut isolated);
        assert!(isolated.is_empty());
    }
    async fn mock_role_service(
        status: &str,
        body: String,
    ) -> (aws_sdk_sso::Client, tokio::task::JoinHandle<String>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let response=format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![0u8; 8192];
            let n = socket.read(&mut request).await.unwrap();
            socket.write_all(response.as_bytes()).await.unwrap();
            String::from_utf8_lossy(&request[..n]).into_owned()
        });
        let client = aws_sdk_sso::Client::from_conf(
            aws_sdk_sso::config::Builder::new()
                .behavior_version_latest()
                .region(aws_config::Region::new("ap-southeast-1"))
                .endpoint_url(endpoint)
                .retry_config(
                    aws_sdk_sso::config::retry::RetryConfig::standard().with_max_attempts(1),
                )
                .build(),
        );
        (client, server)
    }
    #[tokio::test]
    async fn sdk_role_exchange_is_cli_free_and_keeps_real_expiration() {
        let expiration = (now() + 900) * 1000;
        let body=serde_json::json!({"roleCredentials":{"accessKeyId":"synthetic-access","secretAccessKey":"synthetic-secret","sessionToken":"synthetic-session","expiration":expiration}}).to_string();
        let (client, server) = mock_role_service("200 OK", body).await;
        let credentials = fetch_role_credentials(&client, sample_token())
            .await
            .unwrap();
        assert_eq!(credentials.access_key_id(), "synthetic-access");
        assert_eq!(
            credentials.expiry().unwrap(),
            UNIX_EPOCH + Duration::from_millis(expiration)
        );
        assert_eq!(credentials.session_token(), Some("synthetic-session"));
        let request = server.await.unwrap();
        assert!(request.contains("/federation/credentials?"));
        assert!(request.contains("account_id=123456789012"));
        assert!(request.contains("role_name=Transfer"));
        assert!(request.contains("synthetic-device-access-token"));
    }
    #[tokio::test]
    async fn expired_or_denied_role_credentials_never_become_a_usable_session() {
        let (client, server) = mock_role_service(
            "401 Unauthorized",
            r#"{"message":"synthetic-device-access-token private diagnostics"}"#.into(),
        )
        .await;
        let error = fetch_role_credentials(&client, sample_token())
            .await
            .unwrap_err()
            .to_string();
        assert!(!error.contains("synthetic"));
        server.await.unwrap();
        let body=serde_json::json!({"roleCredentials":{"accessKeyId":"access","secretAccessKey":"secret","sessionToken":"session","expiration":1}}).to_string();
        let (client, server) = mock_role_service("200 OK", body).await;
        assert!(fetch_role_credentials(&client, sample_token())
            .await
            .is_err());
        server.await.unwrap();
    }
    #[tokio::test]
    async fn device_poll_pending_slowdown_and_success_follow_aws_state_without_exposing_tokens() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let id = uuid::Uuid::new_v4().to_string();
        sessions().lock().await.insert(
            id.clone(),
            Pending {
                start_url: "https://company.awsapps.com/start".into(),
                region: "ap-southeast-1".into(),
                client_id: "client".into(),
                client_secret: "private-client-secret".into(),
                device_code: "private-device-code".into(),
                verification_uri: String::new(),
                expires_at: now() + 900,
                interval: 5,
                next_poll: 0,
                token: None,
                polling: false,
            },
        );
        for (code, body, expected, interval) in [
            (
                "400 Bad Request",
                r#"{"__type":"AuthorizationPendingException","message":"waiting"}"#,
                "pending",
                5,
            ),
            (
                "400 Bad Request",
                r#"{"__type":"SlowDownException","message":"wait longer"}"#,
                "pending",
                10,
            ),
            (
                "200 OK",
                r#"{"accessToken":"private-new-access-token","expiresIn":3600,"tokenType":"Bearer"}"#,
                "authorized",
                10,
            ),
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let endpoint = format!("http://{}", listener.local_addr().unwrap());
            let response=format!("HTTP/1.1 {code}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = vec![0; 8192];
                let count = stream.read(&mut bytes).await.unwrap();
                assert!(count > 0);
                stream.write_all(response.as_bytes()).await.unwrap();
            });
            let client = aws_sdk_ssooidc::Client::from_conf(
                aws_sdk_ssooidc::config::Builder::new()
                    .behavior_version_latest()
                    .region(aws_config::Region::new("ap-southeast-1"))
                    .endpoint_url(endpoint)
                    .retry_config(
                        aws_sdk_ssooidc::config::retry::RetryConfig::standard()
                            .with_max_attempts(1),
                    )
                    .build(),
            );
            sessions().lock().await.get_mut(&id).unwrap().next_poll = 0;
            let result = tokio::time::timeout(
                Duration::from_secs(5),
                poll_with_client(id.clone(), &client),
            )
            .await
            .unwrap()
            .unwrap();
            assert_eq!(result.status, expected);
            assert_eq!(result.interval, interval);
            assert!(!serde_json::to_string(&result).unwrap().contains("private"));
            server.await.unwrap();
        }
        let map = sessions().lock().await;
        let item = map.get(&id).unwrap();
        assert!(item.client_secret.is_empty());
        assert!(item.device_code.is_empty());
        assert!(item.token.is_some());
        drop(map);
        cancel_identity_center_login(id).await.unwrap();
    }
}
