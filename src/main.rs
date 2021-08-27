mod discord;
mod format;

use std::{fs::File, sync::Arc};

use anyhow::Result;

use dashmap::{mapref::entry::Entry, DashMap};
use format::format_content;
use omegga::{resources::Player, rpc, Omegga};
use rand::{distributions, Rng};
use serde::{Deserialize, Serialize};
use serde_json::json;
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{Intents, Shard};
use twilight_http::Client as HttpClient;
use twilight_model::id::ChannelId;

use crate::format::{compose_vec, role_text, Formatter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub token: String,

    #[serde(rename = "channel-id")]
    pub channel_id: String,

    #[serde(rename = "channel-name-online-format")]
    pub channel_name_online_format: String,

    #[serde(rename = "discord-prefix")]
    pub discord_prefix: String,

    #[serde(rename = "game-message-format")]
    pub game_message_format: String,

    #[serde(rename = "discord-message-format")]
    pub discord_message_format: String,

    #[serde(rename = "join-message-format")]
    pub join_message_format: String,

    #[serde(rename = "leave-message-format")]
    pub leave_message_format: String,

    #[serde(rename = "server-start-format")]
    pub server_start_format: String,

    #[serde(rename = "game-roles")]
    pub game_roles: Vec<String>,

    #[serde(rename = "discord-roles")]
    pub discord_roles: Vec<String>,

    #[serde(rename = "verified-role")]
    pub verified_role: String,

    #[serde(rename = "verified-nickname")]
    pub verified_nickname: bool,
}

#[derive(Clone)]
pub struct State {
    /// The plugin config.
    pub config: Arc<Config>,

    /// The Discord HTTP client.
    pub http: HttpClient,

    /// The Omegga interface.
    pub omegga: Arc<Omegga>,

    /// The Discord cache.
    pub cache: InMemoryCache,

    /// The ID messages are being sent to in Discord.
    pub channel_id: ChannelId,

    /// A buffer of player UUID to verification code to verify on Discord.
    pub verify_buffer: Arc<DashMap<String, String>>,
}

async fn user_formatters(state: &State, user: String) -> Result<Vec<Formatter>> {
    let roles = state
        .omegga
        .get_player_roles(&user)
        .await?
        .unwrap_or_else(Vec::new);

    Ok(vec![
        Formatter {
            key: "role",
            value: role_text(&roles, &state.config.game_roles),
        },
        Formatter {
            key: "user",
            value: user,
        },
    ])
}

