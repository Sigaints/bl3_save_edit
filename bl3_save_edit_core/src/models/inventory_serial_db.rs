use std::collections::HashSet;
use std::convert::TryInto;
use std::io::Read;

use anyhow::{bail, Context, Result};
use json::JsonValue;
use rayon::iter::{ParallelBridge, ParallelIterator};

use crate::bl3_save::bl3_item::Bl3Part;
use crate::resources::INVENTORY_SERIAL_DB_JSON_COMPRESSED;

pub struct InventorySerialDb {
    pub data: JsonValue,
    pub max_version: usize,
}

impl InventorySerialDb {
    pub fn load() -> Result<Self> {
        let mut rdr = snap::read::FrameDecoder::new(INVENTORY_SERIAL_DB_JSON_COMPRESSED);

        let mut decompressed_bytes = String::new();

        rdr.read_to_string(&mut decompressed_bytes)
            .context("failed to read decompressed bytes")?;

        let data = json::parse(&decompressed_bytes)?;

        let max_version = data
            .entries()
            .par_bridge()
            .map(|(category, _)| {
                data[category]["versions"]
                    .members()
                    .par_bridge()
                    .map(|c| c["version"].as_isize())
                    .collect::<Vec<_>>()
            })
            .flatten()
            .flatten()
            .max()
            .and_then(|v| v.try_into().ok())
            .context("failed to read inventory serial db max version")?;

        Ok(Self { data, max_version })
    }

    pub fn get_num_bits(&self, category: &str, version: usize) -> Result<usize> {
        let mut cur_bits = self.data[category]["versions"][0]["bits"]
            .as_isize()
            .context("failed to read cur_bits")?;

        let version_isize = version as isize;

        for cat_version in self.data[category]["versions"].members() {
            let category_version = cat_version["version"]
                .as_isize()
                .context("category version was missing")?;

            if category_version > version_isize {
                return Ok(cur_bits as usize);
            } else if version_isize >= category_version {
                cur_bits = cat_version["bits"]
                    .as_isize()
                    .context("category bits was missing")?;
            }
        }

        Ok(cur_bits as usize)
    }

    pub fn get_part(&self, category: &str, index: usize) -> Result<String> {
        let assets = self.data[category]["assets"].members();

        if index > assets.len() {
            bail!("index was greater than assets length")
        } else {
            Ok(self.data[category]["assets"][index - 1].to_string())
        }
    }

    pub fn get_part_by_name(&self, category: &str, name: &str) -> Result<Bl3Part> {
        let part_info = self.data[category]["assets"]
            .members()
            .into_iter()
            .enumerate()
            .map(|(i, p)| (i, p.to_string()))
            .find(|(_, p)| p.to_lowercase().contains(&name.to_lowercase()))
            .map(|(i, p)| (i, p));

        if let Some((idx, ident)) = part_info {
            let res = Bl3Part {
                ident,
                short_ident: Some(name.to_owned()),
                idx,
            };

            Ok(res)
        } else {
            //This should never happen but lets leave it here just in case
            bail!(
                "failed to find part from inventory serial db - category: {}, name: {}",
                category,
                name
            )
        }
    }

    // Use this to ensure we only show Available Parts in the UI that we can actually add to the weapon
    pub fn par_all_parts(&self) -> HashSet<String> {
        self.data
            .entries()
            .par_bridge()
            .map(|(category, _)| {
                self.data[category]["assets"]
                    .members()
                    .par_bridge()
                    .filter_map(|p| p.to_string().rsplit('.').next().map(|s| s.to_owned()))
                    .collect::<HashSet<_>>()
            })
            .flatten()
            .collect::<HashSet<_>>()
    }
}
