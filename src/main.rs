use actix::{Actor, ActorContext, Addr, AsyncContext, Handler, Message, StreamHandler, WrapFuture, ActorFutureExt, ContextFutureSpawner};
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web_actors::ws;
use actix_files as fs;
use log::{info, warn};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

// Game settings
const MAX_FOOD_COUNT: usize = 100;
const PLAYER_START_SIZE: f32 = 20.0;
const WORLD_WIDTH: f32 = 3000.0;
const WORLD_HEIGHT: f32 = 3000.0;
const FOOD_SIZE: f32 = 10.0;
const FOOD_VALUE: f32 = 1.0;

// Types
type ClientId = String;

// Game state
struct AppState {
    lobby: Addr<Lobby>,
}

// Shared game state and logic
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Player {
    id: ClientId,
    x: f32,
    y: f32,
    size: f32,
    color: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Food {
    id: String,
    x: f32,
    y: f32,
    color: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GameState {
    players: HashMap<ClientId, Player>,
    food: HashMap<String, Food>,
}

impl GameState {
    fn new() -> Self {
        let mut game_state = GameState {
            players: HashMap::new(),
            food: HashMap::new(),
        };
        
        game_state.spawn_food();
        game_state
    }
    
    fn spawn_food(&mut self) {
        let mut rng = rand::thread_rng();
        
        while self.food.len() < MAX_FOOD_COUNT {
            let id = Uuid::new_v4().to_string();
            let x = rng.gen_range(0.0..WORLD_WIDTH);
            let y = rng.gen_range(0.0..WORLD_HEIGHT);
            
            // Generate random hex color
            let color = format!(
                "#{:02x}{:02x}{:02x}", 
                rng.gen_range(0..255), 
                rng.gen_range(0..255), 
                rng.gen_range(0..255)
            );
            
            let food = Food { id: id.clone(), x, y, color };
            self.food.insert(id, food);
        }
    }
    
    // Check for player collisions and determine if one player can eat another
    fn check_player_collisions(&mut self) -> Vec<(ClientId, ClientId)> {
        let mut eaten_players = Vec::new();
        let player_ids: Vec<ClientId> = self.players.keys().cloned().collect();
        
        // Compare each player with every other player
        for i in 0..player_ids.len() {
            let player1_id = &player_ids[i];
            
            // Skip if player was already eaten
            if !self.players.contains_key(player1_id) {
                continue;
            }
            
            for j in 0..player_ids.len() {
                if i == j {
                    continue; // Skip self
                }
                
                let player2_id = &player_ids[j];
                
                // Skip if player was already eaten
                if !self.players.contains_key(player2_id) {
                    continue;
                }
                
                // Check if players can eat each other (larger eats smaller)
                let can_eat = {
                    // Create a copy of player positions and sizes to avoid borrow issues
                    let player1 = self.players.get(player1_id).unwrap().clone();
                    let player2 = self.players.get(player2_id).unwrap().clone();
                    
                    // Calculate distance between players
                    let dx = player1.x - player2.x;
                    let dy = player1.y - player2.y;
                    let distance_squared = dx * dx + dy * dy;
                    
                    // Radius of the larger player
                    let radius_diff = player1.size - player2.size;
                    
                    // Player 1 can eat player 2 if:
                    // 1. Player 1 is at least 20% larger than player 2
                    // 2. Players are overlapping
                    if radius_diff > player2.size * 0.2 && distance_squared < player1.size * player1.size {
                        // Player 1 eats player 2
                        Some((player1_id.clone(), player2_id.clone()))
                    } else if -radius_diff > player1.size * 0.2 && distance_squared < player2.size * player2.size {
                        // Player 2 eats player 1
                        Some((player2_id.clone(), player1_id.clone()))
                    } else {
                        None
                    }
                };
                
                if let Some(eaten) = can_eat {
                    eaten_players.push(eaten);
                }
            }
        }
        
        // Process the eaten players
        for (eater_id, eaten_id) in &eaten_players {
            let eaten_size = if let Some(eaten) = self.players.get(eaten_id) {
                eaten.size
            } else {
                0.0
            };
            
            if let Some(eater) = self.players.get_mut(eater_id) {
                // Increase size of eater based on the size of the eaten player
                eater.size += eaten_size * 0.5;
            }
            
            // Remove the eaten player
            self.players.remove(eaten_id);
        }
        
        eaten_players
    }
}

// Actor that manages the game state
struct Lobby {
    sessions: HashMap<ClientId, Addr<GameSession>>,
    game_state: Arc<Mutex<GameState>>,
}

impl Actor for Lobby {
    type Context = actix::Context<Self>;
    
    fn started(&mut self, ctx: &mut Self::Context) {
        // Start game loop
        ctx.run_interval(Duration::from_millis(50), |actor, _ctx| {
            // Check for player collisions
            let eaten_players = {
                let mut game_state = actor.game_state.lock().unwrap();
                game_state.check_player_collisions()
            };
            
            // Notify players that were eaten
            for (_, eaten_id) in eaten_players {
                if let Some(addr) = actor.sessions.remove(&eaten_id) {
                    // Send game over message
                    addr.do_send(GameOverMessage);
                }
            }
            
            // Send updated game state to all remaining players
            actor.send_game_state_to_all();
        });
    }
}

impl Lobby {
    fn new() -> Self {
        Lobby {
            sessions: HashMap::new(),
            game_state: Arc::new(Mutex::new(GameState::new())),
        }
    }
    
    fn send_game_state_to_all(&self) {
        if self.sessions.is_empty() {
            return;
        }
        
        // Create a copy of the game state to send to clients
        let game_state = self.game_state.lock().unwrap().clone();
        
        // Serialize the game state
        if let Ok(game_state_json) = serde_json::to_string(&game_state) {
            // Send to all connected clients
            for addr in self.sessions.values() {
                addr.do_send(GameStateMessage(game_state_json.clone()));
            }
        }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct Connect {
    id: ClientId,
    addr: Addr<GameSession>,
}

#[derive(Message)]
#[rtype(result = "()")]
struct Disconnect {
    id: ClientId,
}

#[derive(Message)]
#[rtype(result = "()")]
struct PlayerMove {
    id: ClientId,
    x: f32,
    y: f32,
}

#[derive(Message)]
#[rtype(result = "()")]
struct GameStateMessage(String);

#[derive(Message)]
#[rtype(result = "()")]
struct GameOverMessage;

impl Handler<Connect> for Lobby {
    type Result = ();
    
    fn handle(&mut self, msg: Connect, _: &mut Self::Context) -> Self::Result {
        info!("Client connected: {}", msg.id);
        
        // Create a new player
        let mut game_state = self.game_state.lock().unwrap();
        
        let mut rng = rand::thread_rng();
        let player = Player {
            id: msg.id.clone(),
            x: rng.gen_range(100.0..WORLD_WIDTH - 100.0),
            y: rng.gen_range(100.0..WORLD_HEIGHT - 100.0),
            size: PLAYER_START_SIZE,
            color: format!(
                "#{:02x}{:02x}{:02x}",
                rng.gen_range(0..255),
                rng.gen_range(0..255),
                rng.gen_range(0..255)
            ),
        };
        
        game_state.players.insert(msg.id.clone(), player);
        
        // Add the connection to the sessions map
        self.sessions.insert(msg.id, msg.addr);
    }
}

impl Handler<Disconnect> for Lobby {
    type Result = ();
    
    fn handle(&mut self, msg: Disconnect, _: &mut Self::Context) -> Self::Result {
        info!("Client disconnected: {}", msg.id);
        
        // Remove from sessions map
        self.sessions.remove(&msg.id);
        
        // Remove player from game state
        let mut game_state = self.game_state.lock().unwrap();
        game_state.players.remove(&msg.id);
    }
}

impl Handler<PlayerMove> for Lobby {
    type Result = ();
    
    fn handle(&mut self, msg: PlayerMove, _: &mut Self::Context) -> Self::Result {
        let mut game_state = self.game_state.lock().unwrap();
        
        // Clone the food HashMap to avoid borrow checker issues
        let food_clone = game_state.food.clone();
        
        if let Some(player) = game_state.players.get_mut(&msg.id) {
            // Apply speed limitation based on player size (larger players move slower)
            let speed_factor = 1.0 / (1.0 + player.size / 100.0);
            let dx = msg.x - player.x;
            let dy = msg.y - player.y;
            let distance = (dx * dx + dy * dy).sqrt();
            
            if distance > 0.0 {
                let max_move = 5.0 * speed_factor;
                let move_factor = if distance > max_move { max_move / distance } else { 1.0 };
                
                player.x += dx * move_factor;
                player.y += dy * move_factor;
            }
            
            // Keep player within world bounds
            player.x = player.x.max(player.size).min(WORLD_WIDTH - player.size);
            player.y = player.y.max(player.size).min(WORLD_HEIGHT - player.size);
            
            // Check for collisions with food
            let mut food_to_remove = Vec::new();
            let player_size_squared = player.size * player.size;
            
            // Make a copy of player position to avoid borrow checker issues
            let player_x = player.x;
            let player_y = player.y;
            
            // Now we don't need to access game_state.food directly
            for (food_id, food) in food_clone.iter() {
                let dx = player_x - food.x;
                let dy = player_y - food.y;
                let distance_squared = dx * dx + dy * dy;
                
                // If player overlaps with food
                if distance_squared < player_size_squared {
                    food_to_remove.push(food_id.clone());
                    player.size += FOOD_VALUE;
                }
            }
            
            // Remove eaten food
            for food_id in food_to_remove {
                game_state.food.remove(&food_id);
            }
            
            // Spawn new food if needed
            game_state.spawn_food();
        }
    }
}

// WebSocket session for each player
struct GameSession {
    id: ClientId,
    lobby_addr: Addr<Lobby>,
    heartbeat: Instant,
}

impl GameSession {
    fn new(lobby_addr: Addr<Lobby>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            lobby_addr,
            heartbeat: Instant::now(),
        }
    }
    
    fn heartbeat(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(Duration::from_secs(5), |act, ctx| {
            if Instant::now().duration_since(act.heartbeat) > Duration::from_secs(10) {
                // Client hasn't responded to heartbeat for too long
                warn!("Websocket client timed out: {}", act.id);
                act.lobby_addr.do_send(Disconnect {
                    id: act.id.clone(),
                });
                ctx.stop();
                return;
            }
            
            ctx.ping(b"");
        });
    }
}

impl Actor for GameSession {
    type Context = ws::WebsocketContext<Self>;
    
    fn started(&mut self, ctx: &mut Self::Context) {
        self.heartbeat(ctx);
        
        // Register with the lobby
        let addr = ctx.address();
        self.lobby_addr
            .send(Connect {
                id: self.id.clone(),
                addr,
            })
            .into_actor(self)
            .then(|_, _, _| actix::fut::ready(()))
            .wait(ctx);
    }
    
    fn stopping(&mut self, _: &mut Self::Context) -> actix::Running {
        // Notify lobby about disconnect
        self.lobby_addr.do_send(Disconnect {
            id: self.id.clone(),
        });
        actix::Running::Stop
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for GameSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.heartbeat = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.heartbeat = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                // Parse player movement
                if let Ok(movement) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let (Some(x), Some(y)) = (movement.get("x"), movement.get("y")) {
                        if let (Some(x), Some(y)) = (x.as_f64(), y.as_f64()) {
                            self.lobby_addr.do_send(PlayerMove {
                                id: self.id.clone(),
                                x: x as f32,
                                y: y as f32,
                            });
                        }
                    }
                }
            }
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => {}
        }
    }
}

impl Handler<GameStateMessage> for GameSession {
    type Result = ();
    
    fn handle(&mut self, msg: GameStateMessage, ctx: &mut Self::Context) -> Self::Result {
        ctx.text(msg.0);
    }
}

impl Handler<GameOverMessage> for GameSession {
    type Result = ();
    
    fn handle(&mut self, _: GameOverMessage, ctx: &mut Self::Context) -> Self::Result {
        // Send game over message to client
        ctx.text(r#"{"type":"game_over"}"#);
        
        // Close the connection
        ctx.stop();
    }
}

// Web handlers
async fn index() -> impl Responder {
    fs::NamedFile::open_async("./static/index.html").await.unwrap()
}

async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let game_session = GameSession::new(app_state.lobby.clone());
    ws::start(game_session, &req, stream)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    
    // Create the game lobby
    let lobby = Lobby::new().start();
    
    // Create the web server
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(AppState {
                lobby: lobby.clone(),
            }))
            .route("/", web::get().to(index))
            .route("/ws", web::get().to(ws_handler))
            .service(fs::Files::new("/static", "./static").show_files_listing())
    })
    .bind("127.0.0.1:8080")?
    .run();
    
    info!("Server started at http://127.0.0.1:8080");
    server.await
}
