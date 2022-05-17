use std::{
    io,
    error::Error,
    thread,
    sync::mpsc,
    time::Duration,
};

use tui::{
    Terminal,
    //backend::TermionBackend,
    backend::CrosstermBackend,
    widgets::{Block, Borders, Paragraph, Wrap},
    layout::{Layout, Constraint, Direction, Rect},
    text::{Span, Spans},
    style::{Style, Color},
};

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, enable_raw_mode, disable_raw_mode},
    event::{read, KeyEvent, KeyCode},
};

use libschema::data;

/*use termion::{
    raw::IntoRawMode,
    input::TermRead,
    event::Key,
    screen::AlternateScreen,
};*/



use chrono::NaiveTime;
use substring::Substring;

enum Event<I> {
    Key(I),
    Tick
}

pub fn run(lesson_info: &[Vec<data::LessonInfo>]) -> Result<(), Box<dyn Error>> {
    //let stdout = io::stdout().into_raw_mode()?;
    //let stdout = AlternateScreen::from(stdout);
    execute!(io::stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    //let backend = TermionBackend::new(stdout);
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let (tx, rx) = mpsc::channel();

    spawn_threads(tx);

    let mut sorted_lessons: Vec<Vec<data::LessonInfo>> = Vec::new();
    for lesson in lesson_info {
        sorted_lessons.push(sort_lessons(lesson)?);
    }

    let mut first_lesson = NaiveTime::from_hms(23,59,59);
    let mut last_lesson = NaiveTime::from_hms(0,0,0);

    for lesson in &sorted_lessons {
        if lesson.is_empty() {continue;}
        let first_time = NaiveTime::parse_from_str(&lesson[0].time_start.to_string(), "%H:%M:%S").expect("Failed to parse time!");
        let last_time = NaiveTime::parse_from_str(&lesson[lesson.len() - 1].time_end.to_string(), "%H:%M:%S").expect("Failed to parse time!");
        if first_time < first_lesson {
            first_lesson = first_time;
        }
        if last_time > last_lesson {
            last_lesson = last_time;
        }
    }

    let mut lesson_constraints: Vec<Vec<Constraint>> = Vec::new();
    let mut same_time_lessons: Vec<Vec<i32>> = Vec::new();
    for lesson in &sorted_lessons {
        let (constraints, same_times) = generate_constraints(lesson, first_lesson, last_lesson)?;
        lesson_constraints.push(constraints);
        same_time_lessons.push(same_times);
    }

    loop {
        terminal.draw(|f| {
            let mut constraints: Vec<Constraint> = Vec::new();
            for _ in 0..sorted_lessons.len() {
                constraints.push(Constraint::Percentage(100 / sorted_lessons.len() as u16));
            }
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .margin(1)
                .constraints(constraints)
                .split(f.size());

            for j in 0..sorted_lessons.len() {
                let block = Block::default()
                    .borders(Borders::ALL);
                f.render_widget(block, chunks[j]);

                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints(&*lesson_constraints[j])
                    .split(chunks[j]);

                let mut skip_next = false;
                let mut skip_current = false;
                let mut prev_chunks: Vec<Rect> = Vec::new();

                let mut i = 1;
                let mut sorted_i = 0;
                while i < lesson_constraints[j].len() {
                    let chunks = if skip_current {
                        prev_chunks.clone()
                    }
                    else if same_time_lessons[j].contains(&(sorted_i as i32)) {
                        skip_next = true;
                        prev_chunks = Layout::default()
                            .direction(Direction::Horizontal)
                            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                            .split(chunks[i]).clone();
                        prev_chunks.clone()
                    }
                    else {
                        Layout::default()
                            .direction(Direction::Horizontal)
                            .constraints([Constraint::Percentage(100)])
                            .split(chunks[i])
                    };

                    let color = sorted_lessons[j][sorted_i].block.b_color.to_string();
                    let color = Color::Rgb(u8::from_str_radix(color.substring(1,3), 16).ok().unwrap(), u8::from_str_radix(color.substring(3,5), 16).ok().unwrap(), u8::from_str_radix(color.substring(5,7), 16).ok().unwrap());

                    let time = sorted_lessons[j][sorted_i].time_start.to_string();
                    let text = Spans::from(vec![
                        Span::raw("─"),
                        Span::styled(time.substring(0, 5), Style::default().fg(Color::Green)),
                        Span::raw(" - "),
                        Span::styled(sorted_lessons[j][sorted_i].texts[0].to_string(), Style::default().fg(color)),
                    ]);

                    let block = Block::default()
                        .title(text)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(color).bg(Color::Reset))
                        .style(Style::default().bg(color));
                    f.render_widget(block, chunks[skip_current as usize]);

                    if skip_current || skip_next {
                        let chunks = Layout::default()
                            .direction(Direction::Vertical)
                            .margin(1)
                            .constraints([
                                Constraint::Percentage(100),
                            ])
                            .split(chunks[skip_current as usize]);

                        let text = vec![
                            Spans::from(vec![
                                Span::raw(" "),
                                Span::styled(sorted_lessons[j][sorted_i].texts[0].to_string(), Style::default().bg(color).fg(Color::Black))
                            ]),
                            Spans::from(vec![
                                Span::raw(" "),
                                Span::styled(sorted_lessons[j][sorted_i].texts[2].to_string(), Style::default().bg(color).fg(Color::Black)),
                            ])
                        ];

                        let block = Paragraph::new(text).wrap(Wrap { trim: false });
                        f.render_widget(block, chunks[0]);
                    }

                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .margin(0)
                        .constraints([
                            Constraint::Percentage(95),
                            Constraint::Percentage(5),
                        ])
                        .split(chunks[skip_current as usize]);

                    let time = sorted_lessons[j][sorted_i].time_end.to_string();
                    let text = Spans::from(vec![
                        Span::raw("└─"),
                        Span::styled(time.substring(0, 5), Style::default().fg(Color::Red)),
                        Span::raw(" - "),
                        Span::styled(sorted_lessons[j][sorted_i].texts[2].to_string(), Style::default().fg(color)),
                    ]);
                    let block = Paragraph::new(text);
                    f.render_widget(block, chunks[1]);

                    if skip_next {
                        skip_current = true;
                        skip_next = false;
                    }
                    else {
                        skip_current = false;
                        i += 2;
                    }
                    sorted_i += 1;
                }
            }
        })?;

        let evt = rx.recv()?;
        if let Event::Key(key) = evt {
            if let KeyCode::Char('q') = key.code {
                disable_raw_mode()?;
                execute!(io::stdout(), LeaveAlternateScreen)?;
                break
            }
        }
    }

    Ok(())
}

