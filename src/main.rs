use flate2::read::GzDecoder;
use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::str::FromStr;
use std::{env, fs};
use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};
use tar::Archive;
use zip::ZipArchive;

struct DecompressorTarGz {
    file: File,
}

struct DecompressorZip {
    file: File,
}

trait Decompressor {
    fn extract(&self, file: &Path) -> io::Result<()>;
}

impl Decompressor for DecompressorTarGz {
    fn extract(&self, dst: &Path) -> io::Result<()> {
        let decoder = GzDecoder::new(&self.file);
        let mut archive = Archive::new(decoder);

        archive.unpack(dst)
    }
}

impl Decompressor for DecompressorZip {
    fn extract(&self, dst: &Path) -> io::Result<()> {
        let mut archive = ZipArchive::new(&self.file)?;
        Ok(archive.extract(dst)?)
    }
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        println!("usage: autoupdater pid process archives...");
        return Ok(());
    }

    let pid_str = &args[1];
    let executable_name = &args[2];
    let filenames = &args[3..];

    let pid = match Pid::from_str(pid_str) {
        Ok(pid) => pid,
        Err(err) => {
            panic!("invalid pid: {}", err);
        }
    };

    if pid.as_u32() != 0 {
        wait_for_process(pid);
    }

    let temp_folder = Path::new("tmp");
    if temp_folder.is_dir() {
        fs::remove_dir_all(temp_folder)?;
    }

    for file in filenames {
        let path = Path::new(file);
        if !path.exists() {
            continue;
        }

        let ext = path.extension();
        if ext.is_none() {
            continue;
        }

        let archive: Option<Box<dyn Decompressor>> = match path.extension().and_then(OsStr::to_str)
        {
            Some("gz") => {
                if let Ok(compressed_file) = File::open(path) {
                    Some(Box::new(DecompressorTarGz {
                        file: compressed_file,
                    }))
                } else {
                    panic!("failed to open {0}", path.display());
                }
            }
            Some("zip") => {
                if let Ok(compressed_file) = File::open(path) {
                    Some(Box::new(DecompressorZip {
                        file: compressed_file,
                    }))
                } else {
                    panic!("failed to open {0}", path.display());
                }
            }
            Some(ext) => {
                panic!("unknown extension {ext} for file {0}", path.display());
            }
            _ => {
                panic!("failed to get extension of file {0}", path.display());
            }
        };

        if archive.is_none() {
            continue;
        }

        println!("extracting {}", path.display());
        let result = archive.unwrap().extract(temp_folder);
        if result.is_err() {
            panic!("extraction failed: {}", result.unwrap_err());
        }
    }

    for entry in fs::read_dir(temp_folder)? {
        let dir_entry = entry?;

        let target_path = PathBuf::from(dir_entry.file_name());
        if target_path.is_dir() {
            fs::remove_dir_all(&target_path)?;
        }

        fs::rename(dir_entry.path(), target_path)?;
    }

    // Remove temporary files
    for file in filenames {
        fs::remove_file(file)?;
    }

    fs::remove_dir(temp_folder)?;

    // Start the updated game
    let executable_path = Path::canonicalize(Path::new(executable_name))?;

    match spawn_detached_process(&executable_path) {
        Ok(_) => Ok(()),
        Err(err) => {
            println!("failed to start {executable_name}: {err}");
            Err(err)
        }
    }
}

#[cfg(windows)]
fn spawn_detached_process(program_path: &Path) -> std::io::Result<process::Child> {
    use std::os::windows::process::CommandExt;

    const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
    const DETACHED_PROCESS: u32 = 0x00000008;

    process::Command::new(program_path)
        .creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS)
        .spawn()
}

#[cfg(not(windows))]
fn spawn_detached_process(program_path: &Path) -> std::io::Result<process::Child> {
    process::Command::new(program_path).spawn()
}

fn wait_for_process(pid: Pid) {
    let s =
        System::new_with_specifics(RefreshKind::new().with_processes(ProcessRefreshKind::new()));

    // wait for process to exit
    if let Some(process) = s.process(pid) {
        println!("waiting for process {0} ({pid}) to exit...", process.name());
        process.wait();
    }
}
