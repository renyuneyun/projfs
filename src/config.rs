use mime_guess::{mime, Mime};
use serde::Deserialize;
use std::convert::From;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::path::PathBuf;
use std::process::Command;

pub trait ProjectionSpecification: Send + Sync {
    fn should_project(&self, mime: &Mime) -> bool;

    fn convert_filename(&self, filename: &OsStr) -> OsString;

    fn project(&self, input: &OsStr, output: &OsStr);
}

fn user_string_to_mime(string_mime_types: &Vec<String>) -> Vec<Mime> {
    let mut mime_types = Vec::new();
    for mime in string_mime_types {
        let mut mime = mime.to_owned();
        if !mime.contains("/") {
            mime.push('/');
        } else if mime.ends_with("/*") {
            mime.pop();
        }
        mime_types.push(mime.parse().unwrap());
    }
    mime_types
}

fn in_mime_vec(mime: &Mime, mime_types: &Vec<Mime>) -> bool {
    for m in mime_types {
        if mime.type_() == m.type_() {
            if m.subtype() == "" {
                return true;
            } else {
                return mime.subtype() == m.subtype();
            }
        }
    }
    false
}

#[derive(Debug, PartialEq, Deserialize)]
struct PlainConfig {
    mime_types: Vec<String>,
    ignored_mime_types: Option<Vec<String>>,
    name_mapping: String,
    projection_command: String,
}

struct ProjectionConfig {
    mime_types: Vec<Mime>,
    ignored_mime_types: Vec<Mime>,
    name_mapping: Box<dyn Fn(&OsStr) -> OsString + Sync + Send>,
    projection_command: Box<dyn Fn(&OsStr, &OsStr) + Sync + Send>,
}

impl From<PlainConfig> for ProjectionConfig {
    fn from(plain: PlainConfig) -> Self {
        let mime_types = user_string_to_mime(plain.mime_types.as_ref());
        let ignored_mime_types =
            user_string_to_mime(plain.ignored_mime_types.as_ref().unwrap_or(&Vec::new()));
        let _name_mapping = {
            let mapping = &(&plain).name_mapping;
            if mapping.starts_with(".") {
                mapping[1..].to_string()
            } else {
                mapping.to_string()
            }
        };
        let name_mapping = move |filename: &OsStr| {
            let mut path_buf = PathBuf::from(filename);
            path_buf.set_extension(&_name_mapping);
            path_buf.into_os_string()
        };

        let parts: Vec<String> = plain
            .projection_command
            .split(" ")
            .map(|s| s.into())
            .collect();
        let projection_command = move |input: &OsStr, output: &OsStr| {
            let segments: Vec<String> = parts
                .iter()
                .map(|s| s.replace("{input}", input.to_str().unwrap()))
                .map(|s| s.replace("{output}", output.to_str().unwrap()))
                .collect();
            let mut cmd = Command::new(&segments[0])
                .args(&segments[1..])
                .spawn()
                .expect("failed to execute process");
            cmd.wait().unwrap();
        };
        ProjectionConfig {
            mime_types: mime_types,
            ignored_mime_types: ignored_mime_types,
            name_mapping: Box::new(name_mapping),
            projection_command: Box::new(projection_command),
        }
    }
}

impl ProjectionSpecification for ProjectionConfig {
    fn should_project(&self, mime: &Mime) -> bool {
        !in_mime_vec(mime, &self.ignored_mime_types) && in_mime_vec(mime, &self.mime_types)
    }

    fn convert_filename(&self, filename: &OsStr) -> OsString {
        return (self.name_mapping)(filename.as_ref());
    }

    fn project(&self, input: &OsStr, output: &OsStr) {
        (self.projection_command)(input, output)
    }
}

pub fn load(filename: &OsStr) -> Option<Box<dyn ProjectionSpecification>> {
    let f = match File::open(filename) {
        Ok(f) => f,
        Err(e) => {
            error!(
                "Error when opening projection configuration file {:?}: {}",
                filename, e
            );
            return None;
        }
    };
    let plain_config: PlainConfig = match serde_yaml::from_reader(f) {
        Ok(config) => config,
        Err(e) => {
            error!(
                "Error while reading projection configuration file @ {:?}",
                e.location()
            );
            return None;
        }
    };
    Some(Box::new(ProjectionConfig::from(plain_config)))
}

struct DefaultConfig;

impl DefaultConfig {
    fn _should_project(mime_type: &Mime) -> bool {
        return mime_type.type_() == mime::AUDIO && mime_type.subtype() != "ogg"
            || mime_type.type_() == mime::VIDEO;
    }

    fn _filename_conv(partial: &OsStr) -> OsString {
        let mut path_buf = PathBuf::from(partial);
        path_buf.set_extension("ogg");
        path_buf.into_os_string()
    }
    fn _do_proj(input: &OsStr, output: &OsStr) {
        debug!("do_proj() call: {:?} -> {:?}", input, output);
        let mut cmd = Command::new("ffmpeg") // Streaming
            .args(&[
                "-i",
                input.to_str().unwrap(),
                "-vn",
                output.to_str().unwrap(),
            ])
            .spawn()
            .expect("failed to execute process");
        cmd.wait().unwrap();
    }
}

impl ProjectionSpecification for DefaultConfig {
    fn should_project(&self, mime: &Mime) -> bool {
        DefaultConfig::_should_project(mime)
    }

    fn convert_filename(&self, filename: &OsStr) -> OsString {
        DefaultConfig::_filename_conv(filename)
    }

    fn project(&self, input: &OsStr, output: &OsStr) {
        DefaultConfig::_do_proj(input, output)
    }
}

pub fn default() -> Box<dyn ProjectionSpecification> {
    Box::new(DefaultConfig {})
}
