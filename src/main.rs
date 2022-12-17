use std::env;
use std::env::VarError;

use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use commands::{Data, ping::pong, help::help};

use poise::{serenity_prelude as serenity, Prefix};
use serenity::GatewayIntents;

mod commands;

// You might want to change this to include more privileged intents or to make it not be so broad
const INTENTS: GatewayIntents = GatewayIntents::non_privileged().union(serenity::GatewayIntents::MESSAGE_CONTENT);

#[tokio::main]
async fn main() {
    // Placed here so nobody forgets to add a new command to the command handler
    let commands = vec![help(), pong()];

    dotenv().expect("A .env file does not exist!");

    // These are done at runtime so changes can be made when running the bot without the need of a recompilation
    let token = env::var("DISCORD_TOKEN").expect("No discord token found in .env");
    let database_url = env::var("DATABASE_URL").expect("No database url found in .env");
    let (primary_prefix, addition_prefixes) = parse_prefixes();

    // Logging with configuration from environment variables via the `env-filter` feature
    tracing_subscriber::fmt::init();

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

    let data = Data { db };

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: primary_prefix,
                additional_prefixes: addition_prefixes,
                edit_tracker: Some(poise::EditTracker::for_timespan(std::time::Duration::from_secs(120))),
                ..Default::default()
            },
            commands,
            ..Default::default()
        })
        .token(token)
        .intents(INTENTS)
        .setup(|ctx, _ready, framework| 
            Box::pin(async move { 
                //Override slash commands to update them
                let commands = &framework.options().commands;
                let create_commands = poise::builtins::create_application_commands(commands);

                serenity::Command::set_global_application_commands(ctx, |b| {
                    *b = create_commands; // replace the given builder with the one prepared by poise
                    b
                }).await?;

                Ok(data) 
            }
        ));

    framework.run().await.unwrap();
}

fn parse_prefixes() -> (Option<String>, Vec<Prefix>) {
    let unparsed = match env::var("PREFIXES") {
        Ok(unparsed) => unparsed,
        // The defaults for prefix & additional_prefixes is these
        Err(VarError::NotPresent) => return (None, Vec::new()),
        _ => panic!("Could not handle the .env variable for prefixes")
    };

    let mut split = unparsed.split(' ').map(|x| x.to_string());

    let first = split.next().expect("Could not parse prefixes from .env");

    // We need to leak these strings since `Prefix::Literal` only accepts `&'static str` for some reason
    let split = split.map(|x| Box::leak(Box::new(x))).map(|x| Prefix::Literal(x));

    (Some(first), split.collect())
}