#[tokio::main]
async fn main() -> Result<()> {
    // start omegga
    let omegga = Arc::new(Omegga::new());
    let mut rx = omegga.spawn();

    // read the config
    let config = Arc::new(serde_json::from_reader::<_, Config>(File::open(
        "config.json",
    )?)?);

    // connect to discord's gateway
    let (shard, events) = Shard::builder(
        &config.token,
        Intents::GUILDS | Intents::GUILD_MESSAGES | Intents::DIRECT_MESSAGES,
    )
    .build();

    // instantiate a discord http client
    let http = HttpClient::new(config.token.clone());
    let channel_id = ChannelId(config.channel_id.parse().unwrap());

    // start a cache for discord resources
    let cache = InMemoryCache::builder()
        .resource_types(ResourceType::GUILD | ResourceType::MEMBER | ResourceType::ROLE)
        .build();

    // handle discord events in a separate task
    let state = State {
        config,
        http,
        omegga,
        cache,
        channel_id,
        verify_buffer: Arc::new(DashMap::new()),
    };

    let task_state = state.clone();
    tokio::spawn(async move {
        if let Err(_error) = discord::listener(task_state.clone(), events).await {
            task_state
                .omegga
                .error(format!("Error while listening to Discord: {}", _error));
        }
    });

    while let Some(message) = rx.recv().await {
        match message {
            rpc::Message::Request { method, id, .. } if method == "init" || method == "stop" => {
                match method.as_str() {
                    "init" => {
                        shard.start().await?;
                        state.omegga.write_response(
                            id,
                            Some(json!({"registeredCommands": ["discord"]})),
                            None,
                        );
                    }
                    "stop" => state.omegga.write_response(id, None, None),
                    _ => (),
                }
            }
            rpc::Message::Notification { method, params, .. } if method == "start" => {
                #[derive(Serialize, Deserialize, Default)]
                struct StartObject {
                    map: String,
                }

                let params = serde_json::from_value::<Vec<StartObject>>(match params {
                    Some(p) => p,
                    None => continue,
                })
                .unwrap()
                .into_iter()
                .next()
                .unwrap_or_default();

                state
                    .http
                    .create_message(channel_id)
                    .content(&format_content(
                        state.config.server_start_format.clone(),
                        &[Formatter {
                            key: "map",
                            value: params.map,
                        }],
                    ))?
                    .exec()
                    .await?;
            }
            rpc::Message::Notification { method, params, .. } if method == "chat" => {
                let mut params = serde_json::from_value::<Vec<String>>(match params {
                    Some(p) => p,
                    None => continue,
                })
                .unwrap()
                .into_iter();

                let user = params.next().unwrap();
                let message = params.next().unwrap_or_else(String::new);

                let formatters = compose_vec(vec![
                    user_formatters(&state, user.clone()).await?,
                    vec![Formatter {
                        key: "message",
                        value: message,
                    }],
                ]);

                state
                    .http
                    .create_message(channel_id)
                    .content(&format_content(
                        state.config.discord_message_format.clone(),
                        &formatters,
                    ))?
                    .exec()
                    .await?;
            }
            rpc::Message::Notification { method, params, .. }
                if method == "join" || method == "leave" =>
            {
                let mut params = serde_json::from_value::<Vec<Player>>(match params {
                    Some(p) => p,
                    None => continue,
                })
                .unwrap()
                .into_iter();

                let player = params.next().unwrap();

                if method == "leave" {
                    // remove from the verify buffer if the player leaves
                    state.verify_buffer.remove(&player.id);
                }

                let formatters = user_formatters(&state, player.name.clone()).await?;

                state
                    .http
                    .create_message(channel_id)
                    .content(&format_content(
                        match method.as_str() {
                            "join" => state.config.join_message_format.clone(),
                            "leave" => state.config.leave_message_format.clone(),
                            _ => unreachable!(),
                        },
                        &formatters,
                    ))?
                    .exec()
                    .await?;

                if !state.config.channel_name_online_format.is_empty() {
                    let players = state.omegga.get_players().await?;
                    let name = format_content(
                        state.config.channel_name_online_format.to_owned(),
                        &vec![Formatter {
                            key: "n",
                            value: players.len().to_string(),
                        }],
                    );
                    if let Err(error) = state
                        .http
                        .update_channel(channel_id)
                        .name(&name)?
                        .exec()
                        .await
                    {
                        state
                            .omegga
                            .error(format!("Error on updating channel: {}", error));
                    }
                }
            }
            rpc::Message::Notification { method, params, .. } if method == "cmd:discord" => {
                let mut params = serde_json::from_value::<Vec<String>>(params.unwrap())
                    .unwrap()
                    .into_iter();
                let user = params.next().unwrap();
                let subcommand = match params.next() {
                    Some(s) => s,
                    None => {
                        state
                            .omegga
                            .whisper(&user, "<color=\"a00\">Please specify a command to run.</>");
                        continue;
                    }
                };
                let _args = params.collect::<Vec<_>>();

                match subcommand.as_str() {
                    "wipe" => {
                        let player = state.omegga.get_player(&user).await?.unwrap();
                        if player.host.unwrap_or(false) {
                            state.omegga.store_wipe();
                            state.omegga.broadcast("Verification store has been wiped.");
                        }
                    }
                    "verify" => {
                        let player = state.omegga.get_player(&user).await?.unwrap();

                        // check if the user is already verified
                        let entry = state
                            .omegga
                            .store_get(format!("g2d_{}", player.id))
                            .await?;

                        match entry {
                            Some(_) => {
                                // the user is verified
                                state
                                    .omegga
                                    .whisper(&user, "<color=\"a00\">You are already verified!</>");
                            }
                            None => {
                                // the user is not verified
                                match state.verify_buffer.entry(player.id) {
                                    Entry::Occupied(entry) => state.omegga.whisper(&user, format!(
                                        "<color=\"a00\">You have already initiated the verification process! Send <code>{}verify {}</> in the game channel.</>",
                                        state.config.discord_prefix,
                                        entry.get()
                                    )),
                                    Entry::Vacant(entry) => {
                                        let code = rand::thread_rng()
                                            .sample_iter(&distributions::Alphanumeric)
                                            .take(6)
                                            .map(char::from)
                                            .collect::<String>();

                                        let entry = entry.insert(code);
                                        state.omegga.whisper(&user, format!(
                                            "To verify, send <code>{}verify {}</> in the game channel.",
                                            state.config.discord_prefix,
                                            entry.value()
                                        ));
                                    }
                                }
                            }
                        }
                    }

                    unknown => state.omegga.whisper(
                        &user,
                        format!(
                            "<color=\"a00\">There is no Discord command by the name <code>/discord {}</>.</>",
                            unknown
                        ),
                    ),
                }
            }
            _ => (),
        }
    }

    Ok(())
}
