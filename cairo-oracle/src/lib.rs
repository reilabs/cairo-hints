use cairo_proto_serde::configuration::{Configuration, MethodDeclaration};
use cairo_proto_serde::{deserialize_cairo_serde, serialize_cairo_serde};
use indoc::formatdoc;
use itertools::Itertools;
use reqwest::Url;
use serde_json::Value;
use starknet_types_core::felt::Felt as Felt252;

pub struct CairoOracle {
    server: String,
    configuration: Configuration,
}

impl CairoOracle {
    pub fn new(server: String, configuration: Configuration) -> Self {
        Self { server, configuration }
    }

    pub fn new_from_env() -> Self {
        Self { server: "".to_string(), configuration: Default::default() }
    }

    fn method_declaration(&self, selector: &str) -> Option<&MethodDeclaration> {
        self
            .configuration
            .services
            .iter()
            .find_map(|(_, methods)| methods.methods.get(selector))
    }

    pub fn execute_hint(&self, selector: &str, mut data: &[Felt252]) -> Result<Vec<Felt252>, Box<str>> {
        let Some(configuration) = self.method_declaration(selector) else {
            return Err(Box::from(format!(
                "Unknown cheatcode selector: {selector}"
            )));
        };

        let mut server_url = Url::parse(&self.server).expect("oracle-server must be a valid URL");
        server_url.path_segments_mut().expect("cannot be a base URL").push(selector);

        let data = deserialize_cairo_serde(
            &self.configuration,
            &configuration.input,
            &mut data,
        );
        println!("let the oracle decide... Inputs: {data:?}");

        let client = reqwest::blocking::Client::new();

        let req = client.post(server_url.clone()).json(&data).send().expect(
            format!("Couldn't connect to oracle server {server_url}. Is the server running?")
                .as_str(),
        );

        let status_code = req.error_for_status_ref().map(|_| ());
        let body = req.text().expect(
            formatdoc! {
                r#"
                Response from oracle server can't be parsed as string."#
            }
            .as_str(),
        );

        status_code.expect(
            formatdoc! {
                r#"
                Received {body:?}.
                Response status from oracle server not successful."#
            }
            .as_str(),
        );

        let body = serde_json::from_str::<Value>(body.as_str()).expect(
            formatdoc! {
                r#"
                Received {body:?}.
                Error converting response from oracle server {server_url} to JSON."#
            }
            .as_str(),
        );

        let body = body.as_object().expect(
            formatdoc! {r#"
                Received {body:?}.
                Error serialising response as object from oracle server.
            "#}
            .as_str(),
        );

        body.keys()
            .exactly_one()
            .map_err(|_| {
                formatdoc! {r#"
                    Received {body:?}.
                    Expected response format from oracle server is {{"result": <response_object>}}.
                "#}
            })
            .unwrap();

        let output = body.get("result").expect(
            formatdoc! {r#"
                Received {body:?}.
                Expected response format from oracle server is {{"result": <response_object>}}.
            "#}
            .as_str(),
        );

        Ok(serialize_cairo_serde(&self.configuration, &configuration.output, output))
    }
}
