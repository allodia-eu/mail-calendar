//! The configuration an assistant is given to reach this server, as Settings offers to copy it.
//!
//! Serialised, never concatenated: a relay path carries spaces (`/Applications/Allodia Mail.app`)
//! and on Windows backslashes, and a pipe name begins with two, so a hand-built string would look
//! right and parse wrong. The key is [`SERVER_NAME`], the name the server gives itself, because
//! several MCP clients refuse a key with a space or rewrite one to `_` (`docs/mcp.md`, "How the
//! server introduces itself").

use std::collections::BTreeMap;

use serde::Serialize;

use crate::SERVER_NAME;

#[derive(Serialize)]
struct ClientConfig<'a> {
    #[serde(rename = "mcpServers")]
    mcp_servers: BTreeMap<&'a str, ServerEntry<'a>>,
}

#[derive(Serialize)]
struct ServerEntry<'a> {
    command: &'a str,
    args: Vec<&'a str>,
}

/// The MCP-client configuration that starts the relay at `relay_command` with `relay_args`,
/// pointed at `endpoint`: indented, since a person reads it before pasting it.
#[must_use]
pub fn configuration_snippet(relay_command: &str, relay_args: &[String], endpoint: &str) -> String {
    let mut args: Vec<&str> = relay_args.iter().map(String::as_str).collect();
    args.extend(["--endpoint", endpoint]);
    let config = ClientConfig {
        mcp_servers: BTreeMap::from([(
            SERVER_NAME,
            ServerEntry {
                command: relay_command,
                args,
            },
        )]),
    };
    serde_json::to_string_pretty(&config).expect("a map of strings serialises")
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::configuration_snippet;
    use crate::SERVER_NAME;

    fn entry(snippet: &str) -> Value {
        let value: Value = serde_json::from_str(snippet).expect("valid JSON");
        value["mcpServers"][SERVER_NAME].clone()
    }

    #[test]
    fn the_key_is_the_name_the_server_gives_itself_and_portable() {
        assert!(SERVER_NAME.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_".contains(&byte)
        }));
        let snippet = configuration_snippet("relay", &[], "/run/mcp.sock");
        assert_eq!(
            entry(&snippet),
            json!({"command": "relay", "args": ["--endpoint", "/run/mcp.sock"]})
        );
    }

    #[test]
    fn the_relay_s_own_arguments_come_before_the_endpoint() {
        let args = [
            "run".to_owned(),
            "--user".to_owned(),
            "--command=relay".to_owned(),
        ];
        let snippet = configuration_snippet("flatpak", &args, "/home/A Person/mcp.sock");
        assert_eq!(
            entry(&snippet)["args"],
            json!([
                "run",
                "--user",
                "--command=relay",
                "--endpoint",
                "/home/A Person/mcp.sock"
            ])
        );
    }

    #[test]
    fn a_spaced_path_and_a_pipe_name_survive_the_round_trip() {
        let command = r"C:\Program Files\Allodia Mail\allodia-mcp.exe";
        let pipe = r"\\.\pipe\mailcal-mcp";
        let snippet = configuration_snippet(command, &[], pipe);
        let entry = entry(&snippet);
        assert_eq!(entry["command"], command);
        assert_eq!(entry["args"], json!(["--endpoint", pipe]));
    }

    #[test]
    fn a_user_name_outside_ascii_stays_legible() {
        let snippet = configuration_snippet("relay", &[], "/Users/José/mcp.sock");
        assert!(snippet.contains("José"), "{snippet}");
        assert!(snippet.lines().count() > 1, "indented for a person to read");
    }
}
