use std::cmp::Ordering;
use std::collections::{HashMap, VecDeque};
use std::fmt::{Display, Formatter, Result as FmtResult, Write};
use std::ops::Index;
use std::slice::Iter;

/// Represents a player in the game.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Player(char);

impl Player {
    /// Initializes a new player with the given symbol.
    pub fn new(c: char) -> Self {
        Self(c)
    }
}

impl Display for Player {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        f.write_char(self.0)
    }
}

/// The list of players in the game, in cyclic order.
#[derive(Clone, Debug)]
pub struct Players(Vec<Player>);

impl Players {
    /// Initializes a new list of players.
    pub fn new(players: Vec<Player>) -> Self {
        Self(players)
    }

    /// Returns the number of players in the game.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns the index of the current player, based on the turn number.
    pub fn idx(&self, turn: usize) -> usize {
        turn % self.len()
    }
}

impl Default for Players {
    fn default() -> Self {
        Self::new(vec![Player::new('X'), Player::new('O')])
    }
}

impl Index<usize> for Players {
    type Output = Player;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

/// Represents the winners of a game.
#[derive(Default)]
pub struct Winners(Vec<Player>);

impl Index<usize> for Winners {
    type Output = Player;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl Display for Winners {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self.winner_count() {
            1 => write!(f, "Player {} won!", self[0]),
            2 => write!(f, "Players {} and {} tied!", self[0], self[1]),
            _ => {
                write!(f, "Players ")?;

                for player in self.iter().take(self.winner_count() - 1) {
                    write!(f, "{}, ", player)?;
                }

                write!(f, "and {} tied!", self.last().unwrap())
            }
        }
    }
}

impl Winners {
    /// Initializes a winner list.
    fn new(players: Vec<Player>) -> Self {
        Self(players)
    }

    /// Initializes a winner list with a single winner.
    fn single(player: Player) -> Self {
        Self::new(vec![player])
    }

    /// Returns the number of players that won.
    fn winner_count(&self) -> usize {
        self.0.len()
    }

    /// Pushes a player onto the winner list.
    fn push(&mut self, player: Player) {
        self.0.push(player)
    }

    /// Returns an iterator over the winners.
    fn iter(&self) -> Iter<Player> {
        self.0.iter()
    }

    /// Returns the last winner.
    fn last(&self) -> Option<Player> {
        self.0.last().copied()
    }
}

/// A command to be executed by the [`Game`].
#[derive(Clone, Copy)]
enum Command {
    /// Increments the value that's currently being pointed to.
    Increment,

    /// Decrements the value that's currently being pointed to.
    Decrement,

    /// Moves the data pointer left.
    MoveLeft,

    /// Moves the data pointer right.
    MoveRight,
}

/// Any of the possible errors while parsing and running a Brainfuck program.
#[derive(Clone, Copy, Debug)]
pub enum EvalError {
    /// A bucket's fill exceeded its capacity.
    Overflow {
        /// The index of the overflowed bucket.
        position: usize,
    },

    /// A bucket's fill became negative.
    Underflow {
        /// The index of the underflowed bucket.
        position: usize,
    },

    /// The position exceeded the number of buckets.
    OverBounds,

    /// The position became negative.
    UnderBounds,

    /// You attempted to add a counter to a locked bucket.
    LockedIncr {
        /// The position of the bucket.
        position: usize,
    },

    /// You attempted to remove a counter from a locked bucket.
    LockedDecr {
        /// The position of the bucket.
        position: usize,
    },

    /// A left bracket in the string does not have a matching right bracket.
    MismatchedLeft {
        /// The position of the bracket in the string.
        idx: usize,
    },

    /// A right bracket in the string does not have a matching left bracket.
    MismatchedRight {
        /// The position of the bracket in the string.
        idx: usize,
    },

    /// The computation went on for longer than allowed.
    MaxSteps,

    /// The string has an invalid character.
    InvalidChar {
        /// The invalid character.
        c: char,

        /// The index of the invalid character in the string.
        idx: usize,
    },

    /// The string is greater that can be at this specific turn.
    Length {
        /// The length of the string.
        len: usize,

        /// The current turn number, i.e. the maximal string length.
        turn: usize,
    },
}

impl Display for EvalError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match *self {
            Self::Overflow { position } => write!(
                f,
                "you attempted to add a counter to bucket {}, but it was full",
                position + 1
            ),

            Self::Underflow { position } => write!(
                f,
                "you attempted to remove a counter from bucket {}, but it was empty",
                position + 1
            ),

