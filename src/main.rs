mod stui;

use libschedule24::{data, image, Dimensions, RequestError, get_schema, get_schools, get_classes, print_lessons, get_class_guid, get_school_guid, get_lesson_info};
use std::{convert::TryInto, fs::File, io::Write};
use chrono::{Local, Datelike, NaiveTime};
use clap::{App, AppSettings, Arg, SubCommand, crate_authors, crate_description, crate_name, crate_version};

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    domain: String,
    school: String,
    class: String,
    cache: bool,
}

impl ::std::default::Default for Config {
    fn default() -> Self {
        Self {
            domain: String::new(),
            school: String::new(),
            class: String::new(),
            cache: false,
        }
    }
}

#[tokio::main]
async fn main() {
    if run_commands().await.is_ok() {}
}

async fn run_commands() -> Result<(), RequestError> {
    let cfg: Config = confy::load(env!("CARGO_PKG_NAME")).unwrap();

    let day_arg = Arg::with_name("day")
        .short("d")
        .long("day")
        .takes_value(true)
        .validator(|v| {
            if let Ok(n) = v.parse::<u8>() {
                if n > 5 {
                    Err("Day must be less than 6".to_string())
                } else {
                    Ok(())
                }
            } else {
                Err("Day must be a number".to_string())
            }
        })
        .help("Select what day to print");
    let week_arg = Arg::with_name("week")
        .short("w")
        .long("week")
        .takes_value(true)
        .validator(|v| {
            if let Err(_) = v.parse::<u8>() { Err("Day must be a number".to_string()) }
            else { Ok(()) }
        })
        .help("Select what day to print");

    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!("\n"))
        .about(crate_description!())
        .subcommand(SubCommand::with_name("test")
                    .about("Shows test message"))
        .subcommand(SubCommand::with_name("status")
                    .about("Prints current and next lesson. (Default)"))
        .subcommand(SubCommand::with_name("tui")
                    .about("Shows the terminal user interface")
                    .subcommand(SubCommand::with_name("week")
                        .about("Shows entire week")
                        .arg(Arg::with_name("week")
                            .short("w")
                            .long("week")
                            .takes_value(true)))
                    .subcommand(SubCommand::with_name("day")
                        .about("Only show single day")
                        .arg(&week_arg)
                        .arg(&day_arg)))
        .subcommand(SubCommand::with_name("lesson_info")
                    .about("Print the lesson info json")
                    .arg(&day_arg))
        .subcommand(SubCommand::with_name("svg")
                    .about("Generate SVG")
                    .arg(Arg::with_name("output")
                         .short("o")
                         .long("output")
                         .takes_value(true)
                         .default_value("-")
                         .help("Output file"))
                    .arg(Arg::with_name("resolution")
                         .short("r")
                         .long("resolution")
                         .takes_value(true)
                         .validator(|v| {
                             v.parse::<Dimensions>().map(|_| ()).map_err(|_| "Invalid resolution".to_string())
                         })
                         .default_value("1920x1080")
                         .help("Image resolution"))
                    .arg(&day_arg))
        .subcommand(SubCommand::with_name("list")
                    .about("List schools or classes")
                    .setting(AppSettings::SubcommandRequiredElseHelp)
                    .subcommand(SubCommand::with_name("classes")
                                .about("List classes"))
                    .subcommand(SubCommand::with_name("schools")
                                .about("List schools")))
        .arg(Arg::with_name("class")
                .short("c")
                .long("class")
                .takes_value(true)
                .default_value(&cfg.class)
                .help("Select class"))
        .arg(Arg::with_name("school")
                .short("s")
                .long("school")
                .takes_value(true)
                .default_value(&cfg.school)
                .help("Select school"))
        .arg(Arg::with_name("domain")
                .short("d")
                .long("domain")
                .takes_value(true)
                .default_value(&cfg.domain)
                .help("Select Skola24 domain"))
        .arg(Arg::with_name("no-cache")
                .long("no-cache")
                .takes_value(false)
                .help("Disable cachce"))
        .get_matches();

    let should_cache = if matches.is_present("no-cache") { false } else { cfg.cache };

    let selection = {
        let domain = matches.value_of("domain").unwrap();
        let school = get_school_guid(domain, matches.value_of("school").unwrap(), should_cache).await?;
        let class_guid = get_class_guid(domain, &school, matches.value_of("class").unwrap(), should_cache).await?;
        (
            domain.to_string(),
            school,
            class_guid
        )
    };

    if matches.subcommand_matches("test").is_some() {
        println!("Test, {}", get_school_guid(&cfg.domain, &cfg.class, should_cache).await?);
    }
    else if matches.subcommand_matches("status").is_some() {
        status(selection, should_cache).await?;
    }
    else if let Some(matches) = matches.subcommand_matches("lesson_info") {
        println!("{}", serde_json::to_string_pretty(
            &get_lesson_info(
                selection,
                matches.value_of("day").unwrap_or("0").parse()?,
                Local::now().iso_week().week() as i32,
                should_cache
            ).await?
        )?);
    }
    else if let Some(t_matches) = matches.subcommand_matches("tui") {
        if let Some(t_matches) = t_matches.subcommand_matches("week") {
            let week = if let Some(week) = t_matches.value_of("week") {
                week.parse::<i32>()?
            } else { Local::now().iso_week().week() as i32 };
            show_tui(selection, week, None, should_cache).await?;
        }
        else if let Some(t_matches) = t_matches.subcommand_matches("day") {
            let week = if let Some(week) = t_matches.value_of("week") {
                week.parse::<i32>()?
            } else { Local::now().iso_week().week() as i32 };
            let day = if let Some(day) = t_matches.value_of("day") {
                day.parse::<i32>().ok()
            } else { Some(0) };
            show_tui(selection, week, day, should_cache).await?;
        }
        else {
            show_tui(selection, Local::now().iso_week().week() as i32, Some(0), should_cache).await?;
        }
    }
    else if let Some(s_matches) = matches.subcommand_matches("svg") {
        create_svg(
            selection,
            s_matches.value_of("day").unwrap_or("0").parse().unwrap(),
            s_matches.value_of("resolution").unwrap().parse().unwrap(),
            s_matches.value_of("output").unwrap(),
            should_cache
        ).await?;
    }
    else if let Some(l_matches) = matches.subcommand_matches("list") {
        if l_matches.subcommand_matches("schools").is_some() {
            show_schools(selection, should_cache).await?;
        } else if l_matches.subcommand_matches("classes").is_some() {
            show_classes(selection, should_cache).await?;
        } else {
            unreachable!()
        }
    }
    else {
        status(selection, should_cache).await?;
    }

    Ok(())
}

