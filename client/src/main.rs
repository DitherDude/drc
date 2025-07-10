use async_std::{
    sync::Mutex,
    task::{self},
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use std::{io, net::TcpStream, sync::Arc};
use tui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use utils::{receive_data, send_data};

#[async_std::main]
async fn main() -> Result<(), io::Error> {
    let para_text = Text::from(Spans::from(vec![
        Span::styled("Welcome to ", Style::default().fg(Color::White)),
        Span::styled("Dithers Relay Chat", Style::default().fg(Color::Magenta)),
        Span::styled(
            "!\nTo exit at any time, press ",
            Style::default().fg(Color::White),
        ),
        Span::styled("Ctrl", Style::default().fg(Color::Green)),
        Span::styled("+", Style::default().fg(Color::White)),
        Span::styled("C", Style::default().fg(Color::Red)),
        Span::styled(".\n", Style::default().fg(Color::White)),
    ]));
    let mut username = String::new();
    let mut stream = None;
    let para_text = Arc::new(Mutex::new(para_text));
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut input = Span::styled(String::new(), Style::default().fg(Color::White));
    let mut input_box_max_size = 100;
    let mut print_ready = true;
    loop {
        let mut para_text_lock = para_text.lock().await;
        if print_ready {
            if stream.is_none() {
                para_text_lock.lines.push(Spans::from(vec![
                    Span::styled(
                        "To get started, please type the ",
                        Style::default().fg(Color::White),
                    ),
                    Span::styled("IP address", Style::default().fg(Color::Blue)),
                    Span::styled(" of the ", Style::default().fg(Color::White)),
                    Span::styled("DRC", Style::default().fg(Color::Magenta)),
                    Span::styled(
                        " server you wish to connect to.\n",
                        Style::default().fg(Color::White),
                    ),
                ]))
            } else if username.is_empty() {
                para_text_lock.lines.push(Spans::from(vec![
                    Span::styled("Please type the ", Style::default().fg(Color::White)),
                    Span::styled("username", Style::default().fg(Color::Blue)),
                    Span::styled(" you wish to use.\n", Style::default().fg(Color::White)),
                ]))
            }
            print_ready = false;
        }
        terminal.draw(|f| {
            let size = f.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(5)].as_ref())
                .split(size);
            let text_output_height = chunks[0].height as usize - 2;
            while text_output_height < para_text_lock.lines.len() {
                para_text_lock.lines.remove(0);
            }
            let text_output = Paragraph::new(para_text_lock.clone())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Chat Messages"),
                )
                .style(Style::default().fg(Color::White))
                .wrap(Wrap { trim: true });
            let print = input.clone().content + "█";
            let input_field = Paragraph::new(Span::styled(print, input.style))
                .block(Block::default().borders(Borders::ALL).title("Input"))
                .style(Style::default().fg(Color::White))
                .wrap(Wrap { trim: true });
            f.render_widget(text_output, chunks[0]);
            f.render_widget(input_field, chunks[1]);
            input_box_max_size = (chunks[1].width - 2) * (chunks[1].height - 2);
        })?;

        if event::poll(std::time::Duration::from_millis(5))? {
            if let Ok(Event::Key(key)) = event::read() {
                let mut content = input.content.to_string();
                match key.code {
                    KeyCode::Char(c) => {
                        if key.modifiers.contains(event::KeyModifiers::CONTROL) && c == 'c' {
                            break;
                        }
                        if content.len() >= input_box_max_size.into() {
                            continue;
                        }
                        content.push(c)
                    }
                    KeyCode::Backspace => {
                        content.pop();
                    }
                    KeyCode::Enter => {
                        print_ready = true;
                        if stream.is_none() {
                            stream = match TcpStream::connect(&content) {
                                Ok(s) => {
                                    para_text_lock.lines.push(Spans::from(vec![
                                        Span::styled(
                                            "Connected",
                                            Style::default().fg(Color::Green),
                                        ),
                                        Span::styled(" to ", Style::default().fg(Color::White)),
                                        Span::styled(
                                            content.clone(),
                                            Style::default().fg(Color::Blue),
                                        ),
                                        Span::styled("!\n", Style::default().fg(Color::White)),
                                    ]));
                                    let para_text_clone_for_task = Arc::clone(&para_text); // Clone para_text again
                                    let s_clone = s.try_clone().unwrap();
                                    task::spawn(async move {
                                        loop {
                                            let line = receive_from_server(&s_clone);
                                            if line.is_some() {
                                                let (username, message) = line.unwrap();
                                                let mut para_text_lock =
                                                    para_text_clone_for_task.lock().await;
                                                para_text_lock.lines.push(Spans::from(vec![
                                                    Span::styled(
                                                        username,
                                                        Style::default().fg(Color::Blue),
                                                    ),
                                                    Span::styled(
                                                        ": ",
                                                        Style::default().fg(Color::White),
                                                    ),
                                                    Span::styled(
                                                        message,
                                                        Style::default().fg(Color::White),
                                                    ),
                                                ]));
                                            }
                                        }
                                    });
                                    Some(s)
                                }
                                Err(_) => {
                                    para_text_lock.lines.push(Spans::from(vec![
                                        Span::styled("Failed", Style::default().fg(Color::Red)),
                                        Span::styled(
                                            " to connect to ",
                                            Style::default().fg(Color::White),
                                        ),
                                        Span::styled(
                                            content.clone(),
                                            Style::default().fg(Color::Blue),
                                        ),
                                        Span::styled("!\n", Style::default().fg(Color::White)),
                                    ]));
                                    None
                                }
                            };
                        } else if username.is_empty() {
                            username = content.clone();
                            para_text_lock.lines.push(Spans::from(vec![
                                Span::styled("Username set to ", Style::default().fg(Color::White)),
                                Span::styled(username.clone(), Style::default().fg(Color::Blue)),
                                Span::styled(".", Style::default().fg(Color::White)),
                            ]))
                        } else {
                            send_to_server(&stream, &username, &content).await;
                            content.push('\n');
                            para_text_lock.lines.push(Spans::from(vec![
                                Span::styled("You: ", Style::default().fg(Color::Green)),
                                Span::styled(content.clone(), Style::default().fg(Color::White)),
                            ]));
                        }
                        content.clear();
                    }
                    _ => {}
                }
                input.style.fg = Some(Color::White);
                if content.len() == input_box_max_size.into() {
                    input.style.fg = Some(Color::Red);
                }
                input.content = content.into();
            }
        }
    }
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

async fn send_to_server(stream: &Option<TcpStream>, username: &str, message: &str) {
    let stream = match stream {
        Some(s) => s,
        None => return,
    };
    if username.is_empty() {
        return;
    }
    let mut payload = username.as_bytes().to_vec();
    payload.push(0);
    payload.extend_from_slice(message.as_bytes());
    send_data(&payload, stream);
}

fn receive_from_server(stream: &TcpStream) -> Option<(String, String)> {
    let messageraw = receive_data(stream);
    let idx = messageraw.iter().position(|&c| c == 0).unwrap();
    let (username, message) = messageraw.split_at(idx);
    if username.is_empty() || message.is_empty() {
        return None;
    }
    Some((
        String::from_utf8_lossy(username).to_string(),
        String::from_utf8_lossy(message).to_string(),
    ))
}
