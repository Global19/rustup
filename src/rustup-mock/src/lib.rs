//! Mocks for testing

extern crate url;
#[macro_use]
extern crate lazy_static;
extern crate scopeguard;
extern crate walkdir;
extern crate flate2;
extern crate xz2;
extern crate tempdir;
extern crate itertools;
extern crate tar;
extern crate toml;
extern crate rustup_utils;
extern crate sha2;
extern crate wait_timeout;

#[cfg(windows)]
extern crate winapi;
#[cfg(windows)]
extern crate winreg;

pub mod dist;
pub mod clitools;

use std::fs::{self, OpenOptions, File};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

// Mock of the on-disk structure of rust-installer installers
#[derive(PartialEq, Eq, Hash, Clone)]
pub struct MockInstallerBuilder {
    pub components: Vec<MockComponentBuilder>,
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct MockComponentBuilder {
    pub name: String,
    pub files: Vec<MockFile>,
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct MockFile {
    path: String,
    contents: Contents,
}

#[derive(PartialEq, Eq, Hash, Clone)]
enum Contents {
    File(MockContents),
    Dir(Vec<(&'static str, MockContents)>),
}

#[derive(PartialEq, Eq, Hash, Clone)]
struct MockContents {
    contents: Arc<Vec<u8>>,
    executable: bool,
}

impl MockInstallerBuilder {
    pub fn build(&self, path: &Path) {
        for component in &self.components {
            // Update the components file
            let comp_file = path.join("components");
            let ref mut comp_file = OpenOptions::new().write(true).append(true).create(true)
                .open(comp_file.clone()).unwrap();
            writeln!(comp_file, "{}", component.name).unwrap();

            // Create the component directory
            let component_dir = path.join(&component.name);
            if !component_dir.exists() {
                fs::create_dir(&component_dir).unwrap();
            }

            // Create the component files and manifest
            let ref mut manifest = File::create(component_dir.join("manifest.in")).unwrap();
            for file in component.files.iter() {
                let mk = |path: &Path, contents: &MockContents| {
                    let dir_path = path.parent().unwrap().to_owned();
                    fs::create_dir_all(dir_path).unwrap();
                    File::create(&path).unwrap()
                        .write_all(&contents.contents).unwrap();

                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if contents.executable {
                            let mut perm = fs::metadata(path).unwrap().permissions();
                            perm.set_mode(0o755);
                            fs::set_permissions(path, perm).unwrap();
                        }
                    }
                };
                match file.contents {
                    Contents::Dir(ref files) => {
                        writeln!(manifest, "dir:{}", file.path).unwrap();
                        for &(ref name, ref contents) in files {
                            let fname = component_dir.join(&file.path).join(name);
                            mk(&fname, contents);
                        }
                    }
                    Contents::File(ref contents) => {
                        writeln!(manifest, "file:{}", file.path).unwrap();
                        let fname = component_dir.join(&file.path);
                        mk(&fname, contents);
                    }
                }
            }
        }

        let mut ver = File::create(path.join("rust-installer-version")).unwrap();
        writeln!(ver, "3").unwrap();
    }
}

impl MockFile {
    pub fn new<S: Into<String>>(path: S, contents: &[u8]) -> MockFile {
        MockFile::_new(path.into(), Arc::new(contents.to_vec()))
    }

    pub fn new_arc<S: Into<String>>(path: S, contents: Arc<Vec<u8>>) -> MockFile {
        MockFile::_new(path.into(), contents)
    }

    fn _new(path: String, contents: Arc<Vec<u8>>) -> MockFile {
        MockFile {
            path: path,
            contents: Contents::File(MockContents {
                contents: contents,
                executable: false,
            }),
        }
    }

    pub fn new_dir(path: &str, files: &[(&'static str, &'static [u8], bool)]) -> MockFile {
        MockFile {
            path: path.to_string(),
            contents: Contents::Dir(files.iter().map(|&(name, data, exe)| {
                (name, MockContents {
                    contents: Arc::new(data.to_vec()),
                    executable: exe,
                })
            }).collect()),
        }
    }

    pub fn executable(mut self, exe: bool) -> Self {
        match self.contents {
            Contents::File(ref mut c) => c.executable = exe,
            _ => {}
        }
        self
    }
}

#[cfg(windows)]
pub fn get_path() -> Option<String> {
    use winreg::RegKey;
    use winapi::*;

    let root = RegKey::predef(HKEY_CURRENT_USER);
    let environment = root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE).unwrap();

    environment.get_value("PATH").ok()
}

#[cfg(windows)]
pub fn restore_path(p: &Option<String>) {
    use winreg::{RegKey, RegValue};
    use winreg::enums::RegType;
    use winapi::*;

    let root = RegKey::predef(HKEY_CURRENT_USER);
    let environment = root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE).unwrap();

    if let Some(p) = p.as_ref() {
        let reg_value = RegValue {
            bytes: string_to_winreg_bytes(&p),
            vtype: RegType::REG_EXPAND_SZ,
        };
        environment.set_raw_value("PATH", &reg_value).unwrap();
    } else {
        let _ = environment.delete_value("PATH");
    }

    fn string_to_winreg_bytes(s: &str) -> Vec<u8> {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStrExt;
        let v: Vec<_> = OsString::from(format!("{}\x00", s)).encode_wide().collect();
        unsafe { ::std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 2).to_vec() }
    }
}

#[cfg(unix)]
pub fn get_path() -> Option<String> { None }

#[cfg(unix)]
pub fn restore_path(_: &Option<String>) { }