fn sort_lessons(lesson_info: &[data::LessonInfo]) -> Result<Vec<data::LessonInfo>, Box<dyn Error>> {
    let mut sorted = Vec::new();
    let mut lessons = lesson_info.to_vec();

    for _ in 0..lesson_info.len() {
        let mut first = NaiveTime::from_hms(23,59,59);
        let mut first_val = 0;

        for (i, lesson) in lessons.iter().enumerate() {
            let time = NaiveTime::parse_from_str(&lesson.time_start.to_string(), "%H:%M:%S").expect("Failed to parse time!");
            if time < first {
                first = time;
                first_val = i;
            }
        }

        sorted.push(lessons.remove(first_val));
    }

    Ok(sorted)
}

fn generate_constraints(lesson_info: &[data::LessonInfo], first_lesson: NaiveTime, end_of_day: NaiveTime) -> Result<(Vec<Constraint>, Vec<i32>), Box<dyn Error>> {
    let mut constraints: Vec<Constraint> = Vec::new();
    let mut same_times: Vec<i32> = Vec::new();

    let mut entire_duration = 0;
    let mut last_lesson_end = first_lesson;
    let mut last_lesson_start: NaiveTime = NaiveTime::from_hms(0, 0, 0);

    let day = end_of_day.signed_duration_since(first_lesson).num_minutes();

    for (i, lesson) in lesson_info.iter().enumerate() {
        let time_start = NaiveTime::parse_from_str(&lesson.time_start.to_string(), "%H:%M:%S").expect("Failed to parse time!");
        let time_end = NaiveTime::parse_from_str(&lesson.time_end.to_string(), "%H:%M:%S").expect("Failed to parse time!");

        if (last_lesson_end == time_end || last_lesson_start == time_start) && i != 0 {
            same_times.push(i as i32 - 1);
            continue;
        }

        let duration = time_end.signed_duration_since(time_start).num_minutes();
        entire_duration += duration;
        let break_duration = time_start.signed_duration_since(last_lesson_end).num_minutes();
        entire_duration += break_duration;

        constraints.push(Constraint::Ratio((break_duration) as u32, day as u32));
        constraints.push(Constraint::Ratio((duration) as u32, day as u32));
        last_lesson_end = time_end;
        last_lesson_start = time_start;
    }

    if day - entire_duration > 0 {
        constraints.push(Constraint::Ratio((day - entire_duration) as u32, day as u32));
    }

    Ok((constraints, same_times))
}

fn spawn_threads(tx: mpsc::Sender<Event<KeyEvent>>) {
    {
        let tx = tx.clone();
        thread::spawn(move || loop {
            if let crossterm::event::Event::Key(key) = read().unwrap() {
                if let Err(err) = tx.send(Event::Key(key)) {
                    eprintln!("{}", err);
                    return;
                }
            }
        });
    }
    
    thread::spawn(move || loop {
        if let Err(err) = tx.send(Event::Tick) {
            eprintln!("{}", err);
            break;
        }
        thread::sleep(Duration::from_millis(1000));
    });
}