            Self::UnderBounds => {
                write!(f, "you attempted to move left past the first bucket")
            }

            Self::OverBounds => {
                write!(f, "you attempted to move right past the last bucket")
            }

            Self::MismatchedLeft { idx } => {
                write!(f, "mismatched left bracket at index {}", idx + 1)
            }

            Self::MismatchedRight { idx } => {
                write!(f, "mismatched right bracket at index {}", idx + 1)
            }

            Self::LockedIncr { position } => {
                write!(
                    f,
                    "you attempted to add a counter to bucket {}, but it was locked",
                    position + 1
                )
            }

            Self::LockedDecr { position } => {
                write!(
                    f,
                    "you attempted to remove a counter from bucket {}, but it was locked",
                    position + 1
                )
            }

            Self::MaxSteps => {
                write!(f, "computation exceeded maximum number of steps")
            }

            Self::InvalidChar { c, idx } => {
                write!(f, "invalid character {} at index {}", c, idx + 1)
            }

            Self::Length { len, turn } => write!(
                f,
                "move was {} characters, must be {} characters or less",
                len, turn
            ),
        }
    }
}

impl std::error::Error for EvalError {}

/// The result of evaluating a Brainfuck program.
pub type EvalResult<T> = Result<T, EvalError>;

/// Represents a bucket in the game.
#[derive(Debug)]
pub struct Bucket {
    /// The objects in the bucket, together with its capacity.
    pub counters: Vec<Player>,

    /// Whether the bucket is locked, i.e. filled with counters from a single player.
    pub locked: bool,
}

impl Clone for Bucket {
    fn clone(&self) -> Self {
        let mut data = Vec::with_capacity(self.capacity());
        data.clone_from(&self.counters);

        Self {
            counters: data,
            locked: self.locked,
        }
    }
}

impl Display for Bucket {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        for team in &self.counters {
            write!(f, "{}", team)?;
        }

        for _ in 0..self.free() {
            f.write_char('_')?;
        }

        write!(f, " {}/{}", self.fill(), self.capacity())?;
        if self.locked {
            f.write_str(" ✓")?;
        }

        Ok(())
    }
}

impl Bucket {
    /// Initializes a new, empty bucket with the specified capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            counters: Vec::with_capacity(capacity),
            locked: false,
        }
    }

    /// Empties the bucket.
    fn empty(&mut self) {
        *self = Self::new(self.capacity());
    }

    /// Returns the fill of the bucket.
    fn fill(&self) -> usize {
        self.counters.len()
    }

    /// Returns whether the bucket is empty.
    fn is_empty(&self) -> bool {
        self.fill() == 0
    }

    /// Returns the capacity of the bucket.
    fn capacity(&self) -> usize {
        self.counters.capacity()
    }

    /// Returns the amount of free spaces in the bucket.
    fn free(&self) -> usize {
        self.capacity() - self.fill()
    }

    /// Pushes the specified player's counter onto the bucket. Returns `true` if successful.
    fn push(&mut self, player: Player, position: usize) -> EvalResult<()> {
        match self.free() {
            0 => {
                return Err(if self.locked {
                    EvalError::LockedIncr { position }
                } else {
                    EvalError::Overflow { position }
                })
            }

            1 => {
                self.counters.push(player);

                for &counter in self.counters.iter() {
                    if counter != player {
                        return Ok(());
                    }
                }

                self.locked = true;
            }

            _ => {
                self.counters.push(player);
            }
        }

        Ok(())
    }

    /// Pops the last element from the bucket. Returns `true` if succesful.
    fn pop(&mut self, position: usize) -> EvalResult<()> {
        if self.is_empty() {
            Err(EvalError::Underflow { position })
        } else if self.locked {
            Err(EvalError::LockedDecr { position })
        } else {
            self.counters.pop().unwrap();
            Ok(())
        }
    }
}

/// One of the possible brainfuck instructions, after being parsed.
#[derive(Clone, Copy)]
enum BrainfuckToken {
    /// Execute a command, move the pointer to the right.
    Command { cmd: Command },

    /// Move the pointer to the target if the data that's being pointed to is zero.
    JumpIfZero { target: usize },

    /// Move the pointer to the target if the data that's being pointed to is non-zero.
    JumpIfNonzero { target: usize },
}

impl From<Command> for BrainfuckToken {
    fn from(cmd: Command) -> Self {
        Self::Command { cmd }
    }
}

