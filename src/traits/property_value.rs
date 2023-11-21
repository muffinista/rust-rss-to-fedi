// use activitystreams::{
//   base::{AsBase, Base, Extends},
//   markers,
//   object::*,
//   unparsed::*,
// };

use activitystreams::base::AnyBase;
use serde_json::json;


pub fn schema_property_context() -> Result<AnyBase, serde_json::Error> {
  let schema_property_context = json!({
    "schema": "http://schema.org#",
    "PropertyValue": "schema:PropertyValue",
    "value": "schema:value"
  });
  AnyBase::from_arbitrary_json(schema_property_context)  
}
