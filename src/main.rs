mod backup;
mod catalog;
mod db;
mod history;
mod orders;
mod push;
mod users;
mod web;

use std::{error::Error, net::Ipv4Addr, path::PathBuf, sync::Arc};

use clap::{Parser, Subcommand};
use tokio::net::TcpListener;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Serve {
        #[command(flatten)]
        storage: Storage,
        /// Address to listen on; use 0.0.0.0 behind the office gateway.
        #[arg(long, env = "KRINGKRING_HOST", default_value_t = Ipv4Addr::LOCALHOST)]
        host: Ipv4Addr,
        #[arg(long, env = "KRINGKRING_PORT", default_value_t = 8080)]
        port: u16,
        /// VAPID contact sent to push services (mailto: or https: URL).
        #[arg(
            long,
            env = "KRINGKRING_PUSH_CONTACT",
            default_value = "mailto:admin@kringkring.local"
        )]
        push_contact: String,
    },
    Init {
        #[command(flatten)]
        storage: Storage,
        #[arg(long)]
        username: String,
        #[arg(long)]
        pin: String,
    },
}

#[derive(clap::Args)]
struct Storage {
    #[arg(
        long,
        env = "KRINGKRING_DATA_DIR",
        default_value = "/var/lib/kringkring"
    )]
    data_dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    match Cli::parse().command {
        Command::Serve {
            storage,
            host,
            port,
            push_contact,
        } => {
            let db = db::initialize(&storage.data_dir)?;
            tokio::spawn(backup::run(db.clone(), storage.data_dir.join("backups")));
            let vapid = Arc::new(push::Vapid::load_or_create(
                &storage.data_dir,
                push_contact,
            )?);
            let app = web::router(db.clone())
                .layer(axum::Extension(web::PushKey(vapid.public_key.clone())));
            tokio::spawn(push::run(db, vapid, Arc::new(push::Https::new())));
            let listener = TcpListener::bind((host, port)).await?;
            println!("Listening on http://{}", listener.local_addr()?);
            axum::serve(listener, app).await?;
        }
        Command::Init {
            storage,
            username,
            pin,
        } => {
            let db = db::initialize(&storage.data_dir)?;
            users::init_admin(&db, &username, &pin)?;
            println!("Created the first admin.");
        }
    }
    Ok(())
}
