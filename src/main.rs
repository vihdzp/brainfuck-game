use std::env;
use std::sync::Arc;

use crate::game::*;

use serenity::model::id::UserId;
use serenity::model::{channel::Message, gateway::Ready};
use serenity::{async_trait, prelude::*};

pub mod game;

const MAX_PLAYERS: u8 = 8;
const PLAYERS: [char; MAX_PLAYERS as usize] = ['X', 'O', 'Y', 'Z', 'A', 'B', 'C', 'D'];
const ROLE_ID: u64 = 864243710576689223;

/// Stores the current game and its configuration.
struct GameConfig {
    /// The number of players in the game.
    player_count: u8,

    /// The maximum number of steps any Brainfuck command is evaluated for.
    steps: u32,

    /// The game board.
    board: GameBoard,

    player_ids: Vec<UserId>,

    /// Whether a game is currently being played.
    active: bool,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            player_count: 2,
            steps: 1_000_000,
            board: Default::default(),
            player_ids: Vec::new(),
            active: false,
        }
    }
}

impl TypeMapKey for GameConfig {
    type Value = Arc<RwLock<Self>>;
}

impl GameConfig {
    fn eval(&mut self, str: &str) -> Option<EvalResult<()>> {
        self.active
            .then(|| self.board.eval(str, self.steps, self.player_count))
    }

    fn reset(&mut self) {
        self.active = false;
        self.player_ids = Vec::new();
        self.board.reset();
    }

    fn winners(&self) -> Option<Winners> {
        self.board.winners(self.player_count)
    }

    fn id(&self) -> Option<UserId> {
        self.player_ids.get(self.board.player.idx()).copied()
    }
}

struct GameHandler;

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

                if let Err(why) = msg.channel_id.say(&ctx.http, format!("```{}```", contents)).await {
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

        /// Changes a given field of the game configuration to the specified
        /// value. Wraps around a bunch of mutex bologna.
        macro_rules! game_config {
            ($field: ident, $val: expr) => {
                let game_config_lock = {
                    let data_read = ctx.data.read().await;
                    data_read.get::<GameConfig>().unwrap().clone()
                };

                {
                    let mut game_config = game_config_lock.write().await;
                    game_config.$field = $val;
                }
            };
        }

        // Ignore messages from bots, or people without the correct role.
        if msg.author.bot
            || msg
                .author
                .has_role(&ctx.http, msg.guild_id.unwrap(), ROLE_ID)
                .await
                .expect("Could not retrieve role!")
        {
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
                let game_config_lock = {
                    let data_read = ctx.data.read().await;
                    data_read.get::<GameConfig>().unwrap().clone()
                };

                {
                    let mut game_config = game_config_lock.write().await;

                    if game_config.active {
                        post_md!("A game is already active!");
                        return;
                    }

                    game_config.active = true;
                    post_md!("{}", game_config.board);
                }
            }

            // Shows the current state of the board.
            Some("board") => {
                let game_config_lock = {
                    let data_read = ctx.data.read().await;
                    data_read.get::<GameConfig>().unwrap().clone()
                };

                {
                    let game_config = game_config_lock.read().await;
                    post_md!("{}", game_config.board);
                }
            }

            // Resets the game.
            Some("reset") => {
                let game_config_lock = {
                    let data_read = ctx.data.read().await;
                    data_read.get::<GameConfig>().unwrap().clone()
                };

                {
                    let mut game_config = game_config_lock.write().await;
                    game_config.reset();
                }

                post_md!("Reset succesful!");
            }

            // Any message that isn't a command. It might be a move in the game.
            _ => {
                let game_config_lock = {
                    let data_read = ctx.data.read().await;
                    data_read.get::<GameConfig>().unwrap().clone()
                };

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

#[tokio::main]
async fn main() {
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    // Create a new instance of the Client, logging in as a bot. This will
    // automatically prepend your bot token with "Bot ", which is a requirement
    // by Discord for bot users.
    let mut client = Client::builder(&token)
        .event_handler(GameHandler)
        .await
        .expect("Err creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<GameConfig>(Arc::new(RwLock::new(Default::default())));
    }

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform
    // exponential backoff until it reconnects.
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }

    /*
    let mut game = Game::new(vec![50; 10]);
    let mut buf;
    let mut line;
    let mut player = Player::default();

    for i in 1..100 {
        loop {
            print!("{}", game);

            buf = String::new();
            io::stdin().lock().read_line(&mut buf).unwrap();
            line = buf.trim_end();

            if let Err(err) = game.eval(&line, MAX_STEPS) {
                println!("Invalid move: {}", err);
            } else {
                break;
            }
        }

        if let Some(winners) = game.winners() {
            println!("{}\n{}", winners, game);
            return;
        }

        player.next(PLAYER_COUNT);
    } */
}
