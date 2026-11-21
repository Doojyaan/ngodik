use std::time::Duration;

use reqwest::{
    Client, RequestBuilder, StatusCode,
    header::{AUTHORIZATION, HeaderValue},
};
use serde::Deserialize;
use serde_json::json;
use url::{Host, Url};
use zeroize::Zeroizing;

use super::{ProviderError, Result};

pub fn base_url(input: &str) -> Result<Url> {
    let input = input.trim();
    let (scheme, rest) = input
        .split_once("://")
        .ok_or(ProviderError::InvalidEndpoint)?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if input.len() > 500
        || authority.is_empty()
        || authority.contains('@')
        || input.contains('\\')
        || input.chars().any(char::is_whitespace)
    {
        return Err(ProviderError::InvalidEndpoint);
    }
    let mut url = Url::parse(input).map_err(|_| ProviderError::InvalidEndpoint)?;
    let local = match url.host() {
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        Some(Host::Domain(domain)) => domain == "localhost",
        None => return Err(ProviderError::InvalidEndpoint),
    };
    if url.query().is_some()
        || url.fragment().is_some()
        || !(scheme == "https" || (scheme == "http" && local))
    {
        return Err(ProviderError::InvalidEndpoint);
    }
    if !url.path().ends_with('/') {
        url.path_segments_mut()
            .map_err(|_| ProviderError::InvalidEndpoint)?
            .push("");
    }
    Ok(url)
}

pub fn client() -> Result<Client> {
    client_with_timeout(Duration::from_secs(120))
}

pub(super) fn client_with_timeout(timeout: Duration) -> Result<Client> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .resolve("localhost", ([127, 0, 0, 1], 0).into())
        .connect_timeout(Duration::from_secs(10))
        .timeout(timeout)
        .build()
        .map_err(|_| ProviderError::NetworkUnavailable)
}

#[derive(Deserialize)]
struct ModelList {
    data: Vec<Model>,
}
#[derive(Deserialize)]
struct Model {
    id: String,
}

pub async fn models(client: &Client, base: &Url, key: &[u8]) -> Result<Vec<String>> {
    let body = fetch(client.get(route(base, "models")?), key).await?;
    let models: ModelList =
        serde_json::from_slice(&body).map_err(|_| ProviderError::InvalidResponse)?;
    let mut ids = Vec::new();
    for model in models.data {
        if model
            .id
            .as_bytes()
            .windows(key.len())
            .any(|part| part == key)
        {
            return Err(ProviderError::InvalidResponse);
        }
        if super::valid_model(&model.id) && ids.len() < 500 {
            ids.push(model.id);
        }
    }
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}
#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}
#[derive(Deserialize)]
struct ChatMessage {
    role: String,
    content: Option<String>,
}

pub async fn check(client: &Client, base: &Url, key: &[u8], model: &str) -> Result<()> {
    let body = fetch(
        client.post(route(base, "chat/completions")?).json(&json!({
            "model": model,
            "messages": [{"role": "user", "content": "Reply with OK."}],
            "stream": false
        })),
        key,
    )
    .await?;
    // Parse only the message role and whether text exists. Provider output never crosses IPC.
    let chat: ChatResponse =
        serde_json::from_slice(&body).map_err(|_| ProviderError::InvalidResponse)?;
    if !chat.choices.iter().any(|choice| {
        choice.message.role == "assistant"
            && choice
                .message
                .content
                .as_deref()
                .is_some_and(|text| !text.trim().is_empty())
    }) {
        return Err(ProviderError::InvalidResponse);
    }
    Ok(())
}

fn route(base: &Url, suffix: &str) -> Result<Url> {
    base.join(suffix)
        .map_err(|_| ProviderError::InvalidEndpoint)
}

pub async fn generate(
    client: &Client,
    base: &Url,
    key: &[u8],
    model: &str,
    instruction: &str,
    data: &str,
) -> (Result<serde_json::Value>, Option<Duration>) {
    let mut retry_after = None;
    let result = async {
        let body = fetch_with_retry(
            client.post(route(base, "chat/completions")?).json(&json!({
                "model": model,
                "messages": [
                    {"role":"system", "content":instruction},
                    {"role":"user", "content":data}
                ],
                "stream": false
            })),
            key,
            &mut retry_after,
        )
        .await?;
        let chat: ChatResponse =
            serde_json::from_slice(&body).map_err(|_| ProviderError::InvalidResponse)?;
        let content = chat
            .choices
            .into_iter()
            .find(|choice| choice.message.role == "assistant")
            .and_then(|choice| choice.message.content)
            .ok_or(ProviderError::InvalidResponse)?;
        let content = Zeroizing::new(content);
        let text = content.trim();
        let text = text
            .strip_prefix("```json")
            .or_else(|| text.strip_prefix("```"))
            .and_then(|text| text.strip_suffix("```"))
            .unwrap_or(text)
            .trim();
        if text.len() > 32 * 1024 {
            return Err(ProviderError::InvalidResponse);
        }
        serde_json::from_str(text).map_err(|_| ProviderError::InvalidResponse)
    }
    .await;
    (result, retry_after)
}

async fn fetch(request: RequestBuilder, key: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    fetch_with_retry(request, key, &mut None).await
}

async fn fetch_with_retry(
    request: RequestBuilder,
    key: &[u8],
    retry_after: &mut Option<Duration>,
) -> Result<Zeroizing<Vec<u8>>> {
    if key.is_empty() {
        return Err(ProviderError::MissingKey);
    }
    let mut bearer = Zeroizing::new(b"Bearer ".to_vec());
    bearer.extend_from_slice(key);
    let mut auth = HeaderValue::from_bytes(&bearer).map_err(|_| ProviderError::InvalidKey)?;
    auth.set_sensitive(true);

    let mut response = request
        .header(AUTHORIZATION, auth)
        .send()
        .await
        .map_err(network_error)?;
    *retry_after = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|text| {
            text.parse::<u64>()
                .ok()
                .map(Duration::from_secs)
                .or_else(|| {
                    let date = httpdate::parse_http_date(text).ok()?;
                    Some(
                        date.duration_since(std::time::SystemTime::now())
                            .unwrap_or_default(),
                    )
                })
        });
    match response.status() {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            return Err(ProviderError::Unauthorized);
        }
        StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED => {
            return Err(ProviderError::UnavailableRoute);
        }
        StatusCode::TOO_MANY_REQUESTS => return Err(ProviderError::RateLimited),
        status if status.is_redirection() => return Err(ProviderError::RedirectBlocked),
        status if status.is_server_error() => return Err(ProviderError::EndpointUnavailable),
        status if !status.is_success() => return Err(ProviderError::RequestRejected),
        _ => {}
    }
    let mut body = Zeroizing::new(Vec::new());
    while let Some(chunk) = response.chunk().await.map_err(network_error)? {
        if body.len() + chunk.len() > 1024 * 1024 {
            return Err(ProviderError::InvalidResponse);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn network_error(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError::Timeout
    } else {
        ProviderError::NetworkUnavailable
    }
}
