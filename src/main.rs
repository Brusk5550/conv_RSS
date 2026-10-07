use std::fs;
use std::time::Duration;

const URL: &str = "https://tokens.studio/blog";
const USER_AGENT: &str = "conv_rss/0.1 (personal RSS converter for tech watch)";
const CACHE_FILE: &str = "cache/tokens-studio.html";

fn main() -> anyhow::Result<()> {
    // Build an HTTP client with an honest User-Agent and a 20 s timeout.
    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(20))
        .build()?;

    // Download the page (one request), fail on HTTP errors like 404 or 500.
    let html = client.get(URL).send()?.error_for_status()?.text()?;

    // Save the HTML so we can test selectors without downloading again.
    fs::create_dir_all("cache")?;
    fs::write(CACHE_FILE, &html)?;

    println!("Downloaded {} bytes from {URL}", html.len());
    println!("Saved to {CACHE_FILE}");
    Ok(())
}
