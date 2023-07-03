use rocket::{FromForm, get, post, put, delete};
use rocket::form::Form;
use rocket::http::Status;
use rocket::request::FlashMessage;
use rocket::response::{Flash, Redirect};
use rocket::State;
use rocket_dyn_templates::{Template, context};
use rocket::uri;
use rocket::serde::{Serialize, json::Json};

use fang::AsyncRunnable;
use fang::AsyncQueueable;

use std::env;

use sqlx::postgres::PgPool;

use crate::models::User;
use crate::models::Feed;
use crate::models::Item;
use crate::models::Setting;

use crate::services::url_to_feed::url_to_feed_url;

use crate::utils::queue::create_queue;


use crate::tasks::RefreshFeed;

#[derive(FromForm, serde::Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct FeedForm {
  name: String,
  url: String
}

#[derive(FromForm, serde::Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct FeedUpdateForm {
  listed: bool,
  status_publicity: Option<String>,
  content_warning: Option<String>,
  hashtag: Option<String>
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
pub struct FeedLookup {
  src: String,
  url: String,
  error: Option<String>
}

///
/// POST action to create a new feed
///
#[post("/feed", data = "<form>")]
pub async fn add_feed(user: User, db: &State<PgPool>, form: Form<FeedForm>) -> Result<Flash<Redirect>, Status> {
  let signups_enabled = Setting::value_or(&"signups_enabled".to_string(), &"true".to_string(), db).await.unwrap();

  if signups_enabled != "true" {
    return Ok(Flash::error(Redirect::to("/"), "Sorry, something went wrong!"));
  }

  //
  // follow the URL to make sure we add a valid RSS feed at this point
  //
  let url = url_to_feed_url(&form.url).await;
  match url {
    Err(_why) =>{
      Err(Status::NotFound)
    },
    Ok(result) => {
      if result.is_some() {
        let url = result.unwrap();
        let feed = Feed::create(&user, &url, &form.name, db).await;
  
        match feed {
          Ok(feed) => {
            let task = RefreshFeed { id: feed.id };
            let mut queue = create_queue().await;
            queue.connect(fang::NoTls).await.unwrap();

            queue
              .insert_task(&task as &dyn AsyncRunnable)
              .await
              .unwrap();
      
            let notify = user.send_link_to_feed(&feed, db).await;
            match notify {
              Ok(_notify) => log::info!("user notified!"),
              Err(why) => log::info!("something went wrong with notification: {why:?}")
            }
      
            let dest = uri!(show_feed(feed.name, Some(1)));
            Ok(Flash::success(Redirect::to(dest), "Feed created!"))
          },
          Err(why) => {
            log::info!("{why}");
            Ok(Flash::error(Redirect::to("/"), "Sorry, something went wrong!"))
          }
        }
      } else {
        Err(Status::NotFound)
      }
    }
  }
}

///
/// Update settings on a feed
///
#[put("/feed/<username>", data = "<form>")]
pub async fn update_feed(user: User, username: &str, db: &State<PgPool>, form: Form<FeedUpdateForm>) -> Result<Flash<Redirect>, Status> {
  let feed_lookup = Feed::find_by_user_and_name(&user, &username.to_string(), db).await;

  match feed_lookup {
    Ok(feed_lookup) => {
      match feed_lookup {
        Some(mut feed) => {
          feed.listed = form.listed;
          feed.hashtag = None;
          feed.content_warning = form.content_warning.clone();
          feed.hashtag = form.hashtag.clone();
          feed.status_publicity = form.status_publicity.clone();

          let result = feed.save(db).await;
          let dest = uri!(show_feed(&feed.name, None::<i32>));

          match result {
            Ok(_result) => Ok(Flash::success(Redirect::to(dest), "Feed updated!")),
            Err(_why) => Ok(Flash::error(Redirect::to(dest), "Sorry, something went wrong!"))
          }
        },
        None => Err(Status::NotFound)
      }
    },
    Err(_why) => Err(Status::NotFound)
  }
}