/// Represents a Brainfuck program.
struct Brainfuck {
    /// The different tokens that make up the program.
    tokens: Vec<BrainfuckToken>,

    /// The data pointer, which represents the index of the token that's currently being read.
    pointer: usize,
}

impl Brainfuck {
    /// Tokenizes a string.
    fn new(str: &str) -> EvalResult<Self> {
        let mut queue = VecDeque::new();
        let mut tokens = Vec::new();

        // Iterates over non-whitespace characters.
        for (pos, c) in str.chars().filter(|c| !c.is_whitespace()).enumerate() {
            match c {
                '+' => {
                    tokens.push(Command::Increment.into());
                }

                '-' => {
                    tokens.push(Command::Decrement.into());
                }

                '<' => {
                    tokens.push(Command::MoveLeft.into());
                }

                '>' => {
                    tokens.push(Command::MoveRight.into());
                }

                '[' => {
                    tokens.push(BrainfuckToken::JumpIfZero { target: 0 });
                    queue.push_back(pos)
                }

                ']' => {
                    if let Some(target) = queue.pop_back() {
                        tokens.push(BrainfuckToken::JumpIfNonzero { target });

                        if let BrainfuckToken::JumpIfZero { target: old_target } =
                            &mut tokens[target]
                        {
                            *old_target = pos;
                        } else {
                            unreachable!()
                        }
                    } else {
                        return Err(EvalError::MismatchedRight { idx: pos });
                    }
                }

                _ => {
                    return Err(EvalError::InvalidChar { c, idx: pos });
                }
            }
        }

        if let Some(pos) = queue.pop_back() {
            Err(EvalError::MismatchedLeft { idx: pos })
        } else {
            Ok(Self { tokens, pointer: 0 })
        }
    }

    /// Returns the length of the program.
    fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Reads the token at the current position.
    fn read(&self) -> Option<BrainfuckToken> {
        self.tokens.get(self.pointer).copied()
    }

    /// Advances the data pointer.
    fn advance(&mut self) {
        self.pointer += 1;
    }

    /// Jumps the pointer to the target.
    fn jump(&mut self, target: usize) {
        self.pointer = target;
    }
}

/// Represents the memory Brainfuck runs on.
#[derive(Clone, Debug)]
pub struct GameBoard {
    /// The buckets, i.e. the different entries in the memory array.
    pub buckets: Vec<Bucket>,

    /// The index of the active bucket.
    pub position: usize,

    /// The turn number in the game.
    pub turn: usize,

    /// The player characters in the game, in cyclic order.
    pub players: Players,

    /// The number of buckets that can remain unfilled.
    pub buffer_buckets: u16,
}

impl Display for GameBoard {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        writeln!(f, "Turn {} -- {} to move", self.turn + 1, self.player())?;

        for (idx, bucket) in self.buckets.iter().enumerate() {
            if idx == self.position {
                write!(f, "> ")?;
            } else {
                write!(f, "  ")?;
            }

            writeln!(f, "{}", bucket)?;
        }

        Ok(())
    }
}

impl Default for GameBoard {
    fn default() -> Self {
        Self::new(vec![10; 5], 0)
    }
}

impl GameBoard {
    /// Initializes a new game with the specified buckets and the default settings.
    pub fn new(capacities: Vec<usize>, buffer_buckets: u16) -> Self {
        let mut buckets = Vec::new();

        for c in capacities {
            buckets.push(Bucket::new(c));
        }

        Self {
            buckets,
            position: 0,
            turn: 0,
            players: Default::default(),
            buffer_buckets,
        }
    }

    /// Resets the game state.
    pub fn reset(&mut self) {
        for bucket in &mut self.buckets {
            bucket.empty();
        }

        self.position = 0;
        self.turn = 0;
    }

    /// Resets the game, using the new specified capacities but keeping
    /// everything else the same.
    pub fn reset_with(&mut self, capacities: Vec<usize>) {
        self.buckets = Vec::new();

        for c in capacities {
            self.buckets.push(Bucket::new(c));
        }

        self.position = 0;
        self.turn = 0;
    }

    /// Returns a reference to the bucket that's being pointed at.
    fn bucket(&self) -> &Bucket {
        &self.buckets[self.position]
    }

    /// Returns a mutable reference to the bucket that's being pointed at.
    fn bucket_mut(&mut self) -> &mut Bucket {
        &mut self.buckets[self.position]
    }

