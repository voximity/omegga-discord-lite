use anyhow::Result;
use dashmap::mapref::entry::Entry;
use futures::StreamExt;
use omegga::resources::Player;
use serde_json::Value;
use twilight_gateway::{shard::Events, Event};
use twilight_model::{channel::Message, id::RoleId};

use crate::{
    format::{format_content, format_to_game, role_text, Formatter},
    State,
};

pub async fn reply(state: &State, message: &Message, content: &str) -> Result<()> {
    state
        .http
        .create_message(message.channel_id)
        .content(content)?
        .reply(message.id)
        .exec()
        .await?;
    Ok(())
}

pub async fn update_verified(state: &State, message: &Message, player: &Player) -> Result<()> {
    if !state.config.verified_role.is_empty() {
        let _ = state
            .http
            .add_guild_member_role(
                message.guild_id.unwrap(),
                message.author.id,
                RoleId(state.config.verified_role.parse().unwrap()),
            )
            .exec()
            .await;
    }

    if state.config.verified_nickname {
        let _ = state
            .http
            .update_guild_member(message.guild_id.unwrap(), message.author.id)
            .nick(Some(player.name.as_str()))?
            .exec()
            .await;
    }

    Ok(())
}

pub async fn listener(state: State, mut events: Events) -> Result<()> {
    let current_user = state.http.current_user().exec().await?.model().await?;

    while let Some(event) = events.next().await {
        state.cache.update(&event);

        match event {
            Event::Ready(_) => {
                state.omegga.log("Discord client is ready.");
            }
            Event::MessageCreate(message) => {
                // reject the bot
                if message.author.id == current_user.id {
                    continue;
                }

                // only accept messages in the current channel
                if message.channel_id != state.channel_id {
                    continue;
                }

                // parse commands if the message starts with the prefix
                let prefix = &state.config.discord_prefix;
                if message.content.starts_with(prefix) {
                    let (cmd, args) = message
                        .content
                        .split_once(' ')
                        .map(|(c, a)| (&c[prefix.len()..], a))
                        .unwrap_or((&message.content[prefix.len()..], &message.content[0..0]));

                    state.omegga.log(format!(
                        "{} is trying to run {} with {}",
                        message.author.name, cmd, args
                    ));

                    match cmd {
                        "players" => {
                            let players = state.omegga.get_players().await?;
                            if players.is_empty() {
                                reply(&state, &message.0, "**There are no players online.**")
                                    .await?;
                            } else {
                                let mut response = format!(
                                    "**There {} {} player{} online.**\n",
                                    if players.len() == 1 { "is" } else { "are" },
                                    players.len(),
                                    if players.len() == 1 { "" } else { "s" }
                                );

                                for player in players.iter() {
                                    let roles = state
                                        .omegga
                                        .get_player_roles(&player.name)
                                        .await?
                                        .unwrap_or_else(Vec::new);
                                    let role_text = role_text(&roles, &state.config.game_roles);

                                    response.push_str(
                                        format!(
                                            "{}{}{}\n",
                                            role_text,
                                            if !role_text.is_empty() { " " } else { "" },
                                            player.name
                                        )
                                        .as_str(),
                                    );
                                }

                                reply(&state, &message.0, response.as_str()).await?;
                            }
                        }
                        "verify" => {
                            if !state.config.verification {
                                continue;
                            }

                            match args {
                                "" => {
                                    if let Some(player) = state
                                        .omegga
                                        .get_player(
                                            state
                                                .omegga
                                                .store_get(
                                                    format!("d2g_{}", message.author.id).as_str(),
                                                )
                                                .await?
                                                .unwrap_or_default()
                                                .as_str()
                                                .unwrap_or_default(),
                                        )
                                        .await?
                                    {
                                        // update on discord
                                        update_verified(&state, &message.0, &player).await?;
                                        reply(
                                            &state,
                                            &message.0,
                                            "**Synced verification with game.**",
                                        )
                                        .await?;
                                    } else {
                                        reply(&state, &message.0, "**You are not verified!** Start the verification process by running `/discord verify` in-game.").await?;
                                    }
                                }
                                code => {
                                    let key = match state
                                        .verify_buffer
                                        .iter()
                                        .find(|r| r.value() == code)
                                    {
                                        Some(r) => r.key().to_owned(),
                                        None => {
                                            reply(
                                                &state,
                                                &message.0,
                                                format!(
                                                    "**There is no pending verification with that code!**"
                                                )
                                                .as_str(),
                                            )
                                            .await?;
                                            continue;
                                        }
                                    };

                                    if let Entry::Occupied(entry) = state.verify_buffer.entry(key) {
                                        // fetch the in-game player
                                        let player = state
                                            .omegga
                                            .get_player(entry.key().to_owned())
                                            .await?
                                            .unwrap();

                                        // add to the database
                                        state.omegga.store_set(
                                            format!("g2d_{}", entry.key()),
                                            Value::String(message.author.id.to_string()),
                                        );

                                        state.omegga.store_set(
                                            format!("d2g_{}", message.author.id),
                                            Value::String(entry.key().to_string()),
                                        );

                                        // remove from the dashmap
                                        entry.remove();

                                        // confirm to the user that they've been verified in discord
                                        reply(
                                            &state,
                                            &message.0,
                                            format!(
                                                "**Success!** You've been verified as **{}** in Brickadia.",
                                                player.name
                                            )
                                            .as_str(),
                                        )
                                        .await?;

                                        // update on discord
                                        update_verified(&state, &message.0, &player).await?;

                                        // confirm in-game
                                        state.omegga.whisper(player.name, format!("<color=\"0a0\"><b>Success!</></> You've been verified as <b>{}</> in Discord.", message.author.name));
                                    }
                                }
                            }
                        }
                        _ => (),
                    }
                }

                // get user info
                let member = message.member.as_ref().unwrap();
                let mut roles = member
                    .roles
                    .iter()
                    .map(|id| state.cache.role(*id).unwrap())
                    .collect::<Vec<_>>();

                roles.sort_by_key(|r| r.position);

                let role_color = roles
                    .iter()
                    .find(|r| r.color != 0)
                    .map(|r| r.color)
                    .unwrap_or(0xaaaaaa_u32);

                // declare the message formatters
                let formatters = vec![
                    Formatter {
                        key: "role",
                        value: role_text(
                            &roles.iter().map(|r| r.name.to_owned()).collect::<Vec<_>>(),
                            &state.config.discord_roles,
                        ),
                    },
                    Formatter {
                        key: "user",
                        value: member
                            .nick
                            .as_ref()
                            .unwrap_or(&message.author.name)
                            .to_owned(),
                    },
                    Formatter {
                        key: "message",
                        value: format_to_game(message.content.to_owned()),
                    },
                    Formatter {
                        key: "color",
                        value: format!("{:06x}", role_color),
                    },
                ];

                state.omegga.broadcast(format_content(
                    state.config.game_message_format.clone(),
                    &formatters,
                ));

                state
                    .omegga
                    .log(format_content("<$user> $message".into(), &formatters));
            }
            _ => (),
        }
    }

    Ok(())
}
