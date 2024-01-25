use flate2::read::GzDecoder;
use std::fs::File;
use std::io;
use std::path::Path;
use tar::Archive;
use zip::ZipArchive;

pub trait Decompressor {
    fn extract(&self, dst: &Path) -> io::Result<()>;
}

pub struct DecompressorTarGz(pub File);
pub struct DecompressorZip(pub File);

pub enum CompressedFile {
    TarGz(DecompressorTarGz),
    Zip(DecompressorZip),
}

impl Decompressor for DecompressorTarGz {
    fn extract(&self, dst: &Path) -> io::Result<()> {
        let decoder = GzDecoder::new(&self.0);
        let mut archive = Archive::new(decoder);

        archive.unpack(dst)
    }
}

impl Decompressor for DecompressorZip {
    fn extract(&self, dst: &Path) -> io::Result<()> {
        let mut archive = ZipArchive::new(&self.0)?;
        Ok(archive.extract(dst)?)
    }
}

impl Decompressor for CompressedFile {
    fn extract(&self, dst: &Path) -> io::Result<()> {
        match self {
            CompressedFile::TarGz(file) => file.extract(dst),
            CompressedFile::Zip(file) => file.extract(dst),
        }
    }
}
