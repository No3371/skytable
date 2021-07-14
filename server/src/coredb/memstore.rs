/*
 * Created on Fri Jul 02 2021
 *
 * This file is a part of Skytable
 * Skytable (formerly known as TerrabaseDB or Skybase) is a free and open-source
 * NoSQL database written by Sayan Nandan ("the Author") with the
 * vision to provide flexibility in data modelling without compromising
 * on performance, queryability or scalability.
 *
 * Copyright (c) 2021, Sayan Nandan <ohsayan@outlook.com>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 *
*/

//! # In-memory store
//!
//! This is what things look like:
//! ```text
//! -----------------------------------------------------
//! |                                                   |
//! |  |-------------------|     |-------------------|  |
//! |  |-------------------|     |-------------------|  |
//! |  | | TABLE | TABLE | |     | | TABLE | TABLE | |  |
//! |  | |-------|-------| |     | |-------|-------| |  |
//! |  |      Keyspace     |     |      Keyspace     |  |
//! |  |-------------------|     |-------------------|  |
//!                                                     |
//! |  |-------------------|     |-------------------|  |
//! |  | |-------|-------| |     | |-------|-------| |  |
//! |  | | TABLE | TABLE | |     | | TABLE | TABLE | |  |
//! |  | |-------|-------| |     | |-------|-------| |  |
//! |  |      Keyspace     |     |      Keyspace     |  |
//! |  |-------------------|     |-------------------|  |
//! |                                                   |
//! |                                                   |
//! |                                                   |
//! -----------------------------------------------------
//! |                         NODE                      |
//! |---------------------------------------------------|
//! ```
//!
//! So, all your data is at the mercy of [`Memstore`]'s constructor
//! and destructor.

#![allow(dead_code)] // TODO(@ohsayan): Remove this onece we're done

use crate::coredb::array::Array;
use crate::coredb::htable::Coremap;
use crate::coredb::htable::Data;
use crate::coredb::SnapshotStatus;
use crate::kvengine::KVEngine;
use core::mem::MaybeUninit;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[sky_macros::array]
const DEFAULT_ARRAY: [MaybeUninit<u8>; 64] = [b'd', b'e', b'f', b'a', b'u', b'l', b't'];

/// The `DEFAULT` array (with the rest uninit)
pub const DEFAULT: Array<u8, 64> = Array::from_const(DEFAULT_ARRAY, 7);

#[test]
fn test_def_macro_sanity() {
    // just make sure our macro is working as expected
    let mut def = DEFAULT.clone();
    def.push(b'?');
    assert_eq!(
        def.into_iter().map(char::from).collect::<String>(),
        "default?".to_owned()
    );
}

/// typedef for the keyspace/table IDs. We don't need too much fancy here,
/// no atomic pointers and all. Just a nice array. With amazing gurantees
type ObjectID = Array<u8, 64>;

mod cluster {
    /// This is for the future where every node will be allocated a shard
    #[derive(Debug)]
    pub enum ClusterShardRange {
        SingleNode,
    }

    impl Default for ClusterShardRange {
        fn default() -> Self {
            Self::SingleNode
        }
    }

    /// This is for the future for determining the replication strategy
    #[derive(Debug)]
    pub enum ReplicationStrategy {
        /// Single node, no replica sets
        Default,
    }

