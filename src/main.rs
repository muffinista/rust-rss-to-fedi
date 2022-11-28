#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

use rocket_dyn_templates::{Template, tera::Tera, context};
use rocket::request::{self, FromRequest, Request};
use rocket::response::{Redirect};
use rocket::outcome::{Outcome};
use rocket::http::{Cookie, CookieJar};

use rocket::form::Form;
use rocket::http::Status;

use rocket::State;

use sqlx::sqlite::SqlitePool;

use std::env;

mod user;
use crate::user::User;

#[derive(FromForm)]
struct LoginForm {
    email: String
}

use crate::rocket::outcome::IntoOutcome;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for User {
    type Error = std::convert::Infallible;

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<User, Self::Error> {
        let pool = request.rocket().state::<SqlitePool>().unwrap();
        let cookie = request.cookies().get_private("access_token");

        match cookie {
            Some(cookie) => {
                let access_token = cookie.value();
                let user = User::find_by_access(&access_token.to_string(), &pool).await;
                match user {
                    Ok(user) => Outcome::Success(user),
                    Err(_why) => Outcome::Forward(())
                }
                //                    Outcome::Failure((Status::Unauthorized, Error::UnauthorizedError))
              

            },
            None => {
                return Outcome::Forward(())
            }
        }
    }
}


#[get("/")]
fn index_logged_in(user: User) -> Template {
    Template::render("home", context! { logged_in: true })
}

#[get("/", rank = 2)]
fn index() -> Template {
    Template::render("home", context! { logged_in: false })
}

#[get("/login")]
fn login() -> &'static str {
    "Hello, world!"
}

#[get("/user/auth/<login_token>")]
async fn attempt_login(db: &State<SqlitePool>, cookies: &CookieJar<'_>, login_token: &str) -> Result<Redirect, Status> {
    let user = User::find_by_login(&login_token.to_string(), &**db).await;
    
    match user {
        Ok(user) => {
            let token = User::apply_access_token(user, db).await;
            match token {
                Ok(token) => {
                    cookies
                        .add_private(Cookie::new("access_token", token.to_string()));
                    Ok(Redirect::to("/?yay=1"))
                },
                Err(why) => {
                    print!("{}", why);
                    Ok(Redirect::to("/?yay=0"))
                        //Err(Status::NotFound)
                }
            }
        },
        Err(why) => {
            print!("{}", why);
            Ok(Redirect::to("/?yay=3"))
//            Err(Status::NotFound)
        }
    }
}

#[post("/login", data = "<form>")]
async fn do_login(db: &State<SqlitePool>, form: Form<LoginForm>) -> Result<Redirect, Status> {
    let user = User::find_or_create_by_email(&form.email, &**db).await;

    // generate login token
    // send email
    // redirect

    
    match user {
        Ok(user) => {
            print!("/user/auth/{}", user.login_token);
            Ok(Redirect::to("/"))
        },
        Err(why) => {
            print!("{}", why);
            Err(Status::NotFound)
        }
    }
}


#[get("/account")]
fn account() -> &'static str {
    "Hello, world!"
}


pub fn customize(tera: &mut Tera) {
    tera.add_raw_template("about.html", r#"
        {% extends "base" %}
        {% block content %}
            <section id="about">
              <h1>About - Here's another page!</h1>
            </section>
        {% endblock content %}
    "#).expect("valid Tera template");
}

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    //let pool = SqlitePool::connect("sqlite:db.sqlite")

    let db_uri = env::var("DATABASE_URL").expect("DATABASE_URL is not set");
    let pool = SqlitePool::connect(&db_uri)
        .await
        .expect("Failed to create pool");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .ok();

    let _rocket = rocket::build()
        .manage(pool)
        .mount("/", routes![index, index_logged_in, login, do_login, attempt_login, account])
        .attach(Template::custom(|engines| {
            customize(&mut engines.tera);
        }))
        .launch()
        .await?;

    Ok(())
}

/*
#[launch]
fn rocket() -> _ {
    let pool = SqlitePool::connect(&env::var("DATABASE_URL")?).await?;
    rocket::build()
        // .attach(Db::init())
        .mount("/", routes![index, login, do_login, account])
        .attach(Template::custom(|engines| {
            customize(&mut engines.tera);
        }))
}
*/
