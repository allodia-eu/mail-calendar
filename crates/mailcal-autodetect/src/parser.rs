//! A streaming parser for the Mozilla autoconfig XML format (Thunderbird's
//! `config-v1.1.xml` / ISPDB payload), reimplemented from the format's shape, not
//! ported from Thunderbird's code.
//!
//! Strict by design: a document that offers no TLS-capable server, an unknown
//! `socketType`, or no recognised authentication is an error, not a lenient partial
//! parse: a malformed config must fold to "nothing found", never to a plaintext or
//! half-formed account. Only `incomingServer type="imap"` and `outgoingServer
//! type="smtp"` are read (POP3 is skipped); unknown elements; including the ISPDB's
//! top-level `<oAuth2>` endpoint block, are skipped, because OAuth endpoints come from
//! the app's own trusted table, never from a fetched file.

use quick_xml::{Reader, escape::resolve_predefined_entity, events::Event, name::QName};

use crate::{
    hostname::valid_host_or_ip,
    types::{AuthKind, DetectedServer, EmailParts, SocketKind},
};

/// The servers parsed from one autoconfig document, each list in the document's
/// preference order and guaranteed non-empty.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ParsedServers {
    /// The `imap` incoming servers.
    pub incoming: Vec<DetectedServer>,
    /// The `smtp` outgoing servers.
    pub outgoing: Vec<DetectedServer>,
    /// The `<oAuth2><issuer>` the document names, as an HTTPS URL. The format writes it as a
    /// bare host (`login.example.com`), while RFC 8414 requires an issuer identifier to be an
    /// HTTPS URL, so a bare one gets the scheme here. `None` when the document names none, or
    /// names one that is not a hostname.
    pub oauth_issuer: Option<String>,
}

/// Why an autoconfig document could not be turned into usable settings. Each variant is
/// a distinct, testable failure; the orchestrator treats them all as a clean miss.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum ParseError {
    /// The bytes were not well-formed XML.
    #[error("malformed autoconfig XML: {0}")]
    Xml(String),
    /// The root element was not `clientConfig`.
    #[error("missing 'clientConfig' root element")]
    MissingClientConfig,
    /// There was no `emailProvider` element.
    #[error("missing 'emailProvider' element")]
    MissingEmailProvider,
    /// The `emailProvider` had no `id` attribute.
    #[error("missing 'emailProvider' id attribute")]
    MissingProviderId,
    /// The `emailProvider` `id` was not a valid hostname.
    #[error("invalid 'emailProvider' id: {0:?}")]
    InvalidProviderId(String),
    /// No `domain` child validated as a hostname.
    #[error("no valid 'domain' element")]
    NoValidDomain,
    /// A server block had no `hostname`.
    #[error("server missing 'hostname'")]
    MissingHostname,
    /// A server's `hostname` was not a valid host or IP.
    #[error("invalid server hostname: {0:?}")]
    InvalidHostname(String),
    /// A server's `port` was missing, non-numeric, zero, or over 65535.
    #[error("missing or invalid server 'port'")]
    InvalidPort,
    /// A server block had no `username`.
    #[error("server missing 'username'")]
    MissingUsername,
    /// A server's `socketType` was neither `SSL` nor `STARTTLS` (plaintext included).
    #[error("unsupported 'socketType': {0:?}")]
    InvalidSocketType(String),
    /// A server offered no recognised `authentication` method.
    #[error("no usable 'authentication' method")]
    NoUsableAuth,
    /// No supported (`imap`) incoming server was present.
    #[error("no supported 'incomingServer'")]
    NoIncomingServer,
    /// No supported (`smtp`) outgoing server was present.
    #[error("no supported 'outgoingServer'")]
    NoOutgoingServer,
}

/// Parses `xml` against `email` (for placeholder substitution), returning the supported
/// incoming/outgoing servers or the first validation error.
pub(crate) fn parse_autoconfig(
    xml: &[u8],
    email: &EmailParts,
) -> Result<ParsedServers, ParseError> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    // The root element must be `clientConfig`; declarations/PIs/comments before it are
    // fine, but the first *element* is decisive: a nested clientConfig doesn't count.
    loop {
        match read(&mut reader)? {
            Event::Start(e) if local_name(&e.name()) == "clientConfig" => {
                return parse_client_config(&mut reader, email);
            }
            Event::Start(_) | Event::Empty(_) | Event::Eof => {
                return Err(ParseError::MissingClientConfig);
            }
            _ => {}
        }
    }
}

/// Parses the children of `clientConfig`: requires an `emailProvider`, everything else
/// (including a stray top-level `<oAuth2>`) is skipped.
fn parse_client_config(
    reader: &mut Reader<&[u8]>,
    email: &EmailParts,
) -> Result<ParsedServers, ParseError> {
    loop {
        match read(reader)? {
            Event::Start(e) if local_name(&e.name()) == "emailProvider" => {
                let id = attribute(&e, "id").ok_or(ParseError::MissingProviderId)?;
                let id = substitute(&id, email);
                if !valid_host_or_ip(&id) {
                    return Err(ParseError::InvalidProviderId(id));
                }
                return parse_email_provider(reader, email);
            }
            Event::Eof => return Err(ParseError::MissingEmailProvider),
            _ => {}
        }
    }
}

