#![feature(decl_macro)]

#[macro_use]
extern crate rocket;

// use rocket::Rocket;
// use rocket::response::content::RawHtml;
use rocket_dyn_templates::{Template, tera::Tera}; // , context

// use serde::Deserialize;

// #[get("/")]
// async fn hello() -> Result<HttpResponse> {
//     let template = liquid::ParserBuilder::with_stdlib()
//         .build().unwrap()
//         .parse(&std::fs::read_to_string("index.html")?).unwrap();

//     let vars = liquid::object!({"foo": "bar"});
//     let output = template.render(&vars).unwrap();
    
//     Ok(HttpResponse::Ok()
//         .content_type("text/html")
//         .body(output))
// }


// #[derive(Deserialize)]
// pub struct LoginParams {
//     email: String,
// }

// /// Simple handle POST request
// #[post("/login")]
// async fn login(params: web::Form<LoginParams>) -> Result<HttpResponse> {
//     Ok(HttpResponse::Ok()
//         .content_type("text/plain")
//         .body(format!("Your name is {}", params.email)))
// }

// /// extract path info from "/users/{user_id}/{friend}" url
// /// {user_id} - deserializes to a u32
// /// {friend} - deserializes to a String
// #[get("/users/{user_id}/{friend}")] // <- define path parameters
// async fn index(path: web::Path<(u32, String)>) -> Result<String> {
//     let (user_id, friend) = path.into_inner();

//     let template = liquid::ParserBuilder::with_stdlib()
//     .build().unwrap()
//     .parse("Liquid! {{user_id}} {{name}}").unwrap();

//     let globals = liquid::object!({
//         "user_id": user_id,
//         "name": friend
//     });

//     let output = template.render(&globals).unwrap();
//     Ok(output)
// }

// #[post("/echo")]
// async fn echo(req_body: String) -> impl Responder {
//     HttpResponse::Ok().body(req_body)
// }

// async fn manual_hello() -> impl Responder {
//     HttpResponse::Ok().body("Hey there!")
// }

#[get("/")]
fn index() -> &'static str {
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
    .mount("/", routes![index])
    // .mount("/tera", routes![tera::index, tera::hello, tera::about])
    // .mount("/hbs", routes![hbs::index, hbs::hello, hbs::about])
    // .register("/hbs", catchers![hbs::not_found])
    // .register("/tera", catchers![tera::not_found])
    .attach(Template::custom(|engines| {
        customize(&mut engines.tera);
    }))

    // rocket::build().mount("/", routes![index])
}
