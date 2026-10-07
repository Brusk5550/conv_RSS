use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::anyhow;
use reqwest::Url;
use scraper::{Html, Selector};

const URL: &str = "https://tokens.studio/blog";
const USER_AGENT: &str = "conv_rss/0.1 (personal RSS converter for tech watch)";
const CACHE_FILE: &str = "cache/tokens-studio.html";

/// One article found on the page.
struct Article {
    title: String,
    link: String,
}

fn main() -> anyhow::Result<()> {
    let html = load_html(URL, CACHE_FILE)?;
    let articles = extract_articles(&html, URL)?;

    println!("{} articles found:", articles.len());
    for article in &articles {
        println!("- {}", article.title);
        println!("  {}", article.link);
    }
    Ok(())
}

/// Returns the page HTML: from the cache file if it exists, otherwise downloads it.
fn load_html(url: &str, cache_file: &str) -> anyhow::Result<String> {
    if Path::new(cache_file).exists() {
        println!("Using cached HTML: {cache_file}");
        return Ok(fs::read_to_string(cache_file)?);
    }

    // Build an HTTP client with an honest User-Agent and a 20 s timeout.
    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(20))
        .build()?;

    // Download the page (one request), fail on HTTP errors like 404 or 500.
    println!("Downloading {url}");
    let html = client.get(url).send()?.error_for_status()?.text()?;

    // Save the HTML so we can test selectors without downloading again.
    fs::create_dir_all("cache")?;
    fs::write(cache_file, &html)?;
    Ok(html)
}

/// Finds the articles in the HTML. Relative links are resolved against `base_url`.
fn extract_articles(html: &str, base_url: &str) -> anyhow::Result<Vec<Article>> {
    let item_selector = parse_selector(r#"a[href^="./blog/"]"#)?;
    let title_selector = parse_selector("h2")?;
    let base = Url::parse(base_url)?;
    let document = Html::parse_document(html);

    let mut articles = Vec::new();
    for item in document.select(&item_selector) {
        // Title: text of the first <h2> inside the item. No title: skip the item.
        let title = match item.select(&title_selector).next() {
            Some(element) => element.text().collect::<String>().trim().to_string(),
            None => continue,
        };

        // Link: the item is itself the <a>, so read its own href.
        let href = match item.value().attr("href") {
            Some(href) => href,
            None => continue,
        };
        let link = base.join(href)?.to_string();

        articles.push(Article { title, link });
    }
    Ok(articles)
}

/// Parses a CSS selector, turning scraper's error into an anyhow error.
fn parse_selector(css: &str) -> anyhow::Result<Selector> {
    Selector::parse(css).map_err(|e| anyhow!("invalid CSS selector {css:?}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_title_and_absolute_link() {
        let html = r#"
            <nav><a href="./blog">Blog</a></nav>
            <a href="./blog/first-post"><h2> First post </h2><p>Written by</p></a>
            <a href="./blog/no-title"><p>No heading here</p></a>
            <a href="./pricing"><h2>Pricing</h2></a>
        "#;

        let articles = extract_articles(html, "https://tokens.studio/blog").unwrap();

        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].title, "First post");
        assert_eq!(articles[0].link, "https://tokens.studio/blog/first-post");
    }
}
