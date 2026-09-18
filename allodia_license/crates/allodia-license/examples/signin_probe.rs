//! Drive a real sign-in against a real account service, from a terminal.
//!
//! Not a test: it needs a running service, a browser and a person. It exists because the failures
//! that matter here (a scope the service does not know, a redirect it will not accept, a loopback
//! port it matches exactly) are all things no unit test can see, and all things that would
//! otherwise be found on a device, in a client, weeks later.
//!
//!     cargo run -p allodia-license --example signin_probe
//!
//! It binds a loopback listener, prints the authorization URL to open, waits for the redirect and
//! exchanges the code. Then it prints which scopes the grant actually carries, and reads both the
//! entitlement and the account screen with the token it got.

use std::{
    io::{BufRead, BufReader, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener},
};

use allodia_license::{AccountService, Method, Request, Response, SignIn, Transport};
use time::OffsetDateTime;

/// The host's half of the port, over `reqwest`: what a client would supply.
struct Reqwest(reqwest::blocking::Client);

impl Transport for Reqwest {
    fn send(&self, request: &Request) -> Result<Response, String> {
        let method = match request.method {
            Method::Get => reqwest::Method::GET,
            Method::Post => reqwest::Method::POST,
            Method::Put => reqwest::Method::PUT,
            Method::Delete => reqwest::Method::DELETE,
        };
        let mut builder = self
            .0
            .request(method, &request.url)
            .bearer_auth(&request.bearer);
        if let Some(body) = &request.body {
            builder = builder
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.clone());
        }
        if let Some(key) = &request.idempotency_key {
            builder = builder.header("Idempotency-Key", key.clone());
        }
        let response = builder.send().map_err(|error| error.to_string())?;
        let status = response.status().as_u16();
        let body = response.text().map_err(|error| error.to_string())?;
        Ok(Response { status, body })
    }
}

/// Waits for one redirect and returns the full callback URL.
fn await_redirect(listener: &TcpListener, redirect_uri: &str) -> Result<String, String> {
    let (stream, _) = listener.accept().map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|error| error.to_string())?;
    let target = request_line
        .split_whitespace()
        .nth(1)
        .ok_or("no request target")?;
    let mut stream = stream;
    let _ = stream.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nSigned in. You can close this tab.\r\n",
    );
    Ok(format!("{}{}", redirect_uri.trim_end_matches('/'), target))
}

/// Print a JWT's header and payload, without verifying anything.
///
/// **Diagnosis only, and deliberately not a JWT library.** Nothing here checks a signature and
/// nothing decides anything from what it reads; verifying is the API's job and it has the keys.
/// An opaque token (one minted without a `resource`) is not three segments and says so, which is
/// itself the answer: the API refuses anything not minted for it.
fn print_claims(token: &str) {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

    let segments: Vec<&str> = token.split('.').collect();
    if segments.len() != 3 {
        println!(
            "\ntoken is not a JWT ({} segments): opaque, so it carries no audience and the \
                  API will refuse it",
            segments.len()
        );
        return;
    }
    for (label, segment) in [("header", segments[0]), ("claims", segments[1])] {
        match URL_SAFE_NO_PAD
            .decode(segment)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
        {
            Some(json) => println!("\ntoken {label}: {json}"),
            None => println!("\ntoken {label}: could not be decoded"),
        }
    }
}

