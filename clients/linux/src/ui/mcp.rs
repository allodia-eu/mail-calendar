//! Linux endpoint, relay configuration and host-composer bridge for the local MCP server.

use std::path::{Path, PathBuf};

use mailcal_bindings::{AgentDraft, AgentHostUi, MailcalApp};
use serde::Serialize;

use super::AppInput;

pub(super) const CONFIGURATION_KEY: &str = "allodia-mail-and-calendar";
const RELAY_FILE_NAME: &str = "allodia-mcp";
const FLATPAK_INFO_PATH: &str = "/.flatpak-info";

/// Installs the host composer and endpoint together, so a listening server can always open drafts.
pub(super) fn install(app: Option<&MailcalApp>, sender: relm4::Sender<AppInput>) {
    if let Some(app) = app {
        app.set_agent_host_ui(Box::new(AgentComposerBridge::new(sender)));
        app.set_mcp_endpoint(Some(endpoint()));
    }
}

/// Where the app and relay meet. In Flatpak, GLib resolves this inside the app's persistent data
/// directory; the host-side relay enters the same sandbox before connecting.
pub(super) fn endpoint() -> String {
    gtk::glib::user_data_dir()
        .join("mailcal/mcp.sock")
        .to_string_lossy()
        .into_owned()
}

#[derive(Debug, PartialEq, Eq)]
struct RelayInvocation {
    command: String,
    args: Vec<String>,
}

fn relay_invocation() -> RelayInvocation {
    let executable = std::env::current_exe().ok();
    if let Some(invocation) = FlatpakContext::load(Path::new(FLATPAK_INFO_PATH))
        .and_then(|context| context.relay_invocation(executable.as_deref(), &gtk::glib::home_dir()))
    {
        return invocation;
    }
    if let Some(app_id) = std::env::var_os("FLATPAK_ID") {
        return unqualified_flatpak_invocation(&app_id.to_string_lossy());
    }
    let beside = executable.and_then(|executable| {
        executable
            .parent()
            .map(|parent| parent.join(RELAY_FILE_NAME))
    });
    unpackaged_invocation(beside.as_deref())
}

