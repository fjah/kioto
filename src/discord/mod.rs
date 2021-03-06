mod commands;
mod util;

use serenity::{
    async_trait,
    client::{Client, Context, EventHandler},
    framework::standard::{
        help_commands,
        macros::{help, hook},
        Args, CommandGroup, CommandResult, HelpOptions, StandardFramework,
    },
    http::Http,
    model::{
        event::ResumedEvent,
        gateway::Ready,
        prelude::{Message, UserId},
    },
};

use crate::fatal;
use crate::util::{DbPool, Settings};
use log::{debug, error, info};
use std::collections::HashSet;
use std::sync::Arc;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        if let Some(shard) = ready.shard {
            info!(
                "Shard {}/{} is serving {}",
                shard[0], shard[1], ready.user.name,
            );
        } else {
            info!("Bot is serving {}", ready.user.name);
        }
    }

    async fn resume(&self, _: Context, _: ResumedEvent) {
        info!("Client resumed");
    }
}

#[hook]
async fn before_hook(_ctx: &Context, msg: &Message, cmd_name: &str) -> bool {
    debug!(
        "Handling '{}' command for {}#{}",
        cmd_name, msg.author.name, msg.author.discriminator
    );

    true
}

#[help]
async fn help_cmd(
    context: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    help_commands::with_embeds(context, msg, args, help_options, groups, owners).await;
    Ok(())
}

pub async fn init(pool: Arc<DbPool>) -> Result<(), Box<dyn std::error::Error>> {
    let cfg = Settings::get();
    let http = Http::new_with_token(&cfg.bot.token);
    let (owners, bot_id) = match http.get_current_application_info().await {
        Ok(info) => {
            let mut owners = HashSet::new();
            owners.insert(info.owner.id);

            (owners, info.id)
        }
        Err(why) => fatal!("Couldn't get app info: {:?}", why),
    };

    let framework = StandardFramework::new()
        .configure(|c| {
            c.prefix(&cfg.bot.prefix)
                .on_mention(Some(bot_id))
                .owners(owners)
        })
        .help(&HELP_CMD)
        .before(before_hook)
        .group(&commands::general::GENERAL_GROUP);
    let mut client = Client::new(&cfg.bot.token)
        .event_handler(Handler)
        .framework(framework)
        .await?;

    {
        let mut data = client.data.write().await;
        data.insert::<util::ClientShardManager>(Arc::clone(&client.shard_manager));
        data.insert::<util::DbPool>(Arc::clone(&pool));
        data.insert::<Settings>(Arc::new(cfg));
    }

    client.start_autosharded().await.map_err(|e| e.into())
}
