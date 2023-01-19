use anyhow::{anyhow};
use anyhow::Error as AnyError;

use reqwest;
use feed_rs::parser;
use scraper::{Html, Selector};


pub fn is_valid_feed(data:&String) -> bool {
  parser::parse(data.as_bytes()).is_ok()
}

///
/// given a URL, determine if it's a valid feed, or try and find a feed
/// from any HTML returned
///
pub async fn url_to_feed_url(url:&String) -> Result<String, AnyError>{
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
      if is_valid_feed(&contents) {
        return Ok(url.clone())
      }

      // otherwise, parse and look for a link to a feed
      let document = Html::parse_document(&contents);

      // <link rel="alternate" type="application/rss+xml"
      // title="muffinlabs feed"
      // href="http://muffinlabs.com/atom.xml" />
      // <link rel="alternate" type="application/rss+xml" href="https://secretbroadcast.net/feed.rss" />

      let selector = Selector::parse(r#"link[rel="alternate"][href]"#).unwrap();
      let link = document.select(&selector).next();
      match link {
        Some(link) => Ok(link.value().attr("href").unwrap().to_string()),
        None => Err(anyhow!("Nothing found"))
      }
    },
    Err(err) => Err(anyhow!(err.to_string()))
  }
}
