use std::env;

use crate::game::*;

use serenity::prelude::*;

pub mod game;
pub mod play;

#[tokio::main]
async fn main() {
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    // Create a new instance of the Client, logging in as a bot. This will
    // automatically prepend your bot token with "Bot ", which is a requirement
    // by Discord for bot users.
    let mut client = Client::builder(&token)
        .event_handler(play::GameHandler)
        .await
        .expect("Err creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<play::GamesMap>(Default::default());
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
