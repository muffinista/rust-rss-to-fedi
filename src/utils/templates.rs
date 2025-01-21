
pub fn render(
  template: &str,
  tmpl: &tera::Tera,
  context: &tera::Context,
) -> Result<String, tera::Error> {
 // let current = template.replace(".j2", "");
  // if !context.contains_key("current") {
  //     context.insert("current", &current);
  // }
  // context.insert("routes", &self.routes);
  // if let Some(username) = crate::utils::get_session_username(session) {
  //     context.insert("username", &username);
  // }

  //Template::render(name, context).finalize(&ctxt).ok().map(|v| v.1)
  tmpl.render(template, context)
}


// use tokio::sync::OnceCell;

// static TERA: OnceCell<Tera> = OnceCell::const_new();


// pub fn render(name: &str, context: &Context) -> rocket_dyn_templates::tera::Result<String> {
//   init_templating();

//   let tera: &Tera = TERA.get().unwrap();
//   tera.render(name, context)
// }


// ///
// /// lighly modified from the rocket code for initializing tera
// ///
// fn init_tera() -> Tera {
//   let root = Path::new("templates/");
//   let mut templates: Vec<(String, String)> = Vec::<(String, String)>::new();
//   let glob_path = root.join("**").join("*.tera");
//   let glob_path = glob_path.to_str().expect("valid glob path string");

//   for path in glob(glob_path).unwrap().filter_map(std::result::Result::ok) {
//     let name = split_path(root, &path);
//     templates.push((name, path.into_os_string().into_string().unwrap()));
//   }

//   let files = templates.into_iter().map(|(name, path)| (path, Some(name)));

//   let mut tera = Tera::default();
//   let ext = [".html.tera", ".htm.tera", ".xml.tera", ".html", ".htm", ".xml"];

//   tera.add_template_files(files).unwrap();
//   tera.autoescape_on(ext.to_vec());

//   tera
// }

// /// Removes the file path's extension or does nothing if there is none.
// fn remove_extension(path: &Path) -> PathBuf {
//   let stem = match path.file_stem() {
//     Some(stem) => stem,
//     None => return path.to_path_buf()
//   };

//   match path.parent() {
//     Some(parent) => parent.join(stem),
//     None => PathBuf::from(stem)
//   }
// }

// /// Splits a path into a name that may be used to identify the template, and the
// /// template's data type, if any.
// fn split_path(root: &Path, path: &Path) -> String {
//   // println!("path: {path:?} root: {root:?}");
  
//   let rel_path = path.strip_prefix(root).unwrap().to_path_buf();
//   let path_no_ext = remove_extension(&rel_path);
//   let mut name = remove_extension(&path_no_ext).to_string_lossy().into_owned();
  
//   // Ensure template name consistency on Windows systems
//   if cfg!(windows) {
//     name = name.replace('\\', "/");
//   }

//   name
// }
