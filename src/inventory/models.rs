use crate::inventory::builder::InventoryBuilder;
use crate::io::encoding::PostProcessor;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tracing::debug;

#[derive(Clone, Debug)]
pub struct Inventory {
    pub path: PathBuf,
    pub data: InventoryData,
}

impl Inventory {
    /// Build a new inventory.
    pub fn builder() -> InventoryBuilder {
        InventoryBuilder::new()
    }

    /// Read inventory from a target path.
    pub async fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            bail!("inventory file not found, need help? see https://github.com/simbleau/cddns#readme");
        } else {
            debug!("inventory file found");
        }
        let path = path.canonicalize().with_context(|| {
            format!(
                "getting canonical path to inventory file '{}'",
                path.display()
            )
        })?;
        debug!("reading inventory path: '{}'", path.display());
        let contents = tokio::fs::read_to_string(&path)
            .await
            .context("reading inventory file")?;
        Inventory::builder()
            .path(path)
            .with_bytes(contents.as_bytes())?
            .build()
    }

    /// Return the inventory as a processed string.
    pub fn to_string<PP>(&self, post_processor: Option<PP>) -> Result<String>
    where
        PP: PostProcessor,
    {
        self.data.to_string(post_processor)
    }

    /// Save the inventory file at the given path, overwriting if necessary, and
    /// optionally with post-processed comments.
    pub async fn save<PP>(&self, post_processor: Option<PP>) -> Result<()>
    where
        PP: PostProcessor,
    {
        let yaml = self.data.to_string(post_processor)?;
        crate::io::fs::save(&self.path, yaml).await
    }
}

/// The model for DNS record inventory.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InventoryData(pub Option<HashMap<String, InventoryZone>>);

/// The model for a zone with records.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InventoryZone(pub Option<HashSet<InventoryRecord>>);

/// The model for a DNS record.
#[derive(Clone, Debug, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct InventoryRecord(pub String);

impl InventoryData {
    /// Return the inventory as a processed string.
    pub fn to_string<PP>(&self, post_processor: Option<PP>) -> Result<String>
    where
        PP: PostProcessor,
    {
        crate::io::encoding::as_yaml(&self, post_processor)
    }

    /// Returns whether a record exists in the inventory data.
    pub fn contains(
        &self,
        zone_id: impl Into<String>,
        record_id: impl Into<String>,
    ) -> bool {
        let zone_id = zone_id.into();
        let record_id = InventoryRecord(record_id.into());

        // Magic that checks whether the record exists
        self.0
            .as_ref()
            .and_then(|map| map.get(&zone_id))
            .and_then(|zone| zone.0.as_ref())
            .map(|records| records.contains(&record_id))
            .unwrap_or(false)
    }

    /// Insert a record into the inventory data.
    pub fn insert(
        &mut self,
        zone_id: impl Into<String>,
        record_id: impl Into<String>,
    ) {
        // Magic that inserts the record
        self.0
            .get_or_insert(HashMap::new())
            .entry(zone_id.into())
            .or_insert_with(|| InventoryZone(None))
            .0
            .get_or_insert(HashSet::new())
            .insert(InventoryRecord(record_id.into()));
    }

    /// Remove a record from the inventory data. Returns whether the value was
    /// present in the set.
    pub fn remove(
        &mut self,
        zone_id: impl Into<String>,
        record_id: impl Into<String>,
    ) -> Result<bool> {
        let zone_id = zone_id.into();
        let record_id = record_id.into();

        let mut removed = false;
        let mut prune = false; // whether to remove an empty zone container
        if let Some(map) = self.0.as_mut() {
            if let Some(zone) = map.get_mut(&zone_id) {
                if let Some(records) = zone.0.as_mut() {
                    removed = records.remove(&InventoryRecord(record_id));
                    prune = records.is_empty();
                }
            }
            if prune {
                map.remove(&zone_id);
            }
        }
        Ok(removed)
    }

    /// Returns whether the inventory data has no records
    pub fn is_empty(&self) -> bool {
        // Magic that checks whether there are records
        !self
            .0
            .as_ref()
            .map(|map| {
                map.iter().fold(0, |items, (_, zone)| {
                    items + zone.0.as_ref().map(|z| z.len()).unwrap_or(0)
                })
            })
            .is_some_and(|len| len > 0)
    }
}
