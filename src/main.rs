use actix_web::{get, post, web, App, HttpResponse, HttpServer, Result, Responder};
use serde::Deserialize;

#[get("/")]
async fn hello() -> Result<HttpResponse> {
    let template = liquid::ParserBuilder::with_stdlib()
        .build().unwrap()
        .parse(&std::fs::read_to_string("index.html")?).unwrap();

    let vars = liquid::object!({"foo": "bar"});
    let output = template.render(&vars).unwrap();
    
    Ok(HttpResponse::Ok()
        .content_type("text/html")
        .body(output))
}


#[derive(Deserialize)]
pub struct LoginParams {
    email: String,
}

/// Simple handle POST request
#[post("/login")]
async fn login(params: web::Form<LoginParams>) -> Result<HttpResponse> {
    Ok(HttpResponse::Ok()
        .content_type("text/plain")
        .body(format!("Your name is {}", params.email)))
}

/// extract path info from "/users/{user_id}/{friend}" url
/// {user_id} - deserializes to a u32
/// {friend} - deserializes to a String
#[get("/users/{user_id}/{friend}")] // <- define path parameters
async fn index(path: web::Path<(u32, String)>) -> Result<String> {
    let (user_id, friend) = path.into_inner();

    let template = liquid::ParserBuilder::with_stdlib()
    .build().unwrap()
    .parse("Liquid! {{user_id}} {{name}}").unwrap();

    let globals = liquid::object!({
        "user_id": user_id,
        "name": friend
    });

    let output = template.render(&globals).unwrap();
    Ok(output)
}

#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}

async fn manual_hello() -> impl Responder {
    HttpResponse::Ok().body("Hey there!")
}



#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
        .service(hello)
        .service(login)
        .service(index)
        .service(echo)
            .route("/hey", web::get().to(manual_hello))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}