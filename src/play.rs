use std::collections::HashMap;
use std::fmt::Display;
use std::sync::Arc;

use crate::game::*;

use serenity::http::Http;
use serenity::model::id::{ChannelId, UserId};
use serenity::model::{channel::Message, gateway::Ready};
use serenity::{async_trait, prelude::*};

use crate::{game::EvalError, GameBoard};

const ROLE_ID: u64 = 864243710576689223;

macro_rules! format_md {
    ($str: literal) => {
        concat!("```", $str, "```").to_owned()
    };

    ($str: literal, $($arg: tt)*) => {
        format!(concat!("```", $str, "```"), $($arg)*)
    };
}

/// A map from channels into games.
#[derive(Debug, Default)]
pub struct GamesMap(HashMap<ChannelId, Arc<RwLock<GameConfig>>>);

impl TypeMapKey for GamesMap {
    type Value = Self;
}

/// Stores the current game and its configuration.
#[derive(Debug)]
pub struct GameConfig {
    /// The maximum number of steps any Brainfuck command is evaluated for.
    steps: u32,

    /// The game board.
    board: GameBoard,

    /// The user IDs of the players in turn.
    player_ids: Vec<UserId>,

    /// Whether a game is currently being played.
    active: bool,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            steps: 1_000_000,
            board: Default::default(),
            player_ids: Vec::new(),
            active: false,
        }
    }
}

impl GameConfig {
    /// Evaluates a Brainfuck string, and runs it.
    fn eval(&mut self, str: &str) -> Option<EvalResult<()>> {
        self.active.then(|| self.board.eval(str, self.steps))
    }

    fn reset(&mut self) {
        self.active = false;
        self.player_ids = Vec::new();
        self.board.reset();
    }

    fn winners(&self) -> Option<Winners> {
        self.board.winners()
    }

    fn id(&self) -> Option<UserId> {
        self.player_ids.get(self.board.player_idx()).copied()
    }
}

/// A helper struct whose associated methods wrap around some common operations.
struct MessageHelper<'a> {
    ctx: &'a Context,
    channel_id: ChannelId,
}

impl<'a> MessageHelper<'a> {
    fn new(ctx: &'a Context, msg: &'a Message) -> Self {
        Self {
            ctx,
            channel_id: msg.channel_id,
        }
    }

    fn http(&self) -> &Arc<Http> {
        &self.ctx.http
    }

    async fn post<T: Display>(&self, contents: T) {
        if let Err(why) = self.channel_id.say(self.http(), contents).await {
            println!("Error sending message: {:?}", why);
        }
    }

    async fn game_config_lock(&self) -> Arc<RwLock<GameConfig>> {
        let data_read = self.ctx.data.read().await;
        let games_map = data_read.get::<GamesMap>().unwrap();
        if let Some(lock) = games_map.0.get(&self.channel_id) {
            lock.clone()
        } else {
            drop(data_read);
            let mut data_write = self.ctx.data.write().await;
            let lock: Arc<RwLock<GameConfig>> = Default::default();
            data_write
                .get_mut::<GamesMap>()
                .unwrap()
                .0
                .insert(self.channel_id, lock.clone());
            lock
        }
    }

    async fn game_config<Output, F: FnOnce(&GameConfig) -> Output>(&self, f: F) -> Output {
        let game_config_lock = self.game_config_lock().await;

        let game_config = game_config_lock.read().await;
        f(&*game_config)
    }

    async fn game_config_mut<Output, F: FnOnce(&mut GameConfig) -> Output>(&self, f: F) -> Output {
        let game_config_lock = self.game_config_lock().await;

        let mut game_config = game_config_lock.write().await;
        f(&mut *game_config)
    }
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
        let msg_helper = MessageHelper::new(&ctx, &msg);

        /// Posts a formatted message.
        macro_rules! post {
            ($($arg: tt)*) => { msg_helper.post(format!($($arg)*)).await }
        }

        /// Posts a formatted message between triple backticks.
        macro_rules! post_md {
            ($($arg: tt)*) => { msg_helper.post(format_md!($($arg)*)).await }
        }

        /// Gets the game configuration and applies a function to its reference.
        macro_rules! game_config {
            ($f: expr) => {
                msg_helper.game_config($f).await
            };
        }

        /// Gets the game configuration and applies a function to its mutable reference.
        macro_rules! game_config_mut {
            ($f: expr) => {
                msg_helper.game_config_mut($f).await
            };
        }