    /// Returns the number of buckets.
    fn bucket_count(&self) -> usize {
        self.buckets.len()
    }

    fn iter(&self) -> Iter<Bucket> {
        self.buckets.iter()
    }

    /// Increments the current bucket.
    fn incr(&mut self) -> EvalResult<()> {
        let player = self.player();
        let position = self.position;
        self.bucket_mut().push(player, position)
    }

    /// Decrements the current bucket.
    fn decr(&mut self) -> EvalResult<()> {
        let position = self.position;
        self.bucket_mut().pop(position)
    }

    /// Moves the position to the left.
    fn move_left(&mut self) -> EvalResult<()> {
        if self.position == 0 {
            Err(EvalError::UnderBounds)
        } else {
            self.position -= 1;
            Ok(())
        }
    }

    /// Moves the position to the right.
    fn move_right(&mut self) -> EvalResult<()> {
        self.position += 1;
        if self.position == self.buckets.len() {
            Err(EvalError::OverBounds)
        } else {
            Ok(())
        }
    }

    /// Returns the index of the current player.
    pub fn player_idx(&self) -> usize {
        self.players.idx(self.turn)
    }

    /// Returns the current player.
    pub fn player(&self) -> Player {
        self.players[self.player_idx()]
    }

    /// Advances the turn number.
    fn next_turn(&mut self) {
        self.turn += 1;
    }

    /// Executes the specified [`Command`].
    fn exec(&mut self, cmd: Command) -> EvalResult<()> {
        match cmd {
            Command::Increment => self.incr(),
            Command::Decrement => self.decr(),
            Command::MoveLeft => self.move_left(),
            Command::MoveRight => self.move_right(),
        }
    }

    /// Runs a tokenized Brainfuck program for at most the specified amount of steps.
    fn run(&mut self, mut bf: Brainfuck, steps: u32) -> EvalResult<()> {
        let turn = self.turn + 1;

        if bf.len() > turn {
            return Err(EvalError::Length {
                len: bf.len(),
                turn,
            });
        }

        for _ in 0..steps {
            if let Some(instr) = bf.read() {
                match instr {
                    BrainfuckToken::Command { cmd } => {
                        self.exec(cmd)?;
                        bf.advance();
                    }

                    BrainfuckToken::JumpIfZero { target } => {
                        if self.bucket().is_empty() {
                            bf.jump(target);
                        } else {
                            bf.advance();
                        }
                    }

                    BrainfuckToken::JumpIfNonzero { target } => {
                        if !self.bucket().is_empty() {
                            bf.jump(target);
                        } else {
                            bf.advance();
                        }
                    }
                }
            } else {
                return Ok(());
            }
        }

        Err(EvalError::MaxSteps)
    }

    /// Evaluates a Brainfuck string, and runs it.
    pub fn eval(&mut self, str: &str, steps: u32) -> EvalResult<()> {
        let backup = self.clone();
        let res = self.run(Brainfuck::new(str)?, steps);

        if res.is_err() {
            *self = backup;
        } else {
            self.next_turn();
        }

        res
    }

    /// Returns the number of players in the game.
    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    /// Returns the number of locked buckets.
    pub fn locked_buckets(&self) -> usize {
        self.iter().filter(|b| b.locked).count()
    }

    /// Returns the number of buckets that must be filled in order to win.
    pub fn win_bucket_count(&self) -> u16 {
        self.bucket_count() as u16 - self.buffer_buckets
    }

    /// Returns the winners of the game.
    pub fn winners(&self) -> Option<Winners> {
        use std::collections::hash_map::Entry::*;

        let locked_buckets = self.locked_buckets() as u16;
        if locked_buckets < self.win_bucket_count() {
            return None;
        }

        let mut counts = HashMap::with_capacity(self.player_count());

        // Computes the number of buckets each player owns.
        for b in &self.buckets {
            match counts.entry(b.counters[0]) {
                Occupied(mut entry) => {
                    *entry.get_mut() += 1;
                }

                Vacant(entry) => {
                    entry.insert(1);
                }
            }
        }

        let mut max_count = 0;
        let mut winners = Winners::default();

        // Computes the players tied for the greatest amount of buckets.
        for (player, count) in counts.into_iter() {
            match count.cmp(&max_count) {
                Ordering::Equal => {
                    winners.push(player);
                }

                Ordering::Greater => {
                    max_count = count;
                    winners = Winners::single(player);
                }

                _ => {}
            }
        }

        Some(winners)
    }
}