///
/// Take a potential URL/name for a feed and check if they are valid
///
#[post("/test-feed", data = "<form>")]
pub async fn test_feed(_user: User, db: &State<PgPool>, form: Json<FeedForm>) -> Result<Json<FeedLookup>, Status> {
  // check if feed name is already in use
  let feed_exists = Feed::exists_by_name(&form.name, db).await;

  if feed_exists.is_ok() && feed_exists.unwrap() {
    return Ok(Json(FeedLookup {
      src: form.url.to_string(),
      url: form.url.to_string(),
      error: Some("Sorry, that username is already taken".to_string())
    }))
  }

  // check if feed is valid
  let url = url_to_feed_url(&form.url).await;

  match url {
    Err(_why) => Err(Status::NotFound),
    Ok(result) => {
      if result.is_some() {
        Ok(Json(FeedLookup {
          src: form.url.to_string(),
          url: result.unwrap(),
          error: None
        }))
      } else {
        Err(Status::NotFound)
      }
    }
  }
}

///
/// Delete a feed
///
#[delete("/feed/<id>/delete")]
pub async fn delete_feed(user: User, id: i32, db: &State<PgPool>) -> Result<Redirect, Status> {
  let feed = Feed::delete(&user, id, db).await;
  
  match feed {
    Ok(_feed) => {
      Ok(Redirect::to("/"))
    },
    Err(why) => {
      print!("{why}");
      Err(Status::NotFound)
    }
  }
}

