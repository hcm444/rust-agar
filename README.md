# Agar.io Clone

A simple multiplayer Agar.io clone built with Rust, Actix, and WebSockets.

## Features

- Real-time multiplayer gameplay
- Player movement controlled by mouse
- Collect food to grow bigger
- Leaderboard showing top players
- Responsive design for various screen sizes
- Works on both HTTP and HTTPS

## Prerequisites

- Rust and Cargo (latest stable version)
- Web browser with WebSocket support

## How to Run

1. Clone this repository:
```
git clone <repository-url>
cd <repository-directory>
```

2. Build and run the server:
```
cargo run
```

3. Open your web browser and go to:
```
http://localhost:8080
```

## Setting up HTTPS (for production)

To run the server with HTTPS:

1. Generate SSL certificates using a tool like [mkcert](https://github.com/FiloSottile/mkcert) or obtain them from a certificate authority.

2. Modify the `main.rs` file to use SSL:
```rust
.bind_openssl("0.0.0.0:443", ssl_builder)?
```

3. Update the WebSocket connection in the frontend to use WSS instead of WS.

## How to Play

- Move your player by moving your mouse
- Collect the colored dots (food) to grow larger
- The larger you get, the higher you'll climb on the leaderboard
- Try to become the largest player in the game!

## Technologies Used

- Rust
- Actix Web Framework
- WebSockets
- HTML5 Canvas
- JavaScript

## License

MIT License

## Acknowledgments

Inspired by the original [Agar.io](https://agar.io/) game. 