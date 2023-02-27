use anyhow::{anyhow};
use anyhow::Error as AnyError;

use reqwest;
use feed_rs::parser;
use scraper::{Html, Selector};


pub fn is_valid_feed(data:&String) -> bool {
  let result = parser::parse(data.as_bytes());
  
  if result.is_err() {
    return false;
  }
  
  // don't load mastodon feeds
  // this prevents a pretty obvious way to use this service for block evasion
  let generator = result.unwrap().generator;
  if generator.is_some() && generator.unwrap().content.contains("Mastodon ") {
    return false;
  }

  true
}

///
/// given a URL, determine if it's a valid feed, or try and find a feed
/// from any HTML returned
///
pub async fn url_to_feed_url(url:&String) -> Result<Option<String>, AnyError>{
  // grab the URL contents
  let res = reqwest::get(url).await;
  if let Err(err) = res {
    return Err(anyhow!(err.to_string()))
  }

  // Response: HTTP/1.1 200 OK
  // Headers: {
  //     "date": "Tue, 29 Nov 2022 00:48:07 GMT",
  //     "content-type": "application/xml",
  //     "content-length": "68753",
  //     "connection": "keep-alive",
  //     "last-modified": "Tue, 08 Nov 2022 13:54:18 GMT",
  //     "etag": "\"10c91-5ecf5e04f7680\"",
  //     "accept-ranges": "bytes",
  //     "strict-transport-security": "max-age=15724800; includeSubDomains",
  // }
  // eprintln!("Response: {:?} {}", res.version(), res.status());
  // eprintln!("Headers: {:#?}\n", res.headers());

  let contents = &res.unwrap().text().await;
  match contents {
    Ok(contents) => {
      // if it's a valid feed, we're good
      if is_valid_feed(contents) {
        return Ok(Some(url.clone()))
      }

      // otherwise, parse and look for a link to a feed
      let document = Html::parse_document(contents);

      // <link rel="alternate" type="application/rss+xml"
      // title="muffinlabs feed"
      // href="http://muffinlabs.com/atom.xml" />
      // <link rel="alternate" type="application/rss+xml" href="https://secretbroadcast.net/feed.rss" />

      let selector = Selector::parse(r#"link[rel="alternate"][href]"#).unwrap();
      let link = document.select(&selector).next();
      match link {
        Some(link) => Ok(Some(link.value().attr("href").unwrap().to_string())),
        None => Ok(None)
      }
    },
    Err(err) => Err(anyhow!(err.to_string()))
  }
}


#[cfg(test)]
mod test {
  use mockito::mock;
  use std::fs;

  use crate::services::url_to_feed::url_to_feed_url;

  #[tokio::test]
  async fn test_valid_direct_feed_url() -> Result<(), String>  {
    let path = "fixtures/test_feed_to_entries.xml";
    let data = fs::read_to_string(path).unwrap();

    let _m = mock("GET", "/feed.xml")
      .with_status(200)
      .with_body(data)
      .create();

    let feed_url = format!("{}/feed.xml", &mockito::server_url()).to_string();

    let result = url_to_feed_url(&feed_url).await.unwrap();

    match result {
      Some(result) => assert_eq!(feed_url, result),
      None => assert_eq!(false, true)
    }

    Ok(())
  }

  #[tokio::test]
  async fn test_html_with_feed_link() -> Result<(), String>  {
    let path = "fixtures/test_html_with_feed_link.html";
    let data = fs::read_to_string(path).unwrap();

    let _m = mock("GET", "/")
      .with_status(200)
      .with_body(data)
      .create();

    let page_url = format!("{}/", &mockito::server_url()).to_string();
    let feed_url = "http://testfeed.com/atom.xml";

    let result = url_to_feed_url(&page_url).await.unwrap();

    match result {
      Some(result) => assert_eq!(feed_url, result),
      None => assert_eq!(false, true)
    }

    Ok(())
  }

  #[tokio::test]
  async fn test_html_with_no_feed_link() -> Result<(), String>  {
    let path = "fixtures/test_html_with_no_feed_link.html";
    let data = fs::read_to_string(path).unwrap();

    let _m = mock("GET", "/")
      .with_status(200)
      .with_body(data)
      .create();

    let page_url = format!("{}/", &mockito::server_url()).to_string();

    let result = url_to_feed_url(&page_url).await.unwrap();

    assert!(result.is_none());

    Ok(())
  }

  #[tokio::test]
  async fn test_html_with_server_error() -> Result<(), String>  {
    let _m = mock("GET", "/")
      .with_status(500)
      .create();

    let page_url = format!("{}/", &mockito::server_url()).to_string();

    let result = url_to_feed_url(&page_url).await.unwrap();

    assert!(result.is_none());

    Ok(())
  }


  #[tokio::test]
  async fn test_404_feed_url() -> Result<(), String>  {
    let _m = mock("GET", "/feed.xml")
      .with_status(404)
      .create();

    let feed_url = format!("{}/feed.xml", &mockito::server_url()).to_string();

    let result = url_to_feed_url(&feed_url).await.unwrap();

    assert!(result.is_none());

    Ok(())
  }
}