/// Parses an `emailProvider`: at least one valid `domain`, then the supported servers.
fn parse_email_provider(
    reader: &mut Reader<&[u8]>,
    email: &EmailParts,
) -> Result<ParsedServers, ParseError> {
    let mut has_valid_domain = false;
    let mut incoming = Vec::new();
    let mut outgoing = Vec::new();
    let mut oauth_issuer = None;

    loop {
        match read(reader)? {
            Event::Start(e) => {
                let name = local_name(&e.name());
                match name.as_str() {
                    "domain" => {
                        let value = substitute(read_text(reader, &name)?.trim(), email);
                        has_valid_domain |= valid_host_or_ip(&value);
                    }
                    "incomingServer" => {
                        if let Some(server) = parse_server(reader, &e, email, "imap", &name)? {
                            incoming.push(server);
                        }
                    }
                    "outgoingServer" => {
                        if let Some(server) = parse_server(reader, &e, email, "smtp", &name)? {
                            outgoing.push(server);
                        }
                    }
                    "oAuth2" => oauth_issuer = parse_oauth_issuer(reader)?,
                    _ => skip(reader, &name)?,
                }
            }
            Event::End(e) if local_name(&e.name()) == "emailProvider" => break,
            Event::Eof => break,
            _ => {}
        }
    }

    if !has_valid_domain {
        return Err(ParseError::NoValidDomain);
    }
    if incoming.is_empty() {
        return Err(ParseError::NoIncomingServer);
    }
    if outgoing.is_empty() {
        return Err(ParseError::NoOutgoingServer);
    }
    Ok(ParsedServers {
        incoming,
        outgoing,
        oauth_issuer,
    })
}