    impl Default for ReplicationStrategy {
        fn default() -> Self {
            Self::Default
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum DdlError {
    StillInUse,
    ObjectNotFound,
    ProtectedObject,
}

#[derive(Debug)]
/// The core in-memory table
///
/// This in-memory table that houses all keyspaces along with other node properties.
/// This is the structure that you should clone and send around connections for
/// connection-level control abilities over the keyspace
pub struct Memstore {
    /// the keyspaces
    keyspaces: Arc<Coremap<ObjectID, Arc<Keyspace>>>,
}

impl Memstore {
    /// Create a new empty in-memory table with literally nothing in it
    pub fn new_empty() -> Self {
        Self {
            keyspaces: Arc::new(Coremap::new()),
        }
    }
    /// Create a new in-memory table with the default keyspace and the default
    /// tables. So, whenever you're calling this, this is what you get:
    /// ```text
    /// YOURNODE: {
    ///     KEYSPACES: [
    ///         "default" : {
    ///             TABLES: ["default", "_system"]
    ///         }
    ///     ]
    /// }
    /// ```
    ///
    /// When you connect a client without any information about the keyspace you're planning to
    /// use, you'll be connected to `ks:default/table:default`. The `ks:default/table:_system` is not
    /// for you. It's for the system
    pub fn new_default() -> Self {
        Self {
            keyspaces: {
                let n = Coremap::new();
                n.true_if_insert(DEFAULT, Arc::new(Keyspace::empty_default()));
                Arc::new(n)
            },
        }
    }
    /// Get an atomic reference to a keyspace
    pub fn get_keyspace_atomic_ref(&self, keyspace_identifier: ObjectID) -> Option<Arc<Keyspace>> {
        self.keyspaces
            .get(&keyspace_identifier)
            .map(|ns| ns.clone())
    }
    /// Returns true if a new keyspace was created
    pub fn create_keyspace(&self, keyspace_identifier: ObjectID) -> bool {
        self.keyspaces
            .true_if_insert(keyspace_identifier, Arc::new(Keyspace::empty()))
    }
}

/// The date model of a table
pub enum TableType {
    KeyValue,
}

// TODO(@ohsayan): Optimize the memory layouts of the UDFs to ensure that sharing is very cheap

#[derive(Debug)]
/// A keyspace houses all the other tables
pub struct Keyspace {
    /// the tables
    tables: Coremap<Data, Arc<Table>>,
    /// current state of the disk flush status. if this is true, we're safe to
    /// go ahead with writes
    flush_state_healthy: AtomicBool,
    /// the snapshot configuration for this keyspace
    snap_config: Option<SnapshotStatus>,
    /// the replication strategy for this keyspace
    replication_strategy: cluster::ReplicationStrategy,
}

impl Keyspace {
    /// Create a new empty keyspace with the default tables: a `default` table and a
    /// `system` table
    pub fn empty_default() -> Self {
        Self {
            tables: {
                let ht = Coremap::new();
                // add the default table
                ht.true_if_insert(
                    Data::from("default"),
                    Arc::new(Table::KV(KVEngine::default())),
                );
                // add the system table
                ht.true_if_insert(
                    Data::from("_system"),
                    Arc::new(Table::KV(KVEngine::default())),
                );
                ht
            },
            flush_state_healthy: AtomicBool::new(true),
            snap_config: None,
            replication_strategy: cluster::ReplicationStrategy::default(),
        }
    }
    /// Create a new empty keyspace with zero tables
    pub fn empty() -> Self {
        Self {
            tables: Coremap::new(),
            flush_state_healthy: AtomicBool::new(true),
            snap_config: None,
            replication_strategy: cluster::ReplicationStrategy::default(),
        }
    }
    /// Get an atomic reference to a table in this keyspace if it exists
    pub fn get_table_atomic_ref(&self, table_identifier: Data) -> Option<Arc<Table>> {
        self.tables.get(&table_identifier).map(|v| v.clone())
    }
    /// Create a new table with **default encoding**
    pub fn create_table(&self, table_identifier: Data, table_type: TableType) -> bool {
        self.tables.true_if_insert(table_identifier, {
            match table_type {
                TableType::KeyValue => Arc::new(Table::KV(KVEngine::default())),
            }
        })
    }
    pub fn drop_table(&self, table_identifier: Data) -> Result<(), DdlError> {
        if table_identifier.eq(&Data::from("default"))
            || table_identifier.eq(&Data::from("_system"))
        {
            Err(DdlError::ProtectedObject)
        } else if self.tables.contains_key(&table_identifier) {
            // has table
            let did_remove =
                self.tables
                    .true_remove_if(&table_identifier, |_table_id, table_atomic_ref| {
                        // 1 because this should just be us, the one instance
                        Arc::strong_count(table_atomic_ref) == 1
                    });
            if did_remove {
                Ok(())
            } else {
                Err(DdlError::StillInUse)
            }
        } else {
            Err(DdlError::ObjectNotFound)
        }
    }
}

#[test]
fn test_keyspace_drop_no_atomic_ref() {
    let our_keyspace = Keyspace::empty_default();
    assert!(our_keyspace.create_table(Data::from("apps"), TableType::KeyValue));
    assert!(our_keyspace.drop_table(Data::from("apps")).is_ok());
}

#[test]
fn test_keyspace_drop_fail_with_atomic_ref() {
    let our_keyspace = Keyspace::empty_default();
    assert!(our_keyspace.create_table(Data::from("apps"), TableType::KeyValue));
    let _atomic_tbl_ref = our_keyspace
        .get_table_atomic_ref(Data::from("apps"))
        .unwrap();
    assert_eq!(
        our_keyspace.drop_table(Data::from("apps")).unwrap_err(),
        DdlError::StillInUse
    );
}

#[test]
fn test_keyspace_try_delete_protected_table() {
    let our_keyspace = Keyspace::empty_default();
    assert_eq!(
        our_keyspace.drop_table(Data::from("default")).unwrap_err(),
        DdlError::ProtectedObject
    );
    assert_eq!(
        our_keyspace.drop_table(Data::from("_system")).unwrap_err(),
        DdlError::ProtectedObject
    );
}

// same 8 byte ptrs; any chance of optimizations?

#[derive(Debug)]
/// The underlying table type. This is the place for the other data models (soon!)
pub enum Table {
    /// a key/value store
    KV(KVEngine),
}

impl Table {
    /// Get the key/value store if the table is a key/value store
    pub const fn get_kvstore(&self) -> Option<&KVEngine> {
        #[allow(irrefutable_let_patterns)]
        if let Self::KV(kvs) = self {
            Some(kvs)
        } else {
            None
        }
    }
}