        // Checks for the Gamer role.
        let has_role = match msg
            .author
            .has_role(&ctx.http, msg.guild_id.unwrap(), ROLE_ID)
            .await
        {
            // Whether the message author has the role.
            Ok(res) => res,

            // We couldn't check the role.
            Err(err) => {
                println!("{}", err);
                false
            }
        };

        // Ignore messages from bots, empty messages, or people without the correct role.
        if msg.author.bot || msg.content.chars().all(char::is_whitespace) || !has_role {
            return;
        }

        // Splits the message into tokens.
        let mut components = msg.content.split_whitespace();

        match components.next() {
            // Sets up some options.
            Some("set") => match components.next() {
                // Setups the player characters.
                Some("players") => {
                    let res = game_config_mut!(|cfg| {
                        let mut players = Vec::new();

                        for component in components {
                            if component.len() != 1 {
                                return "Each player must be represented by a single character!"
                                    .to_owned();
                            } else {
                                players.push(Player::new(component.chars().next().unwrap()));
                            }
                        }

                        match players.len(){
                            0 => "Configure the players. Specify the characters that will be used to represent each player as a list separated by spaces.".to_owned(), 
                            1 => "Players could not be updated: must be at least 2.".to_owned(),
                            _ => {
                                let mut players_sorted = players.clone();
                                players_sorted.sort();

                                // Checks for repeat characters.
                                for i in 0..players_sorted.len() - 1 {
                                    if players_sorted[i] == players_sorted[i + 1]{
                                        return format!("Players could not be updated: repeated character {}.", players_sorted[i]);
                                    }
                                }

                                cfg.board.players = Players::new(players);
                                "Players succesfully updated!".to_owned()
                            }
                        }
                    });

                    post_md!("{}", res);
                }

                // Setups the maximum number of steps any instruction runs for.
                Some("steps") => {
                    if let Some(component) = components.next() {
                        if let Ok(steps) = component.parse::<u32>() {
                            game_config_mut!(|cfg| cfg.steps = steps);
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
                        game_config_mut!(|cfg| cfg.board = GameBoard::new(capacities));
                        post_md!("Board succesfully updated!");
                    }
                }

                _ => {}
            },

            // Starts a new game.
            Some("play") => {
                let board = game_config_mut!(|cfg| {
                    if cfg.active {
                        return None;
                    }

                    cfg.active = true;
                    Some(cfg.board.to_string())
                });

                if let Some(board) = board {
                    post_md!("{}", board);
                } else {
                    post_md!("A game is already active!");
                }
            }

            // Shows the current state of the board.
            Some("board") => {
                post_md!(
                    "{}",
                    game_config!(|cfg| if cfg.active {
                        cfg.board.to_string()
                    } else {
                        "No game is currently active!".to_owned()
                    })
                );
            }

            // Resets the game.
            Some("reset") => {
                game_config_mut!(GameConfig::reset);
                post_md!("Reset succesful!");
            }

            // Any message that isn't a command. It might be a move in the game,
            // or perhaps a skip.
            component => {
                let id = msg.author.id;

                let res = game_config_mut!(|cfg| {
                    match cfg.id() {
                        Some(new_id) => {
                            // Ignore messages from the incorrect player.
                            if new_id != id {
                                return None;
                            }
                        }

                        None => {
                            // Ignore messages from repeat users.
                            for old_id in &cfg.player_ids {
                                if *old_id == id {
                                    return None;
                                }
                            }

                            cfg.player_ids.push(id);
                        }
                    }

                    let content = if component == Some("skip") {
                        ""
                    } else {
                        &msg.content
                    };

                    // Evaluates the message as Brainfuck code.
                    if let Some(res) = cfg.eval(content) {
                        // Posts any error, except those by invalid moves, as
                        // they're probably just comments.
                        if let Err(err) = res {
                            if matches!(err, EvalError::InvalidChar { .. }) {
                                None
                            } else {
                                Some(format_md!("Invalid move: {}.", err))
                            }
                        } else {
                            Some(
                                // Posts the winners.
                                if let Some(winners) = cfg.winners() {
                                    let res = format_md!("{}\n{}", winners, cfg.board);
                                    cfg.reset();
                                    res
                                }
                                // Posts the current state of the board, together with the poster.
                                else if let Some(id) = cfg.id() {
                                    format!("<@{}>\n```{}```", id, cfg.board)
                                }
                                // Posts the current state of the board.
                                else {
                                    format_md!("{}", cfg.board)
                                },
                            )
                        }
                    } else {
                        None
                    }
                });

                if let Some(post) = res {
                    post!("{}", post);
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