async fn create_svg(selection: (String, String, String), day: i32, res: Dimensions, output: &str, should_cache: bool) -> Result<(), RequestError> {
    let week = Local::now().iso_week().week() as i32;
    let schema = get_schema(selection, day, week, Some(res), should_cache).await?.data;

    let doc = image::generate_svg(&schema, res)?.to_string();
    match output {
        "-"|"" => {
            std::io::stdout().write_all(doc.as_bytes())?;
        },
        path => {
            File::create(path)?.write_all(doc.as_bytes())?;
        },
    }

    Ok(())
}

async fn status(selection: (String, String, String), should_cache: bool) -> Result<(), RequestError> {
    let (lesson_info, next_day) = get_next_lesson_info(selection, should_cache).await?;
    print_lessons(&lesson_info[..], next_day)?;
    Ok(())
}

async fn show_tui(selection: (String, String, String), week: i32, day: Option<i32>, should_cache: bool) -> Result<(), RequestError> {
    let info = if let Some(day) = day {
        if day == 0 { vec!(get_next_lesson_info(selection, should_cache).await?.0) }
        else {
            let mut lesson_info: Vec<Vec<data::LessonInfo>> = Vec::new();
            let i = get_lesson_info(selection, day, week, should_cache).await?;
            lesson_info.push(i);
            lesson_info
        }
    }
    else { get_full_week(selection, week, should_cache).await?.to_vec() };

    match stui::run(&info[..]) {
        Ok(_) => Ok(()),
        Err(e) => {
            println!("Error while running stui: {}", e);
            Ok(())
        }
    }
}

async fn show_classes(selection: (String, String, String), should_cache: bool) -> Result<(), RequestError> {
    let classes = get_classes(&selection.0, &selection.1, should_cache).await?;
    for class in &classes {
        println!("{}", class.group_name);
    }
    Ok(())
}

async fn show_schools(selection: (String, String, String), should_cache: bool) -> Result<(), RequestError> {
    let schools = get_schools(&selection.0, should_cache).await?;
    for school in &schools {
        println!("{}", school.unit_id);
    }
    Ok(())
}

async fn get_next_lesson_info(selection: (String, String, String), should_cache: bool) -> Result<(Vec<data::LessonInfo>, bool), RequestError> {
    let now = Local::now();
    let mut day: i32 = now.weekday().number_from_monday().try_into().unwrap();
    let mut week: i32 = now.iso_week().week() as i32;

    let mut next_day = false;

    if day > 5 {
        next_day = true;
        day = 1;
        week += 1;
    }


    let lesson_info = if next_day {
        get_lesson_info(selection, day, week, should_cache).await?
    }
    else {
        let lesson_info = get_lesson_info(selection.clone(), day, week, should_cache).await?;
        let mut last_lesson = NaiveTime::from_hms(0,0,0);
        for lesson in lesson_info {
            let time = NaiveTime::parse_from_str(&lesson.time_end.to_string(), "%H:%M:%S").expect("Failed to parse time!");
            if time > last_lesson {
                last_lesson = time;
            }
        }

        if Local::now().time() > last_lesson {
            next_day = true;
            day += 1;
            if day > 5 {
                day = 1;
                week += 1;
            }
        }
        get_lesson_info(selection, day, week, should_cache).await?
    };

    Ok((lesson_info, next_day))
}

async fn get_full_week(selection: (String, String, String), week: i32, should_cache: bool) -> Result<[Vec<data::LessonInfo>; 5], RequestError> {
    let mut lesson_info: [Vec<data::LessonInfo>; 5] = Default::default();

    let next_lesson_info = &get_lesson_info(selection, 0, week, should_cache).await?;
    for i in 0..next_lesson_info.len() {
        lesson_info[next_lesson_info[i].day_of_week_number as usize - 1].push(next_lesson_info[i].clone());
    }

    Ok(lesson_info)
}
