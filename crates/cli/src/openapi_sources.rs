use super::*;
use std::io::Read;

const MAX_OPENAPI_DOWNLOAD_BYTES: u64 = 128 * 1024 * 1024;
const MAX_API_DIRECTORY_BYTES: u64 = 16 * 1024 * 1024;

fn load_api_directory() -> Result<registry::Directory> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("kaji/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("configure API directory client")?;
    let response = client
        .get(registry::DIRECTORY_URL)
        .send()
        .context("download API directory")?
        .error_for_status()
        .context("download API directory")?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_API_DIRECTORY_BYTES)
    {
        bail!(
            "API directory is larger than the {} MiB download limit",
            MAX_API_DIRECTORY_BYTES / 1024 / 1024
        )
    }
    let mut document = Vec::new();
    response
        .take(MAX_API_DIRECTORY_BYTES + 1)
        .read_to_end(&mut document)
        .context("read API directory")?;
    if document.len() as u64 > MAX_API_DIRECTORY_BYTES {
        bail!(
            "API directory is larger than the {} MiB download limit",
            MAX_API_DIRECTORY_BYTES / 1024 / 1024
        )
    }
    let document = std::str::from_utf8(&document).context("API directory is not UTF-8")?;
    registry::Directory::parse(document)
}

pub(super) fn discover(options: Discover) -> Result<()> {
    let directory = load_api_directory()?;
    let apis = directory.search(&options.query, options.limit);
    match options.format {
        DiscoverFormat::Json => println!("{}", serde_json::to_string_pretty(&apis)?),
        DiscoverFormat::Human => {
            if apis.is_empty() {
                println!("No OpenAPI directory entries matched {:?}.", options.query);
                return Ok(());
            }
            for api in apis {
                println!("{}  {}  {}", api.id, api.version, api.title);
                if let Some(description) = api.description {
                    println!("  {description}");
                }
                println!("  {}", api.openapi_url);
            }
        }
    }
    Ok(())
}

pub(super) fn download(options: Download) -> Result<()> {
    if options.output.exists() {
        bail!(
            "refusing to overwrite existing file {}; choose a new --output path",
            options.output.display()
        )
    }
    let directory = load_api_directory()?;
    let api = directory.resolve(&options.id, options.version.as_deref())?;
    registry::validate_download(&api)?;
    let source = RemoteInput {
        url: api.openapi_url.clone(),
        headers: BTreeMap::new(),
        auth: None,
    };
    download_openapi(&source, &options.output)?;
    println!(
        "Downloaded {} version {} to {}",
        api.id,
        api.version,
        options.output.display()
    );
    Ok(())
}

pub(super) fn remote_spec_url(source: &Path) -> Option<&str> {
    let source = source.to_str()?;
    (source.starts_with("https://") || source.starts_with("http://")).then_some(source)
}

pub(super) fn compiler_source_origin(remote: &RemoteInput) -> Result<String> {
    let mut url = reqwest::Url::parse(&remote.url).context("invalid OpenAPI source URL")?;
    url.set_username("")
        .map_err(|_| anyhow::anyhow!("cannot sanitize OpenAPI source URL"))?;
    url.set_password(None)
        .map_err(|_| anyhow::anyhow!("cannot sanitize OpenAPI source URL"))?;
    Ok(url.to_string())
}

pub(super) fn download_openapi(source: &RemoteInput, destination: &Path) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("kaji/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .context("configure OpenAPI download client")?;
    let mut response = remote_request(&client, source)?
        .send()
        .with_context(|| format!("download OpenAPI document {}", source.url))?
        .error_for_status()
        .with_context(|| format!("download OpenAPI document {}", source.url))?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_OPENAPI_DOWNLOAD_BYTES)
    {
        bail!(
            "OpenAPI document is larger than the {} MiB download limit",
            MAX_OPENAPI_DOWNLOAD_BYTES / 1024 / 1024
        );
    }
    let mut file = std::fs::File::create(destination)
        .with_context(|| format!("create downloaded OpenAPI file {}", destination.display()))?;
    let copied = std::io::copy(&mut response, &mut file)
        .with_context(|| format!("save downloaded OpenAPI document from {}", source.url))?;
    if copied > MAX_OPENAPI_DOWNLOAD_BYTES {
        bail!(
            "OpenAPI document is larger than the {} MiB download limit",
            MAX_OPENAPI_DOWNLOAD_BYTES / 1024 / 1024
        );
    }
    Ok(())
}

pub(super) fn remote_request(
    client: &reqwest::blocking::Client,
    source: &RemoteInput,
) -> Result<reqwest::blocking::RequestBuilder> {
    let mut request = client.get(&source.url);
    for (name, value) in &source.headers {
        let value = value.resolve(&format!("header {name:?}"))?;
        request = request.header(name, value);
    }
    if let Some(auth) = &source.auth {
        request = match auth {
            RemoteAuth::Basic { username, password } => request.basic_auth(
                username.resolve("basic authentication username")?,
                Some(password.resolve("basic authentication password")?),
            ),
            RemoteAuth::Bearer { token } => {
                request.bearer_auth(token.resolve("bearer authentication token")?)
            }
        };
    }
    Ok(request)
}
