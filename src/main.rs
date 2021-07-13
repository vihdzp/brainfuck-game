use std::env;

use game::GameBoard;
use play::{GameHandler, GamesMap};

use serenity::prelude::*;

mod game;
mod play;

const MAX_PLAYERS: u8 = 8;
const PLAYERS: [char; MAX_PLAYERS as usize] = ['X', 'O', 'Y', 'Z', 'A', 'B', 'C', 'D'];

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
        data.insert::<GamesMap>(Default::default());
    }

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform
    // exponential backoff until it reconnects.
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
