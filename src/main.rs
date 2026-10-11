use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, anyhow};
use chrono::{DateTime, NaiveDate};
use reqwest::Url;
use rss::{Channel, ChannelBuilder, GuidBuilder, Item, ItemBuilder};
use scraper::{ElementRef, Html, Selector};
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
    // Optional: element holding the date (its datetime attribute, or its text).
    date: Option<String>,
    // Optional: element holding a short excerpt of the article.
    summary: Option<String>,
}

/// One article found on the page.
struct Article {
    title: String,
    link: String,
    // Publication date in RSS format (RFC 2822), if found and readable.
    date: Option<String>,
    summary: Option<String>,
}

/// What happened to one source during this run.
enum Outcome {
    /// Feed written with this number of articles.
    Written(usize),
    /// No article found: the old feed was kept.
    Empty,
    /// The source failed (download error, invalid selector...), with the error message.
    Failed(String),
    /// Disabled in the config, or needs JavaScript.
    Skipped,
}

fn main() -> anyhow::Result<()> {
    let config = load_config(CONFIG_FILE)?;

    // One (id, outcome) pair per source, for the summary at the end.
    let mut results = Vec::new();
    let mut failed = 0;

    for source in &config.sources {
        let outcome = if !source.enabled || source.needs_js {
            Outcome::Skipped
        } else {
            // One failing source must not stop the others: keep the error and continue.
            match process_source(source) {
                Ok(0) => Outcome::Empty,
                Ok(count) => Outcome::Written(count),
                Err(error) => {
                    failed += 1;
                    Outcome::Failed(format!("{error:#}"))
                }
            }
        };
        results.push((source.id.clone(), outcome));
    }

    println!();
    println!("Summary");
    for (id, outcome) in &results {
        println!("  {}", status_line(id, outcome));
    }

    // A non-zero exit code tells a scheduler (phase 6) that something went wrong.
    if failed > 0 {
        return Err(anyhow!("{failed} source(s) failed"));
    }
    Ok(())
}

/// One line of the summary, e.g. "OK       tokens-studio      3 articles".
fn status_line(id: &str, outcome: &Outcome) -> String {
    match outcome {
        Outcome::Written(count) => format!("OK       {id:<32} {count:>3} articles"),
        Outcome::Empty => format!("WARNING  {id:<32}   0 articles (feed not updated)"),
        Outcome::Failed(error) => format!("ERROR    {id:<32} {error}"),
        Outcome::Skipped => format!("SKIPPED  {id}"),
    }
}

/// Reads and parses the TOML config file.
fn load_config(path: &str) -> anyhow::Result<Config> {
    let text = fs::read_to_string(path).with_context(|| format!("cannot read {path}"))?;
    let config: Config = toml::from_str(&text).with_context(|| format!("invalid {path}"))?;
    Ok(config)
}

