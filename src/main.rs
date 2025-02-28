use std::ffi::OsStr;
use std::fs;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::process;
use structopt::StructOpt;
use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};

use crate::decompressor::{CompressedFile, Decompressor, DecompressorTarGz, DecompressorZip};

mod decompressor;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "ThisUpdaterOfMine",
    about = "Game updater for This Space Of Mine."
)]
struct Opt {
    #[structopt(short = "a", long = "archive")]
    archives: Vec<PathBuf>,

    #[structopt(
        long = "decompress-folder",
        default_value = "tmp",
        help = "Folder where files will be all decompressed before moving them back"
    )]
    decompress_folder: PathBuf,

    #[structopt(long, help = "pid of a process to wait before updating")]
    pid: Option<u32>,

    #[structopt(short, long, help = "Executable to launch after update")]
    executable: Option<PathBuf>,

    executable_args: Vec<String>,
}

fn main() -> io::Result<()> {
    let opt = Opt::from_args();

    if let Some(pid) = opt.pid {
        wait_for_process(Pid::from_u32(pid));
    }

    if opt.decompress_folder.is_dir() {
        fs::remove_dir_all(&opt.decompress_folder)?;
    }

    // Perform decompression
    if !opt.archives.is_empty() {
        for path in &opt.archives {
            if !path.exists() {
                continue;
            }

            let archive = match path.extension().and_then(OsStr::to_str) {
                Some("gz") | Some("tgz") => match File::open(path) {
                    Ok(compressed_file) => {
                        CompressedFile::TarGz(DecompressorTarGz(compressed_file))
                    }
                    Err(_) => panic!("failed to open {0}", path.display()),
                },
                Some("zip") => match File::open(path) {
                    Ok(compressed_file) => CompressedFile::Zip(DecompressorZip(compressed_file)),
                    Err(_) => panic!("failed to open {0}", path.display()),
                },
                Some(ext) => panic!("unknown extension {ext} for file {0}", path.display()),
                None => continue,
            };

            println!("extracting {}", path.display());
            if let Err(err) = archive.extract(&opt.decompress_folder) {
                panic!("extraction failed: {err}");
            }
        }

        for entry in fs::read_dir(&opt.decompress_folder)? {
            let dir_entry = entry?;

            let target_path = PathBuf::from(dir_entry.file_name());
            if target_path.is_dir() {
                fs::remove_dir_all(&target_path)?;
            }

            fs::rename(dir_entry.path(), target_path)?;
        }

        // Remove temporary files
        for file in opt.archives {
            fs::remove_file(file)?;
        }

        fs::remove_dir(&opt.decompress_folder)?;
    }

    // Start the updated game
    if let Some(executable) = opt.executable {
        let executable_path = executable.canonicalize()?;

        match spawn_detached_process(executable_path.as_path(), &opt.executable_args) {
            Ok(_) => Ok(()),
            Err(err) => {
                eprintln!("failed to start {}: {err}", executable.display());
                Err(err)
            }
        }
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn spawn_detached_process(
    program_path: &Path,
    program_args: &[String],
) -> io::Result<process::Child> {
    use std::os::windows::process::CommandExt;

    const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
    const DETACHED_PROCESS: u32 = 0x00000008;

    process::Command::new(program_path)
        .creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS)
        .args(program_args)
        .spawn()
}

#[cfg(not(windows))]
fn spawn_detached_process(
    program_path: &Path,
    program_args: &[String],
) -> io::Result<process::Child> {
    process::Command::new(program_path)
        .args(program_args)
        .spawn()
}

fn wait_for_process(pid: Pid) {
    let s = System::new_with_specifics(
        RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing()),
    );

    // wait for process to exit
    if let Some(process) = s.process(pid) {
        println!(
            "waiting for process {0:?} ({pid}) to exit...",
            process.name()
        );
        process.wait();
    }
}
