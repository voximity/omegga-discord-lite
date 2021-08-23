# omegga-discord-lite

A lightweight variant of [omegga-discord](https://github.com/technologenesis/omegga-discord),
give or take a few features.

## Installation

At the time of writing, [Rust/Cargo](https://rust-lang.org/) is required to build and install this project.

`omegga install gh:voximity/discord-lite`

## Setup

To start, head over to the [Discord Developers page](https://discord.com/developers) and create a new bot.
Find its token and add it to your server. This process can be seen more thoroughly [here](https://discordpy.readthedocs.io/en/latest/discord.html).

A few configuration settings are required to be set up initially.

| **Name** | **Type** | **Description** |
| --- | --- | --- |
| `token` | string | The token of the bot that will be used to mirror messages. |
| `channel-id` | string | The channel ID of the channel that the bot will mirror messages to. |

The following configuration options are optional, as they have defaults or are not enabled.

| **Name** | **Type** | **Default** | **Description** |
| --- | --- | --- | --- |
| `channel-name-online-format` | string | *(blank)* | When this field is set, the channel's name will dynamically change when a player joins or leaves the game. It has the formatter `$n`, which is the number of players online. See the section on formatters below. |
| `discord-prefix` | string | `!` | The prefix for commands through Discord, like `!players`. |
| `game-message-format` | string | `<color="$color"><b>$user</></>: $message` | The format for messages from Discord to in-game. It has the formatters `$message` (the message content), `$user` (the nickname/username of the speaking user), `$color` (the user's role color in hexadecimal), and `$role` (see the section on Role Formatters below). |
| `discord-message-format` | string | `**$user**: $message` | The format for messages from in-game to Discord. It has the formatters `$message` (the message content), `$user` (the nickname/username of the speaking user), and `$role` (see the section on Role Formatters below). |
| `join-message-format` | string | `**$user joined the game.**` | The format for players joining the game. It has the formatters `$user` (the joining user) and `$role` (see the section on Role Formatters below). |
| `leave-message-format` | string | `**$user left the game.**` | See above. |
| `server-start-format` | string | `**The server has started.**` | The format for when the server starts. It has the formatter `$map` (the map the server started on). |
| `game-roles` | \[string\] | *(empty)* | Role formatters for in-game roles going to Discord. See the section on Role Formatters below. |
| `discord-roles` | \[string\] | *(empty)* | Role formatters for Discord roles going in-game. See the section on Role Formatters below. |

### Formatters

Every configuration option ending in `format` has at least one "formatter," which is a piece of text that will be
replaced when it is shown in-game. All formatters start with `$`, for example `$user`.

In the example `$user joined the game.`, `$user` gets replaced by the joining user automatically. For more complicated
formatting like `$user: $message`, these two formatters get replaced by the chatting user and the message respectively.

The most complicated example I'll give is the default of `game-message-format` which is `<color="$color"><b>$user</></>: $message`.
The formatter `$color` is the hex code of the Discord role name color. Use the Brickadia chat code `<color="HEXCODE">...</>` to set
the color of the text.

#### Role formatters

Every format that has the `$user` formatter also has a `$role` formatter. Depending on what you set `game-roles` and `discord-roles` to,
this changes dynamically.

`game-roles` and `discord-roles` are both lists of role formats, which are formatted like `ROLE NAME:TEXT`. When a user has a role defined
in this list by `ROLE NAME`, their `$role` formatter becomes `TEXT`. For example, if you use the role `Admin` and want to replace it with
the text `[admin]`, you can use the role format `Admin:[admin]`.

The highest role in the hierarchy is prioritized, so whatever role is highest is the one that takes priority. You can use the role name
`default` to dictate the fallback if the user has no other role format.

### An example setup

In our example, we will define the following:

`game-message-format` will be `$role <color="$color"><b>$user</></>: $message`

`discord-message-format` will be `$role **$user**: $message`

`game-roles` will be

* `Admin::hammer:`
* `default::yellow_circle:`

`discord-roles` will be

* `Admin::blegg:`
* `default::egg:`

Normal users chatting in game will have their messages be prefixed with a yellow circle in Discord.
Admins in-game will have their messages prefixed by a hammer emote.

Normal users chatting in Discord will have their messages prefixed with an egg emote in-game.
Users with the role Admin in Discord will have their messages prefixed with the blegg emote in-game.

## Credits

* voximity - creator, maintainer
* [Meshiest](https://github.com/meshiest) - Omegga
* [Technologenesis](https://github.com/technologenesis) - [omegga-discord](https://github.com/technologenesis/omegga-discord)