/// Downloads (or reads from cache) one source, extracts its articles and writes its feed.
/// Returns the number of articles found.
fn process_source(source: &Source) -> anyhow::Result<usize> {
    let cache_file = format!("cache/{}.html", source.id);
    let feed_file = format!("feeds/{}.xml", source.id);

    let html = load_html(&source.url, &cache_file)?;
    let articles = extract_articles(&html, &source.url, &source.selectors)?;

    // Zero articles probably means the site changed its HTML: keep the old feed.
    if articles.is_empty() {
        return Ok(0);
    }

    let channel = build_feed(&source.name, &source.url, &articles);
    fs::create_dir_all("feeds")?;
    let file = fs::File::create(&feed_file)?;
    channel.pretty_write_to(file, b' ', 2)?;
    Ok(articles.len())
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
    let link_selector = parse_optional_selector(&selectors.link)?;
    let date_selector = parse_optional_selector(&selectors.date)?;
    let summary_selector = parse_optional_selector(&selectors.summary)?;
    let base = Url::parse(base_url)?;
    let document = Html::parse_document(html);

    let mut articles = Vec::new();
    for item in document.select(&item_selector) {
        // Title: text of the first matching element inside the item. No title: skip the item.
        let title = match first_text(item, &title_selector) {
            Some(title) => title,
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

        // Date and summary are optional: no selector or nothing found gives None.
        let date = match &date_selector {
            Some(selector) => find_date(item, selector),
            None => None,
        };
        let summary = match &summary_selector {
            Some(selector) => first_text(item, selector),
            None => None,
        };

        articles.push(Article {
            title,
            link,
            date,
            summary,
        });
    }
    Ok(articles)
}

/// Text of the first element matching `selector` inside `item`, with whitespace
/// collapsed like a browser does. Returns None if there is no such element or if its text is empty.
fn first_text(item: ElementRef, selector: &Selector) -> Option<String> {
    let element = item.select(selector).next()?;
    let raw = element.text().collect::<String>();
    let text = raw.split_whitespace().collect::<Vec<&str>>().join(" ");
    if text.is_empty() { None } else { Some(text) }
}

/// Finds the date inside `item`: the datetime attribute if present, otherwise the text.
fn find_date(item: ElementRef, selector: &Selector) -> Option<String> {
    let element = item.select(selector).next()?;
    let raw = match element.value().attr("datetime") {
        Some(value) => value.to_string(),
        None => element.text().collect::<String>(),
    };
    parse_date(raw.trim())
}

/// Converts a date found on a page to the RSS format ("Fri, 18 Jul 2025 00:00:00 +0000").
/// Unreadable date: None, never a made-up date.
fn parse_date(raw: &str) -> Option<String> {
    // 1. ISO 8601 with time and timezone, e.g. "2025-07-18T00:00:00+00:00".
    if let Ok(date) = DateTime::parse_from_rfc3339(raw) {
        return Some(date.to_rfc2822());
    }

    // 2. Day only, e.g. "Oct 9, 2026". The page gives no time, so use midnight UTC.
    if let Ok(day) = NaiveDate::parse_from_str(raw, "%b %d, %Y") {
        let midnight = day.and_hms_opt(0, 0, 0)?;
        return Some(midnight.and_utc().to_rfc2822());
    }

    None
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
                .pub_date(article.date.clone())
                .description(article.summary.clone())
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

/// Same as `parse_selector`, for a selector that may be absent from the config.
fn parse_optional_selector(css: &Option<String>) -> anyhow::Result<Option<Selector>> {
    match css {
        Some(css) => Ok(Some(parse_selector(css)?)),
        None => Ok(None),
    }
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
            date: None,
            summary: None,
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
            date: None,
            summary: None,
        };

        let articles = extract_articles(html, "https://example.com/news", &selectors).unwrap();

        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].title, "Post A");
        assert_eq!(articles[0].link, "https://example.com/posts/a");
    }

    #[test]
    fn extracts_date_and_summary() {
        let html = r#"
            <article class="card">
              <a href="/literature/article/a">Read</a>
              <h2> Post A </h2>
              <p> Short excerpt </p>
              <time datetime="2025-07-18T00:00:00+00:00">1 year ago</time>
            </article>
            <article class="card">
              <a href="/literature/article/b">Read</a>
              <h2>Post B</h2>
              <time>yesterday</time>
            </article>
        "#;
        let selectors = Selectors {
            item: "article.card".to_string(),
            title: "h2".to_string(),
            link: Some("a".to_string()),
            date: Some("time".to_string()),
            summary: Some("p".to_string()),
        };

        let articles = extract_articles(html, "https://ixdf.org/literature", &selectors).unwrap();

        assert_eq!(articles.len(), 2);
        assert_eq!(articles[0].link, "https://ixdf.org/literature/article/a");
        assert_eq!(
            articles[0].date.as_deref(),
            Some("Fri, 18 Jul 2025 00:00:00 +0000")
        );
        assert_eq!(articles[0].summary.as_deref(), Some("Short excerpt"));
        // Unreadable date and missing summary: left empty, nothing made up.
        assert_eq!(articles[1].date, None);
        assert_eq!(articles[1].summary, None);
    }

    #[test]
    fn parse_date_reads_iso_and_rejects_garbage() {
        assert_eq!(
            parse_date("2026-10-01T14:00:00+00:00").as_deref(),
            Some("Thu, 1 Oct 2026 14:00:00 +0000")
        );
        assert_eq!(parse_date("1 week ago"), None);
        assert_eq!(parse_date(""), None);
    }

    #[test]
    fn parse_date_reads_month_day_year_as_midnight_utc() {
        assert_eq!(
            parse_date("Oct 9, 2026").as_deref(),
            Some("Fri, 9 Oct 2026 00:00:00 +0000")
        );
        assert_eq!(
            parse_date("Sep 30, 2026").as_deref(),
            Some("Wed, 30 Sep 2026 00:00:00 +0000")
        );
        assert_eq!(parse_date("Foo 99, 2026"), None);
    }

    #[test]
    fn summary_selector_skips_date_paragraph() {
        let html = r#"
            <div class="post group">
              <header>
                <p class="date">Oct 9, 2026</p>
                <h2><a href="/stories/2026/10/09/a">Story A</a></h2>
                <div class="teaser"><a href="/stories/2026/10/09/a"><img src="x.png"></a></div>
              </header>
              <p>The real
                 summary.</p>
            </div>
        "#;
        let selectors = Selectors {
            item: "div.post".to_string(),
            title: "h2".to_string(),
            link: Some("h2 a".to_string()),
            date: Some("p.date".to_string()),
            summary: Some("p:not(.date)".to_string()),
        };

        let articles = extract_articles(html, "https://dribbble.com/stories", &selectors).unwrap();

        assert_eq!(articles.len(), 1);
        assert_eq!(
            articles[0].link,
            "https://dribbble.com/stories/2026/10/09/a"
        );
        assert_eq!(articles[0].summary.as_deref(), Some("The real summary."));
        assert_eq!(
            articles[0].date.as_deref(),
            Some("Fri, 9 Oct 2026 00:00:00 +0000")
        );
    }

    #[test]
    fn link_selector_skips_author_link() {
        let html = r#"
            <div class="qa-topic-block topic-view js-thread">
              <a class="qa-username" href="https://forum.figma.com/members/djv-115737">djv</a>
              <a class="qa-topic-title" href="https://forum.figma.com/product-updates-3/topic-1">
                <span class="thread-list-block__title-link">Topic 1</span>
                <p class="thread-list-block__content"><span>Summary 1</span></p>
              </a>
              <time dateTime="2026-09-25T16:02:45.000Z">16 days ago</time>
            </div>
        "#;
        let selectors = Selectors {
            item: "div.qa-topic-block".to_string(),
            title: ".thread-list-block__title-link".to_string(),
            link: Some("a.qa-topic-title".to_string()),
            date: None,
            summary: Some("p.thread-list-block__content".to_string()),
        };

        let articles = extract_articles(
            html,
            "https://forum.figma.com/product-updates-3",
            &selectors,
        )
        .unwrap();

        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].title, "Topic 1");
        assert_eq!(
            articles[0].link,
            "https://forum.figma.com/product-updates-3/topic-1"
        );
        assert_eq!(articles[0].summary.as_deref(), Some("Summary 1"));
        // No date selector: the <time> is ignored.
        assert_eq!(articles[0].date, None);
    }

    #[test]
    fn status_line_for_each_outcome() {
        assert_eq!(
            status_line("demo", &Outcome::Written(3)),
            format!("OK       {:<32}   3 articles", "demo")
        );
        assert!(status_line("demo", &Outcome::Empty).starts_with("WARNING  demo"));
        assert!(status_line("demo", &Outcome::Empty).ends_with("(feed not updated)"));
        assert!(
            status_line("demo", &Outcome::Failed("dns error".to_string()))
                .starts_with("ERROR    demo")
        );
        assert!(
            status_line("demo", &Outcome::Failed("dns error".to_string())).ends_with("dns error")
        );
        assert_eq!(status_line("demo", &Outcome::Skipped), "SKIPPED  demo");
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
            date: None,
            summary: None,
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
