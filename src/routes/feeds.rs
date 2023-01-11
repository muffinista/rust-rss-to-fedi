use rocket::{FromForm, get, post};
use rocket::form::Form;
use rocket::http::Status;
use rocket::response::Redirect;
use rocket::State;
use rocket_dyn_templates::{Template, context};
use rocket::uri;

use sqlx::sqlite::SqlitePool;

use crate::models::user::User;
use crate::models::feed::Feed;

#[derive(FromForm)]
pub struct FeedForm {
  name: String,
  url: String
}

#[post("/feed", data = "<form>")]
pub async fn add_feed(user: User, db: &State<SqlitePool>, form: Form<FeedForm>) -> Result<Redirect, Status> {
  let feed = Feed::create(&user, &form.url, &form.name, &db).await;
  
  match feed {
    Ok(feed) => {
      let dest = uri!(show_feed(feed.name));
      Ok(Redirect::to(dest))
    },
    Err(why) => {
      print!("{}", why);
      Err(Status::NotFound)
    }
  }
}


// @todo use proper verb
#[get("/feed/<id>/delete")]
pub async fn delete_feed(user: User, id: i64, db: &State<SqlitePool>) -> Result<Redirect, Status> {
  let feed = Feed::delete(&user, id, &db).await;
  
  match feed {
    Ok(_feed) => {
      Ok(Redirect::to("/"))
    },
    Err(why) => {
      print!("{}", why);
      Err(Status::NotFound)
    }
  }
}

#[get("/feed/<username>", format = "application/activity+json")]
pub async fn render_feed(username: &str, db: &State<SqlitePool>) -> Result<String, Status> {
  let feed_lookup = Feed::find_by_name(&username.to_string(), db).await;

  match feed_lookup {
    Ok(feed_lookup) => {
      match feed_lookup {
        Some(feed) => {
          println!("generate output");
          let ap = feed.to_activity_pub();
          match ap {
            Ok(ap) => {
              //let output = serde_json::to_string(&ap).unwrap();
              println!("{}", ap);
  
              //Ok(output)
              Ok(ap)
            },
            Err(_why) => Err(Status::NotFound)
          }
        },
        None => Err(Status::NotFound)
      }
    },
    Err(_why) => Err(Status::NotFound)
  }
}

#[get("/feed/<username>", format = "text/html", rank = 2)]
pub async fn show_feed(user: Option<User>, username: &str, db: &State<SqlitePool>) -> Result<Template, Status> {
  let feed_lookup = Feed::find_by_name(&username.to_string(), db).await;

  match feed_lookup {
    Ok(feed_lookup) => {
      match feed_lookup {
        Some(feed) => {
          let logged_in = user.is_some();
          let owned_by = logged_in && user.unwrap().id == feed.user_id;
          let feed_url = uri!(show_feed(&feed.name));

          Ok(Template::render("feed", context! {
            logged_in: logged_in,
            owned_by: owned_by,
            feed: feed,
            feed_url: feed_url
          }))
        },
        None => Err(Status::NotFound)
      }
    },
    Err(_why) => Err(Status::NotFound)
  }
}

#[get("/feed/<username>/followers?<page>")]
pub async fn render_feed_followers(username: &str, page: Option<u32>, db: &State<SqlitePool>) -> Result<String, Status> {
  let feed_lookup = Feed::find_by_name(&username.to_string(), db).await;

  match feed_lookup {
    Ok(feed_lookup) => {
      match feed_lookup {
        Some(feed) => {
          // if we got a page param, return a page of followers
          // otherwise, return the summary
          let json = match page {
            Some(page) => {
              let result = feed.followers_paged(page, db).await;
              match result {
                Ok(result) => Ok(serde_json::to_string(&result).unwrap()),
                Err(_why) => Err(Status::InternalServerError)
              }
            },
            None => {
              let result = feed.followers(db).await;
              match result {
                Ok(result) => Ok(serde_json::to_string(&result).unwrap()),
                Err(_why) => Err(Status::InternalServerError)
              }
            }
          };
      
          Ok(json.unwrap())
        },
        None => Err(Status::NotFound)
      }
    },
    Err(_why) => Err(Status::InternalServerError)
  }
}