// Not `#[tokio::main]`: that wraps the whole body in an async context, and the blocking HTTP
// client the Transport below uses cannot be dropped inside one. The async half is scoped to
// `block_on` so the blocking half runs outside it.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if !allodia_license::available() {
        return Err("this build carries no MAILCAL_ALLODIA_CLIENT_ID".into());
    }
    println!("service:  {}", allodia_license::host());
    println!("scopes:   {:?}", allodia_license::SCOPES);
    println!("api:      {}", allodia_license::api_url());

    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))?;
    let redirect_uri = format!("http://127.0.0.1:{}/", listener.local_addr()?.port());
    println!("redirect: {redirect_uri}");

    let runtime = tokio::runtime::Runtime::new()?;
    let (tokens, endpoints) = runtime.block_on(async {
        let signin = SignIn::discover(&redirect_uri).await?;
        let endpoints = signin.endpoints();
        // Both hops of the discovery chain, printed because a mismatch between them is invisible
        // in a `401`: the audience the token is minted for has to be the resource the API names
        // for itself, and the userinfo endpoint belongs to the authorization server rather than
        // to the API, which are two different hosts here.
        println!("\nresource: {:?}", endpoints.resource);
        println!("token:    {}", endpoints.token_endpoint);
        println!("userinfo: {:?}", endpoints.userinfo_endpoint);
        let start = signin.begin(allodia_license::Prompt::SignIn);
        println!(
            "\nOpen this, sign in, and come back:\n\n{}\n",
            start.authorization_url
        );
        let callback = await_redirect(&listener, &redirect_uri)
            .map_err(|error| -> Box<dyn std::error::Error> { error.into() })?;
        let tokens = signin
            .complete(
                &callback,
                &start.state,
                start.pkce.verifier(),
                OffsetDateTime::now_utc(),
            )
            .await?;
        Ok::<_, Box<dyn std::error::Error>>((tokens, endpoints))
    })?;
    println!(
        "access token:  {} chars",
        tokens.access_token.expose().len()
    );
    println!(
        "refresh token: {}",
        if tokens.refresh_token.is_some() {
            "issued"
        } else {
            "NONE — offline_access did not take effect"
        }
    );

    // What the token actually says about itself. A `401` names neither the audience nor the
    // scope, so without this the next step is guesswork; with it, an `aud` that is not the
    // resource above, or an `iss` that is not the authorization server, is one line.
    print_claims(tokens.access_token.expose());

    // Where a `401` on the entitlement call actually comes from. If userinfo accepts this token
    // then the token is fine and our endpoint does not accept this *kind* of token, which is a
    // different repair in a different repository.
    //
    // ⚠️ Userinfo is the **authorization server's**, and that is a different host from the API:
    // asking the API's host for it answers about a route that was never there, which is a `500`
    // that reads as a broken token and is nothing of the sort.
    let http = reqwest::blocking::Client::new();
    let issuer = allodia_license::host();
    let mut probes = vec![("entitlement", format!("{issuer}/api/v1/entitlement"))];
    if let Some(userinfo) = endpoints.userinfo_endpoint.clone() {
        probes.insert(0, ("userinfo", userinfo));
    }
    for (label, url) in probes {
        let response = http
            .get(&url)
            .bearer_auth(tokens.access_token.expose())
            .send()?;
        let status = response.status();
        let body = response.text().unwrap_or_default();
        println!(
            "\n{label}: {status}\n  {}",
            body.chars().take(300).collect::<String>()
        );
    }

    // What the grant actually carries, which is not what was asked for. A scope the service does
    // not advertise is never requested, so this is where a missing `mailcal:subscription:*` shows
    // up: without it the account screen reports "sign in again" and no amount of signing in again
    // changes the answer, because the service has nothing to issue.
    println!("\ngranted scopes: {}", tokens.scope);
    for scope in allodia_license::SCOPES {
        if !tokens.scope.split_whitespace().any(|got| got == *scope) {
            println!("  MISSING {scope}");
        }
    }

    let service = AccountService::new(&issuer);
    let transport = Reqwest(http);
    match service.entitlement(&transport, tokens.access_token.expose()) {
        Ok(answer) => println!("\nparsed entitlement: {answer:#?}"),
        Err(error) => println!("\nparsed entitlement failed: {error}"),
    }

    // The account screen, which needs no purchase and no billing to answer: the first thing that
    // can be driven end to end against the real service. `actions` is what a screen would draw,
    // and `checkoutAvailable` says whether this deployment has billing at all.
    match service.subscription(&transport, tokens.access_token.expose()) {
        Ok(subscription) => println!("\nparsed subscription: {subscription:#?}"),
        Err(error) => println!("\nparsed subscription failed: {error}"),
    }
    Ok(())
}
