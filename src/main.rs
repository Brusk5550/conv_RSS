use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, anyhow};
use reqwest::Url;
use rss::{Channel, ChannelBuilder, GuidBuilder, Item, ItemBuilder};
use scraper::{Html, Selector};
use serde::Deserialize;

const CONFIG_FILE: &str = "sources.toml";
const USER_AGENT: &str = "conv_rss/0.1 (personal RSS converter for tech watch)";

/// The whole sources.toml file.
#[derive(Deserialize)]
struct Config {
    // Each [[source]] block in the file becomes one element of this list.
    #[serde(rename = "source")]
    sources: Vec<Source>,
}

/// One page to convert into an RSS feed.
#[derive(Deserialize)]
struct Source {
    id: String,
    name: String,
    url: String,
    enabled: bool,
    needs_js: bool,
    selectors: Selectors,
}

/// CSS selectors used to find the articles on the page.
#[derive(Deserialize)]
struct Selectors {
    item: String,
    title: String,
    // Optional: if absent, the link is read from the item element itself.
    link: Option<String>,
}

/// One article found on the page.
struct Article {
    title: String,
    link: String,
}

fn main() -> anyhow::Result<()> {
    let config = load_config(CONFIG_FILE)?;

    for source in &config.sources {
        if !source.enabled || source.needs_js {
            println!("[{}] skipped (disabled or needs JavaScript)", source.id);
            continue;
        }
        process_source(source)?;
    }
    Ok(())
}

/// Reads and parses the TOML config file.
fn load_config(path: &str) -> anyhow::Result<Config> {
    let text = fs::read_to_string(path).with_context(|| format!("cannot read {path}"))?;
    let config: Config = toml::from_str(&text).with_context(|| format!("invalid {path}"))?;
    Ok(config)
}

/// Downloads (or reads from cache) one source, extracts its articles and writes its feed.
fn process_source(source: &Source) -> anyhow::Result<()> {
    let cache_file = format!("cache/{}.html", source.id);
    let feed_file = format!("feeds/{}.xml", source.id);

    let html = load_html(&source.url, &cache_file)?;
    let articles = extract_articles(&html, &source.url, &source.selectors)?;

    println!("[{}] {} articles found:", source.id, articles.len());
    for article in &articles {
        println!("- {}", article.title);
        println!("  {}", article.link);
    }

    // Zero articles probably means the site changed its HTML: keep the old feed.
    if articles.is_empty() {
        println!("WARNING: no articles found, {feed_file} was not updated");
        return Ok(());
    }

    let channel = build_feed(&source.name, &source.url, &articles);
    fs::create_dir_all("feeds")?;
    let file = fs::File::create(&feed_file)?;
    channel.pretty_write_to(file, b' ', 2)?;
    println!("Feed written to {feed_file}");
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
fn extract_articles(
    html: &str,
    base_url: &str,
    selectors: &Selectors,
) -> anyhow::Result<Vec<Article>> {
    let item_selector = parse_selector(&selectors.item)?;
    let title_selector = parse_selector(&selectors.title)?;
    let link_selector = match &selectors.link {
        Some(css) => Some(parse_selector(css)?),
        None => None,
    };
    let base = Url::parse(base_url)?;
    let document = Html::parse_document(html);

    let mut articles = Vec::new();
    for item in document.select(&item_selector) {
        // Title: text of the first matching element inside the item. No title: skip the item.
        let title = match item.select(&title_selector).next() {
            Some(element) => element.text().collect::<String>().trim().to_string(),
            None => continue,
        };

        // Link element: found with the link selector, or the item itself if there is none.
        let link_element = match &link_selector {
            Some(selector) => match item.select(selector).next() {
                Some(element) => element,
                None => continue,
            },
            None => item,
        };
        let href = match link_element.value().attr("href") {
            Some(href) => href,
            None => continue,
        };
        let link = base.join(href)?.to_string();

        articles.push(Article { title, link });
    }
    Ok(articles)
}

/// Builds an RSS 2.0 channel from the articles.
fn build_feed(title: &str, page_url: &str, articles: &[Article]) -> Channel {
    let items: Vec<Item> = articles
        .iter()
        .map(|article| {
            // The guid is the article's absolute URL, so readers can spot new items.
            let guid = GuidBuilder::default()
                .value(article.link.clone())
                .permalink(true)
                .build();
            ItemBuilder::default()
                .title(article.title.clone())
                .link(article.link.clone())
                .guid(guid)
                .build()
        })
        .collect();

    ChannelBuilder::default()
        .title(title)
        .link(page_url)
        .description(format!("Articles from {page_url}, converted by conv_rss"))
        .items(items)
        .build()
}

/// Parses a CSS selector, turning scraper's error into an anyhow error.
fn parse_selector(css: &str) -> anyhow::Result<Selector> {
    Selector::parse(css).map_err(|e| anyhow!("invalid CSS selector {css:?}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_title_and_absolute_link_from_item() {
        let html = r#"
            <nav><a href="./blog">Blog</a></nav>
            <a href="./blog/first-post"><h2> First post </h2><p>Written by</p></a>
            <a href="./blog/no-title"><p>No heading here</p></a>
            <a href="./pricing"><h2>Pricing</h2></a>
        "#;
        let selectors = Selectors {
            item: r#"a[href^="./blog/"]"#.to_string(),
            title: "h2".to_string(),
            link: None,
        };

        let articles = extract_articles(html, "https://tokens.studio/blog", &selectors).unwrap();

        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].title, "First post");
        assert_eq!(articles[0].link, "https://tokens.studio/blog/first-post");
    }

    #[test]
    fn extracts_link_with_link_selector() {
        let html = r#"
            <article><h3>Post A</h3><a href="/posts/a">Read</a></article>
            <article><h3>Post without link</h3></article>
        "#;
        let selectors = Selectors {
            item: "article".to_string(),
            title: "h3".to_string(),
            link: Some("a".to_string()),
        };

        let articles = extract_articles(html, "https://example.com/news", &selectors).unwrap();

        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].title, "Post A");
        assert_eq!(articles[0].link, "https://example.com/posts/a");
    }

    #[test]
    fn parses_config() {
        let text = r#"
            [[source]]
            id = "demo"
            name = "Demo"
            url = "https://example.com"
            enabled = true
            needs_js = false

            [source.selectors]
            item = "article"
            title = "h2"
        "#;

        let config: Config = toml::from_str(text).unwrap();

        assert_eq!(config.sources.len(), 1);
        assert_eq!(config.sources[0].id, "demo");
        assert_eq!(config.sources[0].selectors.link, None);
    }

    #[test]
    fn feed_uses_link_as_guid_and_has_no_date() {
        let articles = vec![Article {
            title: "First post".to_string(),
            link: "https://tokens.studio/blog/first-post".to_string(),
        }];

        let channel = build_feed("Test feed", "https://tokens.studio/blog", &articles);

        assert_eq!(channel.items().len(), 1);
        let item = &channel.items()[0];
        assert_eq!(item.title(), Some("First post"));
        assert_eq!(item.link(), Some("https://tokens.studio/blog/first-post"));
        let guid = item.guid().unwrap();
        assert_eq!(guid.value(), "https://tokens.studio/blog/first-post");
        assert!(guid.is_permalink());
        assert_eq!(item.pub_date(), None);
    }
}
