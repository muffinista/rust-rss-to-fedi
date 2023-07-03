
use rocket::Request;
use rocket::http::Header;
use rocket::response::Responder;
use rocket::Response;


const ACTIVITY_JSON: &str = "application/activity+json";

/// Creates a response with the given content type and underlying responder.
///
/// # Example
///
/// ```rust
/// # use rocket::get;
/// use rocket::response::status;
/// use rocket::http::Status;
/// use rustypub::traits::CustomContentType;
///
/// # #[allow(unused_variables)]
/// #[get("/")]
/// fn handler() -> CustomContentType<String> {
///     CustomContentType("application/activity+json".to_string(), "Hi!".to_string())
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CustomContentType<R>(pub String, pub R);

/// Sets the content type of the response and then delegates the remainder of the
/// response to the wrapped responder.
impl<'r, 'o: 'r, R: Responder<'r, 'o>> Responder<'r, 'o> for CustomContentType<R> {
  fn respond_to(self, req: &'r Request<'_>) -> rocket::response::Result<'o> {
      Response::build_from(self.1.respond_to(req)?)
          .header(Header::new("Content-Type", self.0))
          .ok()
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActivityJsonContentType<R>(pub R);

///
/// Implement activity+json content-type response
///
impl<'r, 'o: 'r, R: Responder<'r, 'o>> Responder<'r, 'o> for ActivityJsonContentType<R> {
  fn respond_to(self, req: &'r Request<'_>) -> rocket::response::Result<'o> {
      Response::build_from(self.0.respond_to(req)?)
          .header(Header::new("Content-Type", ACTIVITY_JSON))
          .ok()
  }
}
