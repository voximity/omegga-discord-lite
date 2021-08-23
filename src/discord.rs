use anyhow::Result;
use futures::StreamExt;
use twilight_gateway::{shard::Events, Event};
use twilight_model::channel::Message;

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
                    let (cmd, _args) = message
                        .content
                        .split_once(' ')
                        .map(|(c, a)| (&c[prefix.len()..], a))
                        .unwrap_or((&message.content[prefix.len()..], &message.content[0..0]));

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
                        _ => (),
                    }
                }

                // get user info
                let member = message.member.as_ref().unwrap();
                let roles = member
                    .roles
                    .iter()
                    .map(|id| state.cache.role(*id).unwrap())
                    .collect::<Vec<_>>();

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
            }
            _ => (),
        }
    }

    Ok(())
}