#[cfg(test)]
mod test {
  use crate::server::build_server;
  use rocket::local::asynchronous::Client;
  use rocket::http::{Header, Status};
  use rocket::uri;
  use rocket::{Rocket, Build};
  use crate::models::user::User;
  use crate::models::feed::Feed;
  use crate::utils::*;
  use chrono::Utc;

  use sqlx::sqlite::SqlitePool;
  
  #[sqlx::test]
  async fn test_show_feed(pool: SqlitePool) -> sqlx::Result<()> {
    let user = User { id: 1, email: "foo@bar.com".to_string(), login_token: "lt".to_string(), access_token: Some("at".to_string()), created_at: Utc::now().naive_utc(), updated_at: Utc::now().naive_utc() };

    let url: String = "https://foo.com/rss.xml".to_string();
    let name: String = "testfeed".to_string();

    Feed::create(&user, &url, &name, &pool).await?;

    let server:Rocket<Build> = build_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let req = client.get(uri!(super::show_feed(&name))).header(Header::new("Accept", "text/html"));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    
    let body = response.into_string().await.unwrap();
    println!("{:?}", body);
    assert!(body.contains("Welcome to the feed page"));
    assert!(body.contains(&name));

    Ok(())
  }

  // https://api.rocket.rs/v0.5-rc/rocket/local/blocking/struct.LocalRequest.html

  #[sqlx::test]
  async fn test_render_feed(pool: SqlitePool) -> sqlx::Result<()> {
    let user = User { id: 1, email: "foo@bar.com".to_string(), login_token: "lt".to_string(), access_token: Some("at".to_string()), created_at: Utc::now().naive_utc(), updated_at: Utc::now().naive_utc() };

    let url: String = "https://foo.com/rss.xml".to_string();
    let name: String = "testfeed".to_string();

    Feed::create(&user, &url, &name, &pool).await?;

    let server: Rocket<Build> = build_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let req = client.get(uri!(super::render_feed(&name))).header(Header::new("Accept", "application/activity+json"));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    let body = response.into_string().await.unwrap();
    println!("{:?}", body);
    assert!(body.contains("-----BEGIN PUBLIC KEY-----"));
    assert!(body.contains(&name));

    Ok(())
  }

  #[sqlx::test]
  async fn test_render_feed_followers(pool: SqlitePool) -> sqlx::Result<()> {
    let user = User { id: 1, email: "foo@bar.com".to_string(), login_token: "lt".to_string(), access_token: Some("at".to_string()), created_at: Utc::now().naive_utc(), updated_at: Utc::now().naive_utc() };

    let url: String = "https://foo.com/rss.xml".to_string();
    let name: String = "testfeed".to_string();

    let feed = Feed::create(&user, &url, &name, &pool).await?;

    for i in 1..35 {
      let actor = format!("https://activitypub.pizza/users/colin{}", i);
      sqlx::query!("INSERT INTO followers (feed_id, actor, created_at, updated_at) VALUES($1, $2, datetime(CURRENT_TIMESTAMP, 'utc'), datetime(CURRENT_TIMESTAMP, 'utc'))", feed.id, actor)
        .execute(&pool)
        .await
        .unwrap();
    }

    
    let server: Rocket<Build> = build_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let req = client.get(uri!(super::render_feed_followers(&name, Some(2))));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    let body = response.into_string().await.unwrap();
    println!("{:?}", body);


    assert!(body.contains("OrderedCollectionPage"));
    assert!(body.contains("/colin11"));
    assert!(body.contains("/colin12"));
    assert!(body.contains("/colin13"));
    assert!(body.contains(&format!(r#"first":"{}"#, path_to_url(&uri!(super::render_feed_followers(name.clone(), Some(1)))))));
    assert!(body.contains(&format!(r#"prev":"{}"#, path_to_url(&uri!(super::render_feed_followers(name.clone(), Some(1)))))));      
    assert!(body.contains(&format!(r#"next":"{}"#, path_to_url(&uri!(super::render_feed_followers(name.clone(), Some(3)))))));
    assert!(body.contains(&format!(r#"last":"{}"#, path_to_url(&uri!(super::render_feed_followers(name.clone(), Some(4)))))));
    assert!(body.contains(&format!(r#"current":"{}"#, path_to_url(&uri!(super::render_feed_followers(name.clone(), Some(2)))))));
    
    Ok(())
  }
}