/// Reads an `<oAuth2>` block for its `<issuer>`, skipping the endpoint and client-id
/// elements beside it.
///
/// The issuer is written as a bare hostname in this format and must be an HTTPS URL to be
/// an RFC 8414 issuer identifier, so a bare one gets the scheme. Anything that is not a
/// hostname is dropped rather than passed on: an issuer decides which server a person types
/// their password into, and half-understanding one is worse than not offering sign-in.
fn parse_oauth_issuer(reader: &mut Reader<&[u8]>) -> Result<Option<String>, ParseError> {
    let mut issuer = None;
    loop {
        match read(reader)? {
            Event::Start(e) => {
                let name = local_name(&e.name());
                if name == "issuer" {
                    let value = read_text(reader, &name)?.trim().to_owned();
                    issuer = normalize_issuer(&value);
                } else {
                    skip(reader, &name)?;
                }
            }
            Event::End(e) if local_name(&e.name()) == "oAuth2" => break,
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(issuer)
}

/// An `<issuer>` value as an HTTPS URL, or `None` when it is not a hostname.
///
/// An `https://` prefix already present is kept (some documents write the full URL);
/// `http://` is refused outright rather than upgraded, because a document that names a
/// plaintext issuer is describing something we will not talk to either way.
fn normalize_issuer(value: &str) -> Option<String> {
    let host = value.strip_prefix("https://").unwrap_or(value);
    if host.contains("://") {
        return None;
    }
    let host = host.trim_end_matches('/');
    valid_host_or_ip(host).then(|| format!("https://{host}"))
}

/// Parses one `incomingServer`/`outgoingServer`. Returns `None` (consuming the element)
/// when its `type` is not `wanted_type`: the POP3 skip. `tag` is the element's local
/// name, used to find its closing tag.
fn parse_server(
    reader: &mut Reader<&[u8]>,
    start: &quick_xml::events::BytesStart<'_>,
    email: &EmailParts,
    wanted_type: &str,
    tag: &str,
) -> Result<Option<DetectedServer>, ParseError> {
    if attribute(start, "type").as_deref() != Some(wanted_type) {
        skip(reader, tag)?;
        return Ok(None);
    }

    let mut hostname = None;
    let mut port_text = None;
    let mut socket = None;
    let mut username = None;
    let mut auth = Vec::new();

    loop {
        match read(reader)? {
            Event::Start(e) => {
                let child = local_name(&e.name());
                match child.as_str() {
                    "hostname" => {
                        hostname = Some(substitute(read_text(reader, &child)?.trim(), email));
                    }
                    "port" => port_text = Some(read_text(reader, &child)?.trim().to_owned()),
                    "socketType" => socket = Some(read_text(reader, &child)?.trim().to_owned()),
                    "username" => {
                        username = Some(substitute(read_text(reader, &child)?.trim(), email));
                    }
                    "authentication" => {
                        if let Some(kind) = parse_auth(read_text(reader, &child)?.trim()) {
                            auth.push(kind);
                        }
                    }
                    _ => skip(reader, &child)?,
                }
            }
            Event::End(e) if local_name(&e.name()) == tag => break,
            Event::Eof => break,
            _ => {}
        }
    }

    let hostname = hostname
        .filter(|h| !h.is_empty())
        .ok_or(ParseError::MissingHostname)?;
    if !valid_host_or_ip(&hostname) {
        return Err(ParseError::InvalidHostname(hostname));
    }
    let port = parse_port(port_text.as_deref())?;
    let socket = parse_socket(socket.as_deref())?;
    let username = username
        .filter(|u| !u.is_empty())
        .ok_or(ParseError::MissingUsername)?;
    if auth.is_empty() {
        return Err(ParseError::NoUsableAuth);
    }

    Ok(Some(DetectedServer {
        hostname,
        port,
        socket,
        auth,
        username,
    }))
}

/// Parses a port string to a `1..=65535` value.
fn parse_port(text: Option<&str>) -> Result<u16, ParseError> {
    match text.and_then(|t| t.parse::<u16>().ok()) {
        Some(port) if port != 0 => Ok(port),
        _ => Err(ParseError::InvalidPort),
    }
}

/// Maps a `socketType` string to a [`SocketKind`]; anything else (including `plain`) is
/// a hard error, autodetect never yields a plaintext config.
fn parse_socket(text: Option<&str>) -> Result<SocketKind, ParseError> {
    match text {
        Some("SSL") => Ok(SocketKind::Tls),
        Some("STARTTLS") => Ok(SocketKind::StartTls),
        other => Err(ParseError::InvalidSocketType(
            other.unwrap_or("").to_owned(),
        )),
    }
}

/// Maps an `authentication` string to an [`AuthKind`], or `None` for an unrecognized
/// value (skipped, as long as at least one recognised value remains).
fn parse_auth(text: &str) -> Option<AuthKind> {
    match text {
        "OAuth2" => Some(AuthKind::OAuth2),
        "password-cleartext" => Some(AuthKind::PasswordCleartext),
        "password-encrypted" => Some(AuthKind::PasswordEncrypted),
        _ => None,
    }
}

/// Substitutes the autoconfig placeholders in `value`.
fn substitute(value: &str, email: &EmailParts) -> String {
    value
        .replace("%EMAILADDRESS%", &email.full)
        .replace("%EMAILLOCALPART%", &email.local)
        .replace("%EMAILDOMAIN%", email.domain.as_str())
}

/// Reads the text content of the element named `name` up to its closing tag,
/// concatenating text nodes and ignoring any nested markup.
fn read_text(reader: &mut Reader<&[u8]>, name: &str) -> Result<String, ParseError> {
    let mut text = String::new();
    loop {
        match read(reader)? {
            // A `Text` run is already the literal characters: the reader hands every
            // `&…;` back separately as `GeneralRef`, so a run can never hold one.
            Event::Text(chunk) => text.push_str(&chunk),
            Event::GeneralRef(reference) => {
                let resolved = reference
                    .resolve_char_ref()
                    .map_err(|e| ParseError::Xml(e.to_string()))?;
                match resolved {
                    Some(character) => text.push(character),
                    // The document's DTD is never read, so only the five predefined
                    // entities resolve and whatever a `<!ENTITY>` declared is undeclared
                    // to us: refused, never expanded, so a "billion laughs" cannot
                    // balloon here.
                    None => match resolve_predefined_entity(&reference) {
                        Some(expansion) => text.push_str(expansion),
                        None => {
                            return Err(ParseError::Xml(format!(
                                "undeclared entity `&{};`",
                                &*reference
                            )));
                        }
                    },
                }
            }
            Event::Start(e) => skip(reader, &local_name(&e.name()))?,
            Event::End(e) if local_name(&e.name()) == name => break,
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(text)
}

/// Skips to the closing tag of the currently-open element named `name`.
fn skip(reader: &mut Reader<&[u8]>, name: &str) -> Result<(), ParseError> {
    reader
        .read_to_end(QName(name))
        .map(|_| ())
        .map_err(|e| ParseError::Xml(e.to_string()))
}

/// Reads one event, mapping a quick-xml error into [`ParseError::Xml`].
fn read<'a>(reader: &mut Reader<&'a [u8]>) -> Result<Event<'a>, ParseError> {
    reader
        .read_event()
        .map_err(|e| ParseError::Xml(e.to_string()))
}

/// The local name (after any `prefix:`) of a qualified element name.
fn local_name(name: &QName<'_>) -> String {
    name.local_name().as_ref().to_owned()
}

/// The value of `start`'s attribute named `key`, if present.
fn attribute(start: &quick_xml::events::BytesStart<'_>, key: &str) -> Option<String> {
    start
        .attributes()
        .flatten()
        .find_map(|attr| (attr.key.as_ref() == key).then(|| attr.value.into_owned()))
}

#[cfg(test)]
#[path = "parser_tests.rs"]
mod parser_tests;
