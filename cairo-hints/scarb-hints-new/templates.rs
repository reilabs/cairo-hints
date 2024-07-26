use handlebars::Handlebars;

pub fn get_template_engine() -> handlebars::Handlebars<'static> {
    let mut registry = Handlebars::new();
    registry.register_template_string("dockerfile", include_str!("templates/python/Dockerfile.hbs").to_owned()).unwrap();
    registry.register_template_string("requirements", include_str!("templates/python/requirements.hbs").to_owned()).unwrap();
    registry.register_template_string("main", include_str!("templates/python/main.hbs").to_owned()).unwrap();
    registry.register_template_string("pre-commit", include_str!("templates/python/.pre-commit-config.hbs").to_owned()).unwrap();
    registry.register_template_string("gitignore", include_str!("templates/python/.gitignore.hbs").to_owned()).unwrap();
    registry
}
