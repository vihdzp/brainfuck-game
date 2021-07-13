use std::cmp::Ordering;
use std::collections::VecDeque;
use std::fmt::{Display, Formatter, Result as FmtResult, Write};

/// Represents a player in the game.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Player(u8);

impl Player {
    /// Initializes a new player with the given index.
    pub fn new(idx: u8) -> Self {
        Self(idx)
    }

    pub fn idx(&self) -> usize {
        self.0 as usize
    }

    /// Goes to the next player in turn.
    pub fn next(&mut self, player_count: u8) {
        self.0 += 1;

        if self.0 == player_count {
            self.0 = 0;
        }
    }
}

impl Display for Player {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_char(crate::PLAYERS[self.idx()])
    }
}

/// Represents the winners of a game.
pub struct Winners(Vec<Player>);

impl Display for Winners {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.winner_count() {
            1 => write!(f, "Player {} won!", self.0[0]),
            2 => write!(f, "Players {} and {} tied!", self.0[0], self.0[1]),
            _ => {
                write!(f, "Players ")?;

                for player in self.0.iter().take(self.winner_count() - 1) {
                    write!(f, "{}, ", player)?;
                }

                write!(f, "and {} tied!", self.0.last().unwrap())
            }
        }
    }
}

impl Winners {
    /// Returns the number of players that won.
    fn winner_count(&self) -> usize {
        self.0.len()
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
    LockedIncr { position: usize },

    /// You attempted to remove a counter from a locked bucket.
    LockedDecr { position: usize },

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
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
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
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        for team in &self.counters {
            write!(f, "{}", team)?;
        }

        for _ in 0..self.free() {
            f.write_char('_')?;
        }

        write!(f, " {}/{} ", self.fill(), self.capacity())?;
        if self.locked {
            f.write_char('✓')?;
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
                for &counter in self.counters.iter() {
                    if counter != player {
                        return Ok(());
                    }
                }

                self.locked = true;
            }

            _ => {}
        }

        self.counters.push(player);
        Ok(())
    }

    /// Pops the last element from the bucket. Returns `true` if succesful.
    fn pop(&mut self, position: usize) -> EvalResult<()> {
        if self.is_empty() {
            Err(EvalError::Underflow { position })
        } else if self.locked {
            Err(EvalError::LockedDecr { position })
        } else {
            self.counters.pop();
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

        for (pos, c) in str.chars().enumerate() {
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

                _ => return Err(EvalError::InvalidChar { c, idx: pos }),
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

    /// The number of buckets that have been filled.
    pub filled_buckets: usize,

    /// The turn number in the game.
    pub turn: usize,

    /// The player to move in the game.
    pub player: Player,
}

impl Display for GameBoard {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        writeln!(f, "Turn {} -- {} to move", self.turn, self.player)?;

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
        Self::new(vec![10; 5])
    }
}

impl GameBoard {
    /// Initializes a new game with the specified buckets.
    pub fn new(capacities: Vec<usize>) -> Self {
        let mut buckets = Vec::new();

        for c in capacities {
            buckets.push(Bucket::new(c));
        }

        Self {
            buckets,
            position: 0,
            filled_buckets: 0,
            turn: 1,
            player: Player::default(),
        }
    }

    /// Resets the game state.
    pub fn reset(&mut self) {
        for bucket in &mut self.buckets {
            bucket.empty();
        }

        self.position = 0;
        self.turn = 1;
        self.player = Player::default();
        self.filled_buckets = 0;
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

    /// Increments the current bucket.
    fn incr(&mut self) -> EvalResult<()> {
        let player = self.player;
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

    /// Advances the turn number, goes to the next player.
    fn next_turn(&mut self, player_count: u8) {
        self.turn += 1;
        self.player.next(player_count);
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
        if bf.len() > self.turn {
            return Err(EvalError::Length {
                len: bf.len(),
                turn: self.turn,
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
    pub fn eval(&mut self, str: &str, steps: u32, player_count: u8) -> EvalResult<()> {
        let backup = self.clone();
        let res = self.run(Brainfuck::new(str)?, steps);

        if res.is_err() {
            *self = backup;
        } else {
            self.next_turn(player_count);
        }

        res
    }

    /// Returns the winners of the game.
    pub fn winners(&self, player_count: u8) -> Option<Winners> {
        if self.filled_buckets != self.bucket_count() {
            return None;
        }

        let mut counts = vec![0; player_count as usize];

        for b in &self.buckets {
            let player = b.counters[0];
            counts[player.idx()] += 1;
        }

        let mut max_count = 0;
        let mut winners = Vec::new();

        for (idx, count) in counts.into_iter().enumerate() {
            let player = Player::new(idx as u8);

            match count.cmp(&max_count) {
                Ordering::Equal => {
                    winners.push(player);
                }

                Ordering::Greater => {
                    max_count = count;
                    winners = vec![player];
                }

                _ => {}
            }
        }

        Some(Winners(winners))
    }
}
