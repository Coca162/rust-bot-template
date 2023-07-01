use std::env;
use std::env::VarError;

use commands::{help::help, ping::pong};
use dotenvy::dotenv;
use sqlx::{postgres::PgPoolOptions, PgPool};

use poise::{serenity_prelude as serenity, Prefix};
use serenity::GatewayIntents;
use tracing::{log::warn, metadata::LevelFilter};
use tracing_subscriber::EnvFilter;

mod commands;

// You might want to change this to include more privileged intents or to make it not be so broad
const INTENTS: GatewayIntents =
    GatewayIntents::non_privileged().union(serenity::GatewayIntents::MESSAGE_CONTENT);

pub type Context<'a> = poise::Context<'a, Data, Error>;
pub type Error = Box<dyn std::error::Error + Send + Sync>;

// Data shared across commands and events
pub struct Data {
    pub db: PgPool,
}

#[tokio::main]
async fn main() {
    // Placed here so nobody forgets to add a new command to the command handler
    let commands = vec![help(), pong()];

    // Logging with configuration from environment variables via the `env-filter` feature
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::WARN.into())
                .from_env_lossy(),
        )
        .init();

    match dotenv() {
        Ok(_) => (),
        Err(err) if err.not_found() => {
            if !not_using_dotenv() {
                warn!("You have not included a .env file! If this is intentional you can disable this warning with `DISABLE_NO_DOTENV_WARNING=1`")
            }
        }
        Err(err) => panic!("Dotenv error: {}", err),
    }

    // These are done at runtime so changes can be made when running the bot without the need of a recompilation
    let token = env::var("DISCORD_TOKEN").expect("No discord token found in environment variables");
    let database_url =
        env::var("DATABASE_URL").expect("No database url found in environment variables");
    let (primary_prefix, addition_prefixes) = parse_prefixes();

    // Setting up database connections
    let db = PgPoolOptions::new()
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");

    // // Makes sure the sql tables are updated to the latest definitions
    // sqlx::migrate!()
    //     .run(&db)
    //     .await
    //     .expect("Unable to apply migrations!");

    let data = Data { db: db.clone() };

    let framework_builder = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: primary_prefix,
                additional_prefixes: addition_prefixes,
                edit_tracker: Some(poise::EditTracker::for_timespan(
                    std::time::Duration::from_secs(120),
                )),
                ..Default::default()
            },
            commands,
            ..Default::default()
        })
        .token(token)
        .intents(INTENTS)
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;

                Ok(data)
            })
        });

    // Build the framework
    let framework = framework_builder
        .build()
        .await
        .expect("Cannot build the bot framework!");

    // ctrl+c handler for graceful shutdowns
    let shard_handler = framework.shard_manager().clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Cannot register a ctrl+c handler!");

        tracing::info!("Shutting down the bot!");
        shard_handler.lock().await.shutdown_all().await;
        db.close().await;
    });

    tracing::info!("Starting the bot!");
    framework.start().await.unwrap();
}

fn not_using_dotenv() -> bool {
    match env::var("DISABLE_NO_DOTENV_WARNING")
        .map(|x| x.to_ascii_lowercase())
        .as_deref()
    {
        Ok("1" | "true") => true,
        Ok("0" | "false") => false,
        Ok(_) => {
            panic!("DISABLE_NO_DOTENV_WARNING environment variable is not a valid value (1/0/true/false)")
        }
        Err(VarError::NotPresent) => false,
        Err(VarError::NotUnicode(err)) => panic!(
            "DISABLE_NO_DOTENV_WARNING environment variable is not set to valid Unicode, found: {:?}",
            err
        ),
    }
}

fn parse_prefixes() -> (Option<String>, Vec<Prefix>) {
    let unparsed = match env::var("PREFIXES") {
        Ok(unparsed) => unparsed,
        // The defaults for prefix & additional_prefixes is these
        Err(VarError::NotPresent) => return (None, Vec::new()),
        _ => panic!("Could not handle the environment variable for prefixes"),
    };

    let mut split = unparsed.split(' ').map(|x| x.to_string());

    let first = split
        .next()
        .expect("Could not parse prefixes from environment variables");

    // We need to leak these strings since `Prefix::Literal` only accepts `&'static str` for some reason
    let split = split
        .map(|x| Box::leak(Box::new(x)))
        .map(|x| Prefix::Literal(x));

    (Some(first), split.collect())
}
