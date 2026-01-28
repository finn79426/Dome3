use crate::models::{AddressFormat, AddressLabel};
use anyhow::{Context as _AnyhowContext, Result};
use csv::{Reader, Writer, WriterBuilder};
use directories::ProjectDirs;
use std::fs::{File, OpenOptions};
use std::path::PathBuf;

#[derive(Debug)]
pub struct Context {
    file_path: PathBuf,
    data: Vec<AddressLabel>,
}

impl Default for Context {
    fn default() -> Self {
        if let Some(project_dirs) = ProjectDirs::from("com", "dome3", "app") {
            Self::new(project_dirs.data_dir().join("labeled_address.csv"))
                .expect("Failed to initialize CSV file")
        } else {
            Self::new("labeled_address.csv").expect("Failed to initialize CSV file")
        }
    }
}

impl Context {
    pub fn new<T: Into<PathBuf>>(file_path: T) -> Result<Self> {
        let path = file_path.into();
        let mut data = Vec::new();

        if path.exists() {
            let file = File::open(&path).context("Unable to open existing csv file")?;
            let mut reader = Reader::from_reader(file);
            data = reader
                .deserialize()
                .map(|r| r.context("Failed to deserialize csv rows"))
                .collect::<Result<Vec<AddressLabel>>>()?;
        } else {
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent)
                        .context("Failed to create application data directory")?;
                }
            }

            let file = File::create(&path).context("Unable to create a new csv file")?;
            let mut writer = Writer::from_writer(file);

            // FIXME: Hardcoded values.
            // The column order and names MUST match `models::AddressLabel` exactly.
            // If fields in `AddressLabel` are added, removed, or reordered,
            // this header and the corresponding write logic must be updated,
            // otherwise CSV serialization/deserialization will break.
            //
            // TODO: Generate headers from the `models::AddressLabel`
            writer.write_record(&["network", "address", "label"])?;
            writer.flush().context("Failed to flush writer")?;
        }

        Ok(Self {
            file_path: path,
            data,
        })
    }

    #[allow(unused)]
    /// Synchronize `data` from reading the csv file.
    pub fn sync(&mut self) -> Result<()> {
        let file = File::open(&self.file_path).context("Unable to open existing csv file")?;
        let mut reader = Reader::from_reader(file);

        self.data = reader
            .deserialize()
            .map(|r| r.context("Failed to deserialize csv row during sync"))
            .collect::<Result<Vec<AddressLabel>>>()?;

        Ok(())
    }

    /// Append an `AddressLabel` to the csv file.
    pub fn append(&mut self, record: AddressLabel) -> Result<()> {
        self.data.push(record);

        let file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(&self.file_path)
            .context("Unable to open existing csv file")?;

        let mut writer = WriterBuilder::new().has_headers(false).from_writer(file);

        if let Some(last_record) = self.data.last() {
            writer
                .serialize(last_record)
                .context("Failed to serialize record")?;
            writer.flush().context("Failed to flush writer")?;
        }

        Ok(())
    }

    /// Find a entry by `network` and `address`.
    /// If multiple entries are found, return the last one
    pub fn find(&self, network: &AddressFormat, address: &str) -> Option<&AddressLabel> {
        self.data
            .iter()
            .rev()
            .find(|record| record.network == *network && record.address == address)
    }
}