///
/// show a feed's HTML output
///
#[get("/feed/<username>?<added>", format = "text/html")]
pub async fn show_feed(user: Option<User>, username: &str, flash: Option<FlashMessage<'_>>, added: Option<i32>, db: &State<PgPool>) -> Result<Template, Status> {
  let feed_lookup = Feed::find_by_name(&username.to_string(), db).await;

  match feed_lookup {
    Ok(feed_lookup) => {
      match feed_lookup {
        Some(feed) => {
          let logged_in = user.is_some();
          let owned_by = logged_in && user.as_ref().unwrap().id == feed.user_id;
          let follow_url = feed.permalink_url();

          let items = if !owned_by && !feed.show_statuses_in_outbox() {
            Ok(Vec::<Item>::new())
          } else {
            Item::for_feed(&feed, 10, db).await
          };
          

          let username = if user.is_some() {
            user.as_ref().unwrap().full_username()
          } else {
            None
          };

          match items {
            Ok(items) => {
              Ok(Template::render("feed", context! {
                flash: flash,
                is_admin: feed.is_admin(),
                noindex: !feed.listed,
                logged_in: logged_in,
                username: username,
                owned_by: owned_by,
                feed: feed,
                items: items,
                follow_url: follow_url,
                added: added.is_some(),
                instance_domain: env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set")
              }))    
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

///
/// show a feed's ActivityPub output
///
#[get("/feed/<username>", format = "any", rank = 2)]
pub async fn render_feed(username: &str, db: &State<PgPool>) -> Result<String, Status> {
  let feed_lookup = Feed::find_by_name(&username.to_string(), db).await;

  match feed_lookup {
    Ok(feed_lookup) => {
      match feed_lookup {
        Some(feed) => {
          let ap = feed.to_activity_pub();
          match ap {
            Ok(ap) => Ok(ap),
            Err(_why) => Err(Status::NotFound)
          }
        },
        None => Err(Status::NotFound)
      }
    },
    Err(_why) => Err(Status::NotFound)
  }
}


///
/// Render the AP data for a feed's followers
///
#[get("/feed/<username>/followers?<page>")]
pub async fn render_feed_followers(username: &str, page: Option<i32>, db: &State<PgPool>) -> Result<String, Status> {
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
  use rocket::local::asynchronous::Client;
  use rocket::http::{Header, Status};
  use rocket::uri;
  use rocket::{Rocket, Build};

  use chrono::Utc;

  use crate::utils::test_helpers::{build_test_server, real_user, real_feed, real_item};
  use crate::utils::path_to_url;

  use crate::models::Feed;

  use sqlx::postgres::PgPool;
  
  #[sqlx::test]
  async fn test_show_feed(pool: PgPool) -> sqlx::Result<()> {
    let feed = real_feed(&pool).await.unwrap();

    for _i in 1..4 {
      real_item(&feed, &pool).await?;
    }


    let server: Rocket<Build> = build_test_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let req = client.get(uri!(super::show_feed(&feed.name, None::<i32>))).header(Header::new("Accept", "text/html"));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    
    let body = response.into_string().await.unwrap();
    println!("{:}", body);
    assert!(body.contains(&format!("Feed for {}", feed.name)));
    assert!(body.contains(&"Posted at"));

    Ok(())
  }
  
  #[sqlx::test]
  async fn test_show_feed_direct_publicity(pool: PgPool) -> sqlx::Result<()> {
    let mut feed:Feed = real_feed(&pool).await?;
    feed.status_publicity = Some("direct".to_string());
    feed.save(&pool).await?;

    for _i in 1..4 {
      real_item(&feed, &pool).await?;
    }


    let server: Rocket<Build> = build_test_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let req = client.get(uri!(super::show_feed(&feed.name, None::<i32>))).header(Header::new("Accept", "text/html"));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    
    let body = response.into_string().await.unwrap();
    assert!(body.contains(&format!("Feed for {}", feed.name)));
    assert!(body.contains(&"No entries"));

    Ok(())
  }

  #[sqlx::test]
  async fn test_render_feed(pool: PgPool) -> sqlx::Result<()> {
    let feed = real_feed(&pool).await.unwrap();

    let server: Rocket<Build> = build_test_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let name = feed.name;
    let req = client.get(uri!(super::render_feed(&name))).header(Header::new("Accept", "application/activity+json"));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    let body = response.into_string().await.unwrap();
    assert!(body.contains("-----BEGIN PUBLIC KEY-----"));
    assert!(body.contains(&name));

    Ok(())
  }

  #[sqlx::test]
  async fn test_render_feed_text_accept(pool: PgPool) -> sqlx::Result<()> {
    let feed = real_feed(&pool).await.unwrap();

    let server: Rocket<Build> = build_test_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let name = feed.name;
    let req = client.get(uri!(super::render_feed(&name))).header(Header::new("Accept", "text/plain"));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    let body = response.into_string().await.unwrap();
    assert!(body.contains("-----BEGIN PUBLIC KEY-----"));
    assert!(body.contains(&name));

    Ok(())
  }

  #[sqlx::test]
  async fn test_render_feed_json_accept(pool: PgPool) -> sqlx::Result<()> {
    let feed = real_feed(&pool).await.unwrap();

    let server: Rocket<Build> = build_test_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let name = feed.name;
    let req = client.get(uri!(super::render_feed(&name))).header(Header::new("Accept", "application/json"));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    let body = response.into_string().await.unwrap();
    assert!(body.contains("-----BEGIN PUBLIC KEY-----"));
    assert!(body.contains(&name));

    Ok(())
  }

  #[sqlx::test]
  async fn test_test_feed(pool: PgPool) -> sqlx::Result<()> {
    let user = real_user(&pool).await.unwrap();

    let server: Rocket<Build> = build_test_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    crate::utils::test_helpers::login_user(&client, &user).await;   
    
    let url: String = "https://muffinlabs.com/".to_string();
    let name: String = "testfeed".to_string();

    let json = format!(r#"{{"name":"{}","url": "{}"}}"#, name, url).to_string();
    
    let post = client.post(uri!(super::test_feed())).body(json);
    let response = post.dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    let body = response.into_string().await.unwrap();
    assert!(body.contains(r#"{"src":"https://muffinlabs.com/","url":"http://muffinlabs.com/atom.xml","error":null}"#));

    Ok(())
  }

  #[sqlx::test]
  async fn test_render_feed_followers(pool: PgPool) -> sqlx::Result<()> {
    let feed = real_feed(&pool).await.unwrap();
    let now = Utc::now();

    for i in 1..35 {
      let actor = format!("https://activitypub.pizza/users/colin{}", i);
      sqlx::query!("INSERT INTO followers (feed_id, actor, created_at, updated_at) VALUES($1, $2, $3, $4)", feed.id, actor, now, now)
        .execute(&pool)
        .await
        .unwrap();
    }
    
    let server: Rocket<Build> = build_test_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let name = feed.name;
    let req = client.get(uri!(super::render_feed_followers(&name, Some(2))));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    let body = response.into_string().await.unwrap();
 
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
