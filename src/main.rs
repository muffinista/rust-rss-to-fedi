#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

use rocket_dyn_templates::{Template, tera::Tera, context};
use rocket::request::{self, FromRequest, Request};
use rocket::response::{Redirect};
use rocket::outcome::{Outcome};

use rocket::form::Form;
use rocket::http::Status;

use rocket::State;

use sqlx::sqlite::SqlitePool;

mod user;
use crate::user::User;

#[derive(FromForm)]
struct LoginForm {
    email: String
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for User {
    type Error = std::convert::Infallible;

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<User, Self::Error> {
        let pool = request.rocket().state::<SqlitePool>().unwrap();
        let access_token = request.cookies()
            .get_private("access_token")
            .and_then(|cookie| cookie.value().parse().ok());

        match access_token {
            Some(access_token) => {
                let user_check = User::find_by_access(access_token, pool).await;
                match user_check {
                    Ok(user_check) => {
                        Outcome::Success(user_check)
                    }
                    Err(_why) => Outcome::Forward(())
                }
            }
            None => Outcome::Forward(())
        }
    }
}


#[get("/")]
fn index() -> Template {
    Template::render("home", context! { })
}

#[get("/login")]
fn login() -> &'static str {
    "Hello, world!"
}

#[post("/login", data = "<form>")]
async fn do_login(db: &State<SqlitePool>, form: Form<LoginForm>) -> Result<Redirect, Status> {
    let user = User::find_or_create_by_email((form.email).to_string(), &**db).await;

    // generate login token
    // send email
    // redirect

    
    match user {
        Ok(_user) => Ok(Redirect::to("/")),
        _ => Err(Status::NotFound)
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
    let pool = SqlitePool::connect("sqlite::memory")
        .await
        .expect("Failed to create pool");
    
    let _rocket = rocket::build()
        .manage(pool)
        .mount("/", routes![index, login, do_login, account])
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
