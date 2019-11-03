use mime_guess::Mime;
use serde::Deserialize;
use std::convert::From;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, PartialEq, Deserialize)]
struct PlainConfig {
    mime_types: Vec<String>,
    name_mapping: String,
    projection_command: String,
}

pub struct ProjectionConfig {
    mime_types: Vec<Mime>,
    name_mapping: Box<dyn Fn(&OsStr) -> OsString + Sync + Send>,
    projection_command: Box<dyn Fn(&OsStr, &OsStr) + Sync + Send>,
}

impl From<PlainConfig> for ProjectionConfig {
    fn from(plain: PlainConfig) -> Self {
        let mut mime_types = Vec::new();
        for mime in &plain.mime_types {
            let mut mime = mime.to_owned();
            if !mime.contains("/") {
                mime.push('/');
            } else if mime.ends_with("/*") {
                mime.pop();
            }
            mime_types.push(mime.parse().unwrap());
        }
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
            name_mapping: Box::new(name_mapping),
            projection_command: Box::new(projection_command),
        }
    }
}

impl ProjectionConfig {
    pub fn should_project(&self, mime: &Mime) -> bool {
        for m in &self.mime_types {
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

    pub fn convert_filename<T: AsRef<OsStr>>(&self, filename: T) -> OsString {
        return (self.name_mapping)(filename.as_ref());
    }

    pub fn project(&self, input: &OsStr, output: &OsStr) {
        (self.projection_command)(input, output)
    }
}

pub fn load(filename: &OsStr) -> Option<ProjectionConfig> {
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
    Some(ProjectionConfig::from(plain_config))
}

pub fn default() -> ProjectionConfig {
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
    ProjectionConfig {
        mime_types: vec!["audio/".parse().unwrap(), "video/".parse().unwrap()],
        name_mapping: Box::new(_filename_conv),
        projection_command: Box::new(_do_proj),
    }
}
