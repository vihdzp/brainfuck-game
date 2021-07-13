use std::{collections::HashMap, sync::Arc};

use crate::{game::EvalError, GameBoard, GameConfig, MAX_PLAYERS};
use serenity::{
    async_trait,
    model::{channel::Message, id::ChannelId, prelude::Ready},
    prelude::*,
};

const ROLE_ID: u64 = 864243710576689223;

#[derive(Debug, Default)]
pub struct GamesMap(HashMap<ChannelId, Arc<RwLock<GameConfig>>>);

impl std::ops::Deref for GamesMap {
    type Target = HashMap<ChannelId, Arc<RwLock<GameConfig>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for GamesMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl TypeMapKey for GamesMap {
    type Value = Self;
}

pub struct GameHandler;

#[async_trait]
impl EventHandler for GameHandler {
    // Set a handler for the `message` event - so that whenever a new message
    // is received - the closure (or function) passed will be called.
    //
    // Event handlers are dispatched through a threadpool, and so multiple
    // events can be dispatched simultaneously.
    async fn message(&self, ctx: Context, msg: Message) {
        /// Posts a formatted message.
        macro_rules! post {
            ($($arg:tt)*) => {
                let contents = format!($($arg)*);

                if let Err(why) = msg.channel_id.say(&ctx.http, format!("{}", contents)).await {
                    println!("Error sending message: {:?}", why);
                }
            };
        }

        /// Posts a formatted message, enclosed in triple backticks.
        macro_rules! post_md {
            ($($arg:tt)*) => {
                post!("```{}```", format!($($arg)*));
            };
        }

        let game_config_lock = {
            let mut data_read = ctx.data.write().await;
            let games_map = data_read.get_mut::<GamesMap>().unwrap();
            if let Some(gcl) = games_map.get(&msg.channel_id) {
                gcl.clone()
            } else {
                games_map.insert(msg.channel_id, Default::default());
                games_map[&msg.channel_id].clone()
            }
        };

        /// Changes a given field of the game configuration to the specified
        /// value. Wraps around a bunch of mutex bologna.
        macro_rules! game_config {
            ($field: ident, $val: expr) => {{
                let mut game_config = game_config_lock.write().await;
                game_config.$field = $val;
            }};
        }

        // Checks for the Gamer role.
        let has_role = msg
            .author
            .has_role(&ctx.http, msg.guild_id.unwrap(), ROLE_ID)
            .await
            .expect("Could not retrieve role!");

        // Ignore messages from bots, empty messages, or people without the correct role.
        if msg.author.bot || msg.content.chars().all(char::is_whitespace) || !has_role {
            return;
        }

        let mut components = msg.content.split_whitespace();

        match components.next() {
            // Sets up some options.
            Some("set") => match components.next() {
                // Setups the amount of players.
                Some("players") => {
                    if let Some(component) = components.next() {
                        if let Ok(num) = component.parse::<u8>() {
                            if num > 1 && num <= MAX_PLAYERS {
                                game_config!(player_count, num);
                                post_md!("Player count updated to {}.", num);
                            } else {
                                post_md!("Player count could not be updated: must be at least 2 and at most {}", MAX_PLAYERS);
                            }
                        } else {
                            post_md!("Player count could not be parsed.");
                        }
                    } else {
                        post_md!("Specify the number of players that will play.");
                    }
                }

                // Setups the maximum number of steps any instruction runs for.
                Some("steps") => {
                    if let Some(component) = components.next() {
                        if let Ok(steps) = component.parse::<u32>() {
                            game_config!(steps, steps);
                            post_md!("Maximum program steps updated to {}.", steps);
                        } else {
                            post_md!("Step count could not be parsed.");
                        }
                    } else {
                        post_md!("Specify the maximum amount of steps a Brainfuck code should run for before halting.");
                    }
                }

                // Setups the board layout.
                Some("board") => {
                    let mut capacities = Vec::new();

                    for component in components {
                        if let Ok(num) = component.parse::<u16>() {
                            capacities.push(num as usize);
                        } else {
                            post_md!("Could not parse board.");
                            break;
                        }
                    }

                    if capacities.is_empty() {
                        post_md!("Configure the board. Specify the capacities of the buckets as a list separated by spaces.");
                    } else {
                        game_config!(board, GameBoard::new(capacities));
                        post_md!("Board succesfully updated!");
                    }
                }

                _ => {}
            },

            // Starts a new game.
            Some("play") => {
                let mut game_config = game_config_lock.write().await;

                if game_config.active {
                    post_md!("A game is already active!");
                    return;
                }

                game_config.active = true;
                post_md!("{}", game_config.board);
            }

            // Shows the current state of the board.
            Some("board") => {
                let game_config = game_config_lock.read().await;
                post_md!("{}", game_config.board);
            }

            // Resets the game.
            Some("reset") => {
                {
                    let mut game_config = game_config_lock.write().await;
                    game_config.reset();
                }

                post_md!("Reset succesful!");
            }

            // Any message that isn't a command. It might be a move in the game.
            _ => {
                let mut game_config = game_config_lock.write().await;
                let id = msg.author.id;

                match game_config.id() {
                    Some(new_id) => {
                        // Ignore messages from the incorrect player.
                        if new_id != id {
                            return;
                        }
                    }

                    None => {
                        // Ignore messages from repeat users.
                        for old_id in &game_config.player_ids {
                            if *old_id == id {
                                return;
                            }
                        }

                        game_config.player_ids.push(id);
                    }
                }

                // Evaluates the message as Brainfuck code.
                if let Some(res) = game_config.eval(&msg.content) {
                    // Posts any error, except those by invalid moves, as
                    // they're probably just comments.
                    if let Err(err) = res {
                        if !matches!(err, EvalError::InvalidChar { .. }) {
                            post_md!("Invalid move: {}", err);
                        }
                    } else {
                        // Posts the winners.
                        if let Some(winners) = game_config.winners() {
                            post_md!("{}\n{}", winners, game_config.board);
                            game_config.reset();
                        }
                        // Posts the current state of the board.
                        else {
                            if let Some(id) = game_config.id() {
                                post!("<@{}>\n```{}```", id, game_config.board);
                            } else {
                                post_md!("{}", game_config.board);
                            }
                        }
                    }
                }
            }
        }
    }
    // Set a handler to be called on the `ready` event. This is called when a
    // shard is booted, and a READY payload is sent by Discord. This payload
    // contains data like the current user's guild Ids, current user data,
    // private channels, and more.
    //
    // In this case, just print what the current user's username is.
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}