fn unqualified_flatpak_invocation(app_id: &str) -> RelayInvocation {
    RelayInvocation {
        command: "flatpak".to_owned(),
        args: vec![
            "run".to_owned(),
            format!("--command={RELAY_FILE_NAME}"),
            app_id.to_owned(),
        ],
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FlatpakKind {
    Application,
    DevelopmentRuntime,
}

#[derive(Debug, PartialEq, Eq)]
struct FlatpakContext {
    reference: String,
    deployment_path: PathBuf,
    kind: FlatpakKind,
}

impl FlatpakContext {
    fn load(path: &Path) -> Option<Self> {
        let file = gtk::glib::KeyFile::new();
        file.load_from_file(path, gtk::glib::KeyFileFlags::NONE)
            .ok()?;
        Self::from_key_file(&file)
    }

    fn from_key_file(file: &gtk::glib::KeyFile) -> Option<Self> {
        let (name, deployment_path, kind) = file
            .string("Application", "name")
            .ok()
            .and_then(|name| {
                file.string("Instance", "app-path")
                    .ok()
                    .map(|path| (name, path, FlatpakKind::Application))
            })
            .or_else(|| {
                file.string("Runtime", "name").ok().and_then(|name| {
                    file.string("Instance", "runtime-path")
                        .ok()
                        .map(|path| (name, path, FlatpakKind::DevelopmentRuntime))
                })
            })?;
        let branch = file.string("Instance", "branch").ok()?;
        let arch = file.string("Instance", "arch").ok()?;
        Some(Self {
            reference: format!("{name}/{arch}/{branch}"),
            deployment_path: PathBuf::from(deployment_path.as_str()),
            kind,
        })
    }

    fn relay_invocation(&self, executable: Option<&Path>, home: &Path) -> Option<RelayInvocation> {
        let installation = if self.deployment_path.starts_with(home) {
            "--user"
        } else {
            "--system"
        };
        let mut args = vec!["run".to_owned(), installation.to_owned()];
        match self.kind {
            FlatpakKind::Application => {
                args.push(format!("--command={RELAY_FILE_NAME}"));
            }
            FlatpakKind::DevelopmentRuntime => {
                let relay = executable?.parent()?.join(RELAY_FILE_NAME);
                args.extend([
                    "--devel".to_owned(),
                    "--filesystem=host".to_owned(),
                    "--filesystem=/tmp".to_owned(),
                    "--no-a11y-bus".to_owned(),
                    format!("--command={}", relay.to_string_lossy()),
                ]);
            }
        }
        args.push(self.reference.clone());
        Some(RelayInvocation {
            command: "flatpak".to_owned(),
            args,
        })
    }
}

fn unpackaged_invocation(beside: Option<&Path>) -> RelayInvocation {
    let command = beside.filter(|path| path.is_file()).map_or_else(
        || RELAY_FILE_NAME.to_owned(),
        |path| path.to_string_lossy().into_owned(),
    );
    RelayInvocation {
        command,
        args: Vec::new(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientConfig {
    mcp_servers: std::collections::BTreeMap<&'static str, ServerEntry>,
}

#[derive(Serialize)]
struct ServerEntry {
    command: String,
    args: Vec<String>,
}

/// The MCP-client configuration the Settings panel offers to copy.
pub(super) fn configuration_snippet(endpoint: &str) -> String {
    configuration_snippet_for(endpoint, relay_invocation())
}

fn configuration_snippet_for(endpoint: &str, mut relay: RelayInvocation) -> String {
    relay
        .args
        .extend(["--endpoint".to_owned(), endpoint.to_owned()]);
    let config = ClientConfig {
        mcp_servers: [(
            CONFIGURATION_KEY,
            ServerEntry {
                command: relay.command,
                args: relay.args,
            },
        )]
        .into_iter()
        .collect(),
    };
    serde_json::to_string_pretty(&config).unwrap_or_default()
}

/// Non-blocking hop from the server's connection task onto Relm4's UI input channel.
#[derive(Clone, Debug)]
pub(super) struct AgentComposerBridge {
    sender: relm4::Sender<AppInput>,
}

impl AgentComposerBridge {
    pub(super) fn new(sender: relm4::Sender<AppInput>) -> Self {
        Self { sender }
    }
}

impl AgentHostUi for AgentComposerBridge {
    fn open_composer(&self, draft: AgentDraft) {
        self.sender.emit(AppInput::OpenAgentDraft(Box::new(draft)));
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use gtk::glib::{KeyFile, KeyFileFlags};
    use serde_json::Value;

    use super::{
        CONFIGURATION_KEY, FlatpakContext, RELAY_FILE_NAME, configuration_snippet_for,
        unpackaged_invocation, unqualified_flatpak_invocation,
    };

    fn flatpak_invocation(metadata: &str, executable: &str, home: &str) -> super::RelayInvocation {
        let file = KeyFile::new();
        file.load_from_data(metadata, KeyFileFlags::NONE)
            .expect("Flatpak metadata");
        FlatpakContext::from_key_file(&file)
            .expect("Flatpak context")
            .relay_invocation(Some(Path::new(executable)), Path::new(home))
            .expect("relay invocation")
    }

    #[test]
    fn configuration_key_is_protocol_name_and_portable() {
        assert_eq!(CONFIGURATION_KEY, "allodia-mail-and-calendar");
        assert!(CONFIGURATION_KEY.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_".contains(&byte)
        }));
    }

    #[test]
    fn flatpak_snippet_enters_the_installed_app_before_running_the_relay() {
        let snippet = configuration_snippet_for(
            "/home/A Person/.var/app/eu.allodia.mailcal/data/mailcal/mcp.sock",
            flatpak_invocation(
                "[Application]\nname=eu.allodia.mailcal\n\
                 [Instance]\napp-path=/home/A Person/.local/share/flatpak/app/eu.allodia.mailcal/files\nbranch=stable\narch=x86_64\n",
                "/app/bin/mailcal",
                "/home/A Person",
            ),
        );
        let value: Value = serde_json::from_str(&snippet).expect("valid JSON");
        let entry = &value["mcpServers"][CONFIGURATION_KEY];
        assert_eq!(entry["command"], "flatpak");
        assert_eq!(
            entry["args"],
            serde_json::json!([
                "run",
                "--user",
                "--command=allodia-mcp",
                "eu.allodia.mailcal/x86_64/stable",
                "--endpoint",
                "/home/A Person/.var/app/eu.allodia.mailcal/data/mailcal/mcp.sock"
            ])
        );
    }

    #[test]
    fn sdk_snippet_does_not_leave_the_runtime_installation_ambiguous() {
        let snippet = configuration_snippet_for(
            "/tmp/mailcal/mcp.sock",
            flatpak_invocation(
                "[Runtime]\nname=org.gnome.Sdk\n\
                 [Instance]\nruntime-path=/home/Developer/.local/share/flatpak/runtime/org.gnome.Sdk/files\nbranch=50\narch=x86_64\n",
                "/home/Developer/project/target/flatpak-sdk/debug/mailcal-linux",
                "/home/Developer",
            ),
        );
        let value: Value = serde_json::from_str(&snippet).expect("valid JSON");
        let entry = &value["mcpServers"][CONFIGURATION_KEY];
        assert_eq!(
            entry["args"],
            serde_json::json!([
                "run",
                "--user",
                "--devel",
                "--filesystem=host",
                "--filesystem=/tmp",
                "--no-a11y-bus",
                "--command=/home/Developer/project/target/flatpak-sdk/debug/allodia-mcp",
                "org.gnome.Sdk/x86_64/50",
                "--endpoint",
                "/tmp/mailcal/mcp.sock"
            ])
        );
    }

    #[test]
    fn system_flatpak_snippet_selects_the_system_installation() {
        let invocation = flatpak_invocation(
            "[Application]\nname=eu.allodia.mailcal\n\
             [Instance]\napp-path=/var/lib/flatpak/app/eu.allodia.mailcal/files\nbranch=stable\narch=x86_64\n",
            "/app/bin/mailcal",
            "/home/Developer",
        );
        assert_eq!(invocation.args[1], "--system");
    }

    #[test]
    fn metadata_fallback_keeps_older_flatpak_environments_usable() {
        assert_eq!(
            unqualified_flatpak_invocation("eu.allodia.mailcal").args,
            ["run", "--command=allodia-mcp", "eu.allodia.mailcal"]
        );
    }

    #[test]
    fn unpackaged_relay_uses_the_binary_beside_the_app_when_present() {
        let directory =
            std::env::temp_dir().join(format!("mailcal-mcp-endpoint-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("create temp directory");
        let relay = directory.join(RELAY_FILE_NAME);
        fs::write(&relay, []).expect("create relay marker");
        assert_eq!(
            unpackaged_invocation(Some(&relay)).command,
            relay.to_string_lossy()
        );
        fs::remove_dir_all(directory).expect("remove temp directory");
    }

    #[test]
    fn missing_unpackaged_relay_falls_back_to_path() {
        assert_eq!(
            unpackaged_invocation(Some(Path::new("/not/present/allodia-mcp"))).command,
            RELAY_FILE_NAME
        );
    }
}
