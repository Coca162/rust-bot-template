use crate::{Context, Error};

/// Pong!
#[poise::command(slash_command, prefix_command, track_edits)]
pub async fn pong(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("pong!").await?;
    Ok(())
}