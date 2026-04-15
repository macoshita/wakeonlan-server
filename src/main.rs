use axum::{
    extract::{Form, Query},
    http::header::{COOKIE, SET_COOKIE},
    http::HeaderMap,
    response::{Html, IntoResponse, Response, Redirect},
    routing::get,
    Router,
};
use askama::Template;
use clap::{Parser, Subcommand};
use serde::Deserialize;
use std::net::SocketAddr;
use std::process::Command;

use std::path::Path;
use wake_on_lan::MagicPacket;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(long, default_value_t = 3000)]
    port: u16,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Service {
        #[command(subcommand)]
        opts: ServiceOpts,
    },
}

#[derive(Subcommand)]
enum ServiceOpts {
    Install,
    Uninstall,
}

#[derive(Template)]
#[template(path = "wol.html")]
struct WolTemplate {
    message: Option<String>,
    addr: String,
}

#[derive(Deserialize)]
struct WolForm {
    addr: String,
}

#[derive(Deserialize)]
struct WolQuery {
    addr: Option<String>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Service { opts }) => match opts {
            ServiceOpts::Install => install_service(cli.port),
            ServiceOpts::Uninstall => uninstall_service(),
        },
        None => run_server(cli.port).await,
    }
}

async fn run_server(port: u16) {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new()
        .route("/wol", get(wol_get).post(wol_post))
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn install_service(port: u16) {
    let current_exe = std::env::current_exe().expect("Failed to get current executable path");
    let exe_path = current_exe.to_str().expect("Path is not valid UTF-8");
    let work_dir = current_exe.parent().expect("Failed to get parent directory").to_str().expect("Path is not valid UTF-8");
    
    let service_content = format!(
        "[Unit]
Description=Wake-on-LAN Server
After=network.target

[Service]
Type=simple
User=root
ExecStart={} --port {}
Restart=always
WorkingDirectory={}

[Install]
WantedBy=multi-user.target
",
        exe_path, port, work_dir
    );

    let service_path = "/etc/systemd/system/wakeonlan-server.service";
    
    // Check for root privileges by attempting to write to /etc/systemd/system
    if let Err(e) = std::fs::write(service_path, service_content) {
        eprintln!("Failed to write service file: {}. Are you running as root?", e);
        std::process::exit(1);
    }

    println!("Service file created at {}", service_path);

    run_command("systemctl", &["daemon-reload"]);
    run_command("systemctl", &["enable", "--now", "wakeonlan-server"]);
    
    println!("Service installed and started successfully.");
}

fn uninstall_service() {
    let service_path = "/etc/systemd/system/wakeonlan-server.service";
    if !Path::new(service_path).exists() {
        println!("Service file does not exist. Nothing to uninstall.");
        return;
    }

    run_command("systemctl", &["disable", "--now", "wakeonlan-server"]);
    
    if let Err(e) = std::fs::remove_file(service_path) {
         eprintln!("Failed to remove service file: {}. Are you running as root?", e);
         std::process::exit(1);
    }
    
    println!("Service file removed.");

    run_command("systemctl", &["daemon-reload"]);

    println!("Service uninstalled successfully.");
}

fn run_command(cmd: &str, args: &[&str]) {
    let status = Command::new(cmd)
        .args(args)
        .status()
        .unwrap_or_else(|_| panic!("Failed to execute {}", cmd));

    if !status.success() {
        eprintln!("Command '{} {:?}' failed with status: {}", cmd, args, status);
        std::process::exit(1);
    }
}

async fn wol_get(headers: HeaderMap, Query(query): Query<WolQuery>) -> impl IntoResponse {
    let mut message = None;

    // Extract message from Cookie if present
    if let Some(cookie_header) = headers.get(COOKIE) {
        if let Ok(cookie_str) = cookie_header.to_str() {
            for cookie in cookie_str.split(';') {
                let cookie = cookie.trim();
                if let Some(msg) = cookie.strip_prefix("flash_message=") {
                    message = Some(urlencoding::decode(msg).unwrap_or_default().into_owned());
                    break;
                }
            }
        }
    }

    let has_message = message.is_some();
    let mut response = HtmlTemplate(WolTemplate {
        message,
        addr: query.addr.unwrap_or_default(),
    })
    .into_response();

    // Clear the flash message cookie
    if has_message {
        response.headers_mut().append(
            SET_COOKIE,
            "flash_message=; Path=/wol; Max-Age=0; HttpOnly".parse().unwrap(),
        );
    }

    response
}

async fn wol_post(Form(form): Form<WolForm>) -> impl IntoResponse {
    let message = if let Some(mac) = parse_mac(&form.addr) {
        let magic_packet = MagicPacket::new(&mac);
        match magic_packet.send() {
            Ok(_) => "Sent magic packet",
            Err(_) => "Failed to send magic packet",
        }
    } else {
        "Invalid MAC address"
    };

    let mut response = Redirect::to(&format!("/wol?addr={}", form.addr)).into_response();
    
    let cookie_value = format!(
        "flash_message={}; Path=/wol; HttpOnly",
        urlencoding::encode(message)
    );
    
    response.headers_mut().append(
        SET_COOKIE,
        cookie_value.parse().unwrap(),
    );

    response
}

fn parse_mac(s: &str) -> Option<[u8; 6]> {
    let clean_s = s.replace(':', "").replace('-', "");
    if clean_s.len() != 12 {
        return None;
    }

    let mut arr = [0u8; 6];
    for i in 0..6 {
        arr[i] = u8::from_str_radix(&clean_s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(arr)
}

struct HtmlTemplate<T>(T);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template: {}", err),
            )
                .into_response(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mac() {
        assert_eq!(parse_mac("01:23:45:67:89:AB"), Some([0x01, 0x23, 0x45, 0x67, 0x89, 0xAB]));
        assert_eq!(parse_mac("01-23-45-67-89-AB"), Some([0x01, 0x23, 0x45, 0x67, 0x89, 0xAB]));
        assert_eq!(parse_mac("0123456789AB"), Some([0x01, 0x23, 0x45, 0x67, 0x89, 0xAB]));
        assert_eq!(parse_mac("invalid"), None);
        assert_eq!(parse_mac("01:23:45"), None);
    }
}
