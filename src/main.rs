use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Parser, Subcommand};
use eyre::{eyre, Result};
use qmk_via_api::api::KeyboardApi;
use qmk_via_api::keycodes::Keycode;

const VID: u16 = 0x3434;
const PID: u16 = 0x0960;
// const PID: u16 = 0xd030;
const USAGE_PAGE: u16 = 0xff60;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    GetMacroSize,
    GetMacros(GetMacros),
    SetMacros(SetMacros),
    SwapMacros(SwapMacros),
    Test,
    GetMicState,
    SetMicState(SetMicState),
}

#[derive(Args)]
struct GetMacros {
    #[arg(short, long)]
    file: Option<PathBuf>,

    #[arg(short, long)]
    pretty: bool,

    #[arg(short, long)]
    json: bool,
}

#[derive(Args)]
struct SetMacros {
    #[arg(short, long)]
    file: Option<PathBuf>,
}

#[derive(Args)]
struct SwapMacros {
    #[arg(short, long)]
    from: usize,

    #[arg(short, long)]
    to: usize,

    #[arg(short, long)]
    print: bool,
}

#[derive(Args)]
struct SetMicState {
    #[arg()]
    state: u8,
}

fn macro_to_string(bytes: &[u8]) -> String {
    let mut fix = false;
    bytes
        .split(|byte| byte == &0x01)
        .skip(1)
        .map(|chunk| {
            if chunk.is_empty() {
                fix = true;
                return String::new();
            }
            let chunk = if fix {
                fix = false;
                let one = [1u8].as_slice();
                [one, chunk].concat()
            } else {
                chunk.to_vec()
            };
            let mut result = String::from("{");
            result += match chunk[0] {
                1 | 4 => "",
                2 => "+",
                3 => "-",
                _ => unreachable!(),
            };

            if chunk[0] == 4 {
                result += &String::from_utf8_lossy(&chunk[1..]);
                result = result.replace("|", "}");
            } else {
                let key = Keycode::try_from(chunk[1] as u16).unwrap();
                result += &format!("{key:?}}}{}", String::from_utf8_lossy(&chunk[2..]));
            }

            result
        })
        .collect()
}

fn prettify(api: &KeyboardApi, json: bool, bytes: &[u8]) -> Result<Vec<u8>> {
    let macro_count = api
        .get_macro_count()
        .ok_or(eyre!("failed to fetched macro count"))?;
    let macros: Vec<String> = bytes
        .split(|byte| byte == &0x00)
        .take(macro_count as usize)
        .map(macro_to_string)
        .collect();
    if json {
        Ok(serde_json::to_vec_pretty(&macros)?)
    } else {
        Ok(macros
            .iter()
            .enumerate()
            .fold(String::new(), |result, (i, m)| {
                result + &format!("{i:>2}. {m}\n")
            })
            .into_bytes())
    }
}

fn get_macro_size(api: &KeyboardApi) -> Result<()> {
    let bytes = api
        .get_macro_bytes()
        .ok_or(eyre!("failed to fetch macro bytes"))?;

    let macro_count = api
        .get_macro_count()
        .ok_or(eyre!("failed to fetched macro count"))?;

    let used_size = bytes
        .split(|byte| byte == &0x00)
        .take(macro_count as usize)
        .flatten()
        .count();

    println!("macro count: {macro_count}");
    println!(
        "macro size:  {used_size} / {} B ({:.1}%)",
        bytes.len(),
        used_size as f32 / bytes.len() as f32 * 100.0
    );

    Ok(())
}

fn get_macros(api: &KeyboardApi, args: GetMacros) -> Result<()> {
    let mut bytes = api
        .get_macro_bytes()
        .ok_or(eyre!("failed to fetch macro bytes"))?;

    let mut writer: Box<dyn Write> = match args.file {
        Some(path) => Box::new(std::fs::File::create(path)?),
        None => Box::new(std::io::stdout().lock()),
    };

    if args.pretty {
        bytes = prettify(api, args.json, &bytes)?;
    }

    writer.write_all(&bytes)?;
    Ok(())
}

fn set_macros(api: &KeyboardApi, args: SetMacros) -> Result<()> {
    let mut reader: Box<dyn Read> = match args.file {
        Some(path) => Box::new(std::fs::File::open(path)?),
        None => Box::new(std::io::stdin().lock()),
    };
    let mut bytes = api
        .get_macro_bytes()
        .ok_or(eyre!("failed to fetch macro bytes"))?;
    reader.read_exact(&mut bytes)?;
    api.set_macro_bytes(bytes)
        .ok_or(eyre!("failed to send macro bytes"))?;
    Ok(())
}

fn swap_macros(api: &KeyboardApi, args: SwapMacros) -> Result<()> {
    let bytes = api
        .get_macro_bytes()
        .ok_or(eyre!("failed to fetch macro bytes"))?;

    let macro_count = api
        .get_macro_count()
        .ok_or(eyre!("failed to fetched macro count"))?;

    let mut macros: Vec<&[u8]> = bytes
        .split(|byte| byte == &0x00)
        .take(macro_count as usize)
        .collect();
    macros.swap(args.from, args.to);

    let bytes = macros.join(&0x00);

    if args.print {
        let mut writer = std::io::stdout().lock();
        let pretty_bytes = prettify(api, false, &bytes)?;
        writer.write_all(&pretty_bytes)?;
    }

    api.set_macro_bytes(bytes)
        .ok_or(eyre!("failed to send macro bytes"))?;
    Ok(())
}

fn test(api: &KeyboardApi) {
    for i in 21..=30 {
        println!("sending {i}");
        api.set_custom_menu_value(vec![0, 3, i]);
        std::thread::sleep(Duration::from_secs_f32(0.5));
    }
}

fn get_mic_state(api: &KeyboardApi) {
    api.hid_send(vec![8, 0, 4]);
    println!("{}", api.hid_read().unwrap()[3]);
}

fn set_mic_state(api: &KeyboardApi, args: SetMicState) {
    api.set_custom_menu_value(vec![0, 4, args.state]);
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let api = KeyboardApi::new(VID, PID, USAGE_PAGE)?;

    match cli.command {
        Command::GetMacroSize => get_macro_size(&api)?,
        Command::GetMacros(args) => get_macros(&api, args)?,
        Command::SetMacros(args) => set_macros(&api, args)?,
        Command::SwapMacros(args) => swap_macros(&api, args)?,
        Command::Test => test(&api),
        Command::GetMicState => get_mic_state(&api),
        Command::SetMicState(args) => set_mic_state(&api, args),
    }

    Ok(())
}
