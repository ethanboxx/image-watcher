pub mod error_change;

use error_change::ChangeError;
use read_input::prelude::*;
use std::{
    convert::TryFrom, fs::File, io::prelude::*, iter::Iterator, path::Path, str::FromStr, thread,
    time, time::SystemTime,
};
use yaml_rust::{Yaml, YamlLoader};

#[derive(Debug, Clone)]
struct FileWatched {
    file: FileWatch,
    time: Option<SystemTime>,
}
#[derive(Debug, Clone)]
struct FileWatch {
    path: String,
    output: String,
    size: Size,
}
#[derive(Debug, Clone)]
enum Size {
    Width(u32),
    Height(u32),
    WidthHeight(u32, u32),
}
#[derive(Debug)]
enum Mode {
    Compile,
    Watch,
}

fn main() {
    let mode = mode();
    println!("Parsing config file image_watcher.yaml");
    let files_list = match parse_config() {
        Ok(x) => x,
        Err(e) => {
            println!("Error: {}", e);
            return;
        }
    };

    let mut files_list: Vec<FileWatched> = files_list
        .into_iter()
        .map(|x| FileWatched {
            file: x.clone(),
            time: None,
        })
        .collect();
    loop {
        for (index, file) in files_list.clone().iter().enumerate() {
            let modified = match when_modified(Path::new(&file.file.path)) {
                Ok(s) => s,
                Err(_) => return,
            };
            match file.time {
                Some(last) => {
                    if last != modified {
                        files_list[index].time = Some(modified);
                        resize_image(&file.file.path, &file.file.output, &file.file.size).unwrap()
                    };
                }
                None => {
                    files_list[index].time = Some(modified);
                    resize_image(&file.file.path, &file.file.output, &file.file.size).unwrap()
                }
            };
        }
        if let Mode::Compile = mode {
            return;
        }
        thread::sleep(time::Duration::from_millis(1000))
    }
}

fn resize_image(path: &str, output: &str, size: &Size) -> Result<(), String> {
    println!("updating {} to {}", path, output);
    let path = Path::new(path);
    let img = image::open(path).set_error(&format!("failed to open file {}", path.display()))?;
    let size = match size {
        Size::WidthHeight(x, y) => (*x, *y),
        Size::Width(x) => (*x, u32::max_value()),
        Size::Height(x) => (u32::max_value(), *x),
    };
    println!("{:?}", size);
    let img = img.resize(size.0, size.1, image::FilterType::Gaussian);
    img.save(output).unwrap();
    Ok(())
}

fn when_modified(path: &Path) -> Result<SystemTime, String> {
    Ok::<_, String>(
        Path::new(path)
            .metadata()
            .set_error(&format!("failed to open file {} metadata", path.display()))?
            .modified()
            .set_error(&format!(
                "failed to find files date modifide {}",
                path.display()
            )),
    )?
}

fn parse_config() -> Result<Vec<FileWatch>, String> {
    let files_list = {
        YamlLoader::load_from_str(&{
            let mut contents = String::new();

            File::open("image_watcher.yaml")
                .set_error("Failed to open config file.")?
                .read_to_string(&mut contents)
                .set_error("Failed to open read file.")?;
            contents
        })
        .set_error("Failed to parse config file.")?[0]
            .clone()
    }
    .into_hash()
    .set_error("Base of the file not a hash.")?
    .get(&Yaml::String("files".to_string()))
    .set_error("No files section in config file.")?
    .clone()
    .into_vec()
    .set_error("Files section in config is not a list.")?
    .into_iter();
    let mut files_as_hash_list = Vec::new();
    for (index, file) in files_list.enumerate() {
        files_as_hash_list.push(
            file.clone()
                .into_hash()
                .set_error(&format!("file index {} is not a hash", index))?,
        )
    }
    let mut files_list = Vec::new();
    for (index, file) in files_as_hash_list.into_iter().enumerate() {
        files_list.push({
            let path = file
                .get(&Yaml::String("path".to_string()))
                .set_error(&format!("file index {} has no path", index))?
                .clone()
                .into_string()
                .set_error(&format!(
                    "file index {} has a path that is not a string",
                    index
                ))?;
            let width = file.get(&Yaml::String("width".to_string()));
            let height = file.get(&Yaml::String("height".to_string()));
            FileWatch {
                path: path.clone(),
                output: match file.get(&Yaml::String("output".to_string())) {
                    Some(x) => x.clone().into_string().set_error(&format!(
                        "file index {} has a output path that is not a string",
                        index
                    ))?,
                    None => format!(
                        "{}{}.min.{}",
                        {
                            let parent = Path::new(&path).parent().unwrap().to_str().unwrap();
                            if parent.is_empty() {
                                parent.to_string()
                            } else {
                                format!("{}/", parent)
                            }
                        },
                        Path::new(&path).file_stem().unwrap().to_str().unwrap(),
                        Path::new(&path).extension().unwrap().to_str().unwrap()
                    ),
                },
                size: match (width, height) {
                    (Some(width), Some(height)) => Size::WidthHeight(
                        u32::try_from(width.clone().into_i64().expect("7")).unwrap(),
                        u32::try_from(height.clone().into_i64().expect("7")).unwrap(),
                    ),
                    (Some(width), None) => {
                        Size::Width(u32::try_from(width.clone().into_i64().expect("7")).unwrap())
                    }
                    (None, Some(height)) => {
                        Size::Height(u32::try_from(height.clone().into_i64().expect("7")).unwrap())
                    }
                    (None, None) => {
                        return Err(format!("file index {} has no width nor height", index))
                    }
                },
            }
        })
    }
    Ok(files_list)
}


fn mode() -> Mode {
    for a in std::env::args() {
        match a.as_ref() {
            "-c" => return Mode::Compile,
            "-w" => return Mode::Watch,
            _ => (),
        }
    }
    input()
        .repeat_msg("Do you want to run in compile or watch mode?: ")
        .err("Input the word compile or the word watch.")
        .default(Mode::Watch)
        .get()
}

impl FromStr for Mode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "C" | "c" | "-c" | "-C" | "--compile" | "--Compile" | "-compile" | "-Compile"
            | "--C" | "--c" | "compile" | "Compile" => Ok(Mode::Compile),
            "W" | "w" | "-w" | "-W" | "--watch" | "--Watch" | "-watch" | "-Watch" | "--W"
            | "--w" | "watch" | "Watch" => Ok(Mode::Watch),
            _ => Err(()),
        }
    }
}
