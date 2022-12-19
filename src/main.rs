use std::env;
use std::env::VarError;

use commands::{help::help, ping::pong};
use dotenvy::dotenv;
use sqlx::{postgres::PgPoolOptions, PgPool};

use poise::{serenity_prelude as serenity, Prefix};
use serenity::GatewayIntents;

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

    if let Err(err) = dotenv() {
        if err.not_found() && !not_using_dotenv() {
            println!("You have not included a .env file! If this is intentional you can disable this warning with `DISABLE_NO_DOTENV_WARNING=1`")
        } else {
            panic!("Panicked on dotenv error: {}", err);
        }
    };

    // Logging with configuration from environment variables via the `env-filter` feature
    tracing_subscriber::fmt::init();

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

    // Makes sure the sql tables are updated to the latest definitions
    sqlx::migrate!()
        .run(&db)
        .await
        .expect("Unable to apply migrations!");

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
    match env::var("DISABLE_NO_DOTENV_WARNING") {
        Ok(value) if value == "1" => true,
        Ok(value) if value == "0" => false,
        Ok(_) => {
            panic!("DISABLE_NO_DOTENV_WARNING environment variable is equal to something other then 1 or 0")
        }
        Err(_) => false,
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
