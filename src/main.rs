#![feature(proc_macro_hygiene, decl_macro)]
// #![feature(decl_macro)]

#[macro_use]
extern crate rocket;

use rocket_dyn_templates::{Template, tera::Tera, context};

use rocket_db_pools::{sqlx, Database};

#[derive(Database)]
#[database("sqlite_logs")]
struct Logs(sqlx::SqlitePool);

use rocket::form::Form;

#[derive(FromForm)]
struct LoginForm {
    email: String
}


#[get("/")]
fn index() -> Template {
    Template::render("home", context! { })
}

#[get("/login")]
fn login() -> &'static str {
    "Hello, world!"
}

use rocket_db_pools::Connection;
use rocket_db_pools::sqlx::Row;

#[post("/login", data = "<form>")]
async fn do_login(mut db: Connection<Logs>, form: Form<LoginForm>) -> &'static str {
    // sqlx::query("SELECT * FROM users WHERE email = ?").bind(form.email)
    //     .fetch_one(&mut *db).await
    //     .and_then(|r| Ok(r.try_get(0)?))
    //     .ok()
    "Hello, world!"
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

#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(Logs::init())
        .mount("/", routes![index, login, do_login, account])
        .attach(Template::custom(|engines| {
            customize(&mut engines.tera);
        }))
}
