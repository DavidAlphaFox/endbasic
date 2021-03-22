// EndBASIC
// Copyright 2021 Julio Merino
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License.  You may obtain a copy
// of the License at:
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
// WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.  See the
// License for the specific language governing permissions and limitations
// under the License.

//! In-memory implementation of the storage system.

use crate::storage::{Drive, Metadata};
use std::collections::{BTreeMap, HashMap};
use std::io;
use std::str;

/// A drive that records all data in memory only.
#[derive(Default)]
pub struct InMemoryDrive {
    programs: HashMap<String, String>,
}

impl InMemoryDrive {
    /// Returns the mapping of stored file names to their contents.
    pub fn as_hashmap(&self) -> &HashMap<String, String> {
        &self.programs
    }
}

impl Drive for InMemoryDrive {
    fn delete(&mut self, name: &str) -> io::Result<()> {
        match self.programs.remove(name) {
            Some(_) => Ok(()),
            None => Err(io::Error::new(io::ErrorKind::NotFound, "Entry not found")),
        }
    }

    fn enumerate(&self) -> io::Result<BTreeMap<String, Metadata>> {
        let date = time::OffsetDateTime::from_unix_timestamp(1_588_757_875);

        let mut entries = BTreeMap::new();
        for (name, contents) in &self.programs {
            entries.insert(name.clone(), Metadata { date, length: contents.len() as u64 });
        }
        Ok(entries)
    }

    fn get(&self, name: &str) -> io::Result<String> {
        match self.programs.get(name) {
            Some(content) => Ok(content.to_owned()),
            None => Err(io::Error::new(io::ErrorKind::NotFound, "Entry not found")),
        }
    }

    fn put(&mut self, name: &str, content: &str) -> io::Result<()> {
        self.programs.insert(name.to_owned(), content.to_owned());
        Ok(())
    }
}
