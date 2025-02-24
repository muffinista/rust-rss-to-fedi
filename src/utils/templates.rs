
pub fn render(
  template: &str,
  tmpl: &tera::Tera,
  context: &tera::Context,
) -> Result<String, tera::Error> {
 // let current = template.replace(".j2", "");
  tmpl.render(template, context)
}

