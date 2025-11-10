//
// Copyright (c) 2025 ZettaScale Technology
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0
//
// Contributors:
//   Julien Enoch, <julien.enoch@zettascale.tech>
//
use std::path::PathBuf;

use strum::{AsRefStr, EnumString};
use zenoh_keyexpr::OwnedKeyExpr;

use crate::type_description::HashedTypeDescription;

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, AsRefStr, EnumString, PartialEq, Eq)]
#[strum(ascii_case_insensitive)]
pub(crate) enum TypeKind {
    MSG,
    SRV,
    ACTION,
}

pub(crate) struct TypeInfo {
    pub full_name: OwnedKeyExpr, // e.g. "std_msgs/msg/String", stored as KeyExpr to facilitate key expression matching
    pub package_name: String,    // e.g. "std_msgs" for "std_msgs/msg/String"
    pub short_name: String,      // e.g. "String" for "std_msgs/msg/String"
    pub kind: TypeKind,          // MSG, SRV, or ACTION
    pub type_description: HashedTypeDescription, // complete type description from the .json file
    pub type_hash: String,       // the type hash string
    pub json_path: PathBuf,      // path to the .json file
    pub definition_path: PathBuf, // path to the original .msg/.srv/.action file
    pub definition_content: String, // content of the original .msg/.srv/.action file
}

impl TypeInfo {
    pub fn new(
        full_name: OwnedKeyExpr,
        kind: TypeKind,
        type_description: HashedTypeDescription,
        definition_content: String,
        json_path: PathBuf,
        definition_path: PathBuf,
    ) -> Result<Self, String> {
        let elements: Vec<&str> = full_name.as_str().split('/').collect();
        if elements.len() != 3 {
            return Err(format!(
                "Invalid type name format: {}. Expected format is <package>/<kind>/<name>, e.g. std_msgs/msg/String",
                full_name
            ));
        }
        let package_name = elements[0].to_string();
        let short_name = elements[2].to_string();

        // check that the kind element is the expected one
        match TypeKind::try_from(elements[1]) {
            Ok(k) => {
                if k != kind {
                    return Err(format!(
                        "Type kind mismatch: expected {:?}, found {:?} in type name {}",
                        kind.as_ref().to_lowercase(),
                        elements[1],
                        full_name
                    ));
                }
            }
            Err(_) => {
                return Err(format!(
                    "Invalid type kind '{}' in type name {}. Expected {}",
                    elements[1],
                    full_name,
                    kind.as_ref().to_lowercase()
                ));
            }
        }

        // Get this type hash
        let type_hash = type_description
            .type_hashes
            .iter()
            .find(|th| th.type_name == full_name.as_str())
            .ok_or(format!(
                "No hash found for type {} in {}",
                full_name,
                json_path.display()
            ))?
            .hash_string
            .clone();

        Ok(Self {
            full_name,
            package_name,
            short_name,
            kind,
            type_description,
            type_hash,
            json_path,
            definition_path,
            definition_content,
        })
    }

    // Return the short type name, e.g. "std_msgs/msg/String" becomes "std_msgs/String"
    pub(crate) fn get_short_type_name(&self) -> String {
        format!("{}/{}", self.package_name, self.short_name)
    }
}
