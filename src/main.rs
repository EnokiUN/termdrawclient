use std::{
    env,
    io::{stdin, stdout, Read, Write},
    time::Duration,
};

use anyhow::Context;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseButton,
        MouseEventKind,
    },
    execute,
    style::{Color, Print, ResetColor, SetBackgroundColor},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use futures::{SinkExt, StreamExt};
use termdrawserver::{ClientPayload, Pixel, PixelColour, ServerPayload};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |p| {
        execute!(
            stdout(),
            DisableMouseCapture,
            ResetColor,
            Clear(ClearType::All),
            Show
        )
        .unwrap();
        disable_raw_mode().unwrap();
        hook(p);
    }));
    let mut colour = PixelColour::White;

    let server_url = match env::args().nth(1) {
        Some(url) => url,
        None => {
            print!("Please supply a server URL to connect to: ");
            stdout().flush().context("Could not flush StdOut")?;
            let mut url = String::new();
            stdin()
                .read_line(&mut url)
                .context("Could not read from StdIn")?;
            url.trim().to_string()
        }
    };

    let (mut stream, _) = connect_async(&server_url)
        .await
        .with_context(|| format!("Could not conntect to server on {}", server_url))?;

    let room_id = {
        print!("Please enter the room id (leave empty to create a new one): ");
        stdout().flush().context("Could not flush StdOut")?;
        let mut id = String::new();
        stdin()
            .read_line(&mut id)
            .context("Could not read from StdIn")?;
        let id = id.trim().to_string();
        if id.is_empty() {
            None
        } else {
            Some(id)
        }
    };

    let room_id = match room_id {
        Some(id) => {
            let id = Uuid::parse_str(&id)
                .with_context(|| format!("Could not parse {} as a valid v4 Uuid", id))?;
            stream
                .send(Message::Text(
                    serde_json::to_string(&ClientPayload::JoinRoom(id)).unwrap(),
                ))
                .await
                .context("Could not send JoinRoom OPCode")?;
            id
        }
        None => {
            stream
                .send(Message::Text(
                    serde_json::to_string(&ClientPayload::CreateRoom).unwrap(),
                ))
                .await
                .context("Could not send CreateRoom OPCode")?;
            loop {
                if let Some(Ok(Message::Text(msg))) = stream.next().await {
                    if let Ok(payload) = serde_json::from_str::<ServerPayload>(&msg) {
                        match payload {
                            ServerPayload::NewRoom { room_id, .. } => {
                                println!("Your room id is {}, go put this somewhere (press enter to continue)", room_id);
                                stdin().read(&mut []).ok();
                                break room_id;
                            }
                            ServerPayload::RoomNotFound => println!("Unknown room, try again"),
                            _ => anyhow::bail!("Unexpected payload, bailing"),
                        }
                    }
                }
            }
        }
    };

    let (mut tx, mut rx) = stream.split();

    tokio::spawn(async move {
        while let Some(Ok(Message::Text(msg))) = rx.next().await {
            if let Ok(payload) = serde_json::from_str::<ServerPayload>(&msg) {
                match payload {
                    ServerPayload::Draw(pixel) => draw_pixel(&pixel),
                    ServerPayload::Reset => {
                        execute!(stdout(), ResetColor, Clear(ClearType::All)).unwrap()
                    }
                    _ => {}
                }
            }
        }
    });

    enable_raw_mode().unwrap();
    execute!(stdout(), EnableMouseCapture, Hide).unwrap();

    loop {
        if let Ok(true) = event::poll(Duration::from_millis(100)) {
            match event::read().unwrap() {
                Event::Mouse(evt) => match evt.kind {
                    MouseEventKind::Down(button) | MouseEventKind::Drag(button) => {
                        let pixel = Pixel {
                            x: evt.column as u32,
                            y: evt.row as u32,
                            colour: if button == MouseButton::Right {
                                PixelColour::Clear
                            } else {
                                colour.clone()
                            },
                        };
                        draw_pixel(&pixel);
                        tx.send(Message::Text(
                            serde_json::to_string(&ClientPayload::Draw(pixel)).unwrap(),
                        ))
                        .await
                        .context("Could not send Draw OPCode")?;
                    }
                    _ => {}
                },
                Event::Key(key) => match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        execute!(stdout(), ResetColor, Clear(ClearType::All)).unwrap();
                        tx.send(Message::Text(
                            serde_json::to_string(&ClientPayload::Reset).unwrap(),
                        ))
                        .await
                        .context("Could not send CreateRoom OPCode")?;
                    }
                    KeyCode::Char('1') => {
                        colour = PixelColour::White;
                    }
                    KeyCode::Char('2') if key.modifiers.contains(KeyModifiers::ALT) => {
                        colour = PixelColour::DarkRed;
                    }
                    KeyCode::Char('2') => {
                        colour = PixelColour::Red;
                    }
                    KeyCode::Char('3') if key.modifiers.contains(KeyModifiers::ALT) => {
                        colour = PixelColour::DarkBlue;
                    }
                    KeyCode::Char('3') => {
                        colour = PixelColour::Blue;
                    }
                    KeyCode::Char('4') if key.modifiers.contains(KeyModifiers::ALT) => {
                        colour = PixelColour::DarkGreen;
                    }
                    KeyCode::Char('4') => {
                        colour = PixelColour::Green;
                    }
                    KeyCode::Char('5') if key.modifiers.contains(KeyModifiers::ALT) => {
                        colour = PixelColour::DarkYellow;
                    }
                    KeyCode::Char('5') => {
                        colour = PixelColour::Yellow;
                    }
                    KeyCode::Char('6') if key.modifiers.contains(KeyModifiers::ALT) => {
                        colour = PixelColour::DarkMagenta;
                    }
                    KeyCode::Char('6') => {
                        colour = PixelColour::Magenta;
                    }
                    KeyCode::Char('7') if key.modifiers.contains(KeyModifiers::ALT) => {
                        colour = PixelColour::DarkGrey;
                    }
                    KeyCode::Char('7') => {
                        colour = PixelColour::Grey;
                    }
                    KeyCode::Char('8') => {
                        colour = PixelColour::Black;
                    }
                    _ => {}
                },

                _ => {}
            }
        }
        execute!(stdout(), MoveTo(0, 0), Print(room_id)).context("Could not write room id")?;
    }

    execute!(
        stdout(),
        DisableMouseCapture,
        ResetColor,
        Clear(ClearType::All),
        Show
    )
    .unwrap();
    disable_raw_mode().unwrap();

    Ok(())
}

fn draw_pixel(pixel: &Pixel) {
    let colour = match pixel.colour {
        PixelColour::Clear => Color::Reset,
        PixelColour::White => Color::White,
        PixelColour::DarkRed => Color::DarkRed,
        PixelColour::Red => Color::Red,
        PixelColour::DarkBlue => Color::DarkBlue,
        PixelColour::Blue => Color::Blue,
        PixelColour::DarkGreen => Color::DarkGreen,
        PixelColour::Green => Color::Green,
        PixelColour::DarkYellow => Color::DarkYellow,
        PixelColour::Yellow => Color::Yellow,
        PixelColour::DarkMagenta => Color::DarkMagenta,
        PixelColour::Magenta => Color::Magenta,
        PixelColour::DarkGrey => Color::DarkGrey,
        PixelColour::Grey => Color::Grey,
        PixelColour::Black => Color::Black,
    };
    execute!(
        stdout(),
        MoveTo(pixel.x as u16, pixel.y as u16),
        SetBackgroundColor(colour),
        Print(" "),
        ResetColor,
    )
    .expect("Could not draw pixel");
}
