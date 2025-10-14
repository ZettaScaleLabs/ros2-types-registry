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

use crate::{
    type_description::HashedTypeDescription,
    type_info::{TypeInfo, TypeKind},
};
use core::convert::TryFrom;
use std::path::PathBuf;
use zenoh::key_expr::{
    keyexpr,
    keyexpr_tree::{IKeyExprTree, IKeyExprTreeMut, KeBoxTree},
    KeyExpr,
};
use zenoh_keyexpr::{keyexpr_tree::traits::IKeyExprTreeNode, OwnedKeyExpr};

pub(crate) struct Registry<'a> {
    types: KeBoxTree<TypeInfo>,
    size: usize,
    _marker: std::marker::PhantomData<&'a TypeInfo>,
}

impl<'a> Registry<'a> {
    pub fn new() -> Self {
        Self {
            types: KeBoxTree::new(),
            size: 0,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn load_types_from_dir(&mut self, dir: &PathBuf) {
        tracing::debug!("Loading types from {}", dir.display());

        let mut count = 0usize;
        for entry in walkdir::WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| {
                if let Err(err) = &e {
                    tracing::warn!("Error accessing entry: {err}");
                }
                e.ok()
            })
            .filter(|e| e.path().is_file())
        {
            if let Some(extension) = entry.path().extension() {
                let kind = if extension == "msg" {
                    TypeKind::MSG
                } else if extension == "srv" {
                    TypeKind::SRV
                } else if extension == "action" {
                    TypeKind::ACTION
                } else {
                    continue;
                };

                match self.load_type_from_file(entry.path().into(), kind) {
                    Ok(()) => count += 1,
                    Err(e) => tracing::warn!("  {e}"),
                }
            }
        }
        tracing::info!("{} types loaded from {}", count, dir.display());
        self.size += count;
    }

    pub fn load_type_from_file(
        &mut self,
        definition_path: std::path::PathBuf,
        kind: TypeKind,
    ) -> Result<(), String> {
        // Find and read the corresponding JSON file
        let json_path = definition_path.with_extension("json");
        if !json_path.exists() {
            return Err(format!(
                "No JSON description found for {}",
                definition_path.display()
            ));
        }
        let json_str = std::fs::read_to_string(&json_path)
            .map_err(|e| format!("Failed to read JSON file {}: {}", json_path.display(), e))?;
        let type_description: HashedTypeDescription = serde_json::from_str(&json_str)
            .map_err(|e| format!("Failed to parse JSON file {}: {}", json_path.display(), e))?;

        // Get this type name
        let type_name = OwnedKeyExpr::try_from(
            type_description
                .type_description_msg
                .type_description
                .type_name
                .clone(),
        )
        .map_err(|e| {
            format!(
                "Invalid type name '{}' in {}: {}",
                type_description
                    .type_description_msg
                    .type_description
                    .type_name,
                json_path.display(),
                e
            )
        })?;

        // Read the definition file content
        let definition_content = std::fs::read_to_string(&definition_path).map_err(|e| {
            format!(
                "Failed to read definition file {}: {}",
                definition_path.display(),
                e
            )
        })?;

        let type_info = TypeInfo::new(
            type_name,
            kind,
            type_description,
            definition_content,
            json_path,
            definition_path,
        )?;

        // Check if already loaded
        if let Some(existing) = self.types.weight_at(&type_info.full_name) {
            if existing.type_hash == type_info.type_hash {
                // Already loaded, same version - skip
                return Ok(());
            } else {
                return Err(format!("Found conflicting hash for {} loaded from {} : see {}. Check types definitions!",
                    type_info.full_name, existing.json_path.display(), type_info.json_path.display()));
            }
        }

        tracing::debug!(
            "{} loaded from {} and {}",
            type_info.full_name,
            type_info.json_path.display(),
            type_info.definition_path.display()
        );

        self.types.insert(&type_info.full_name.clone(), type_info);

        Ok(())
    }

    pub fn get_size(&self) -> usize {
        self.size
    }

    // Get all types matching a key expression
    pub fn get_types(&'a self, ke: &'a keyexpr) -> Vec<&'a TypeInfo> {
        tracing::debug!("Searching types matching {}", ke);
        self.types
            .included_nodes(ke)
            .filter_map(|n| n.weight())
            .collect()
    }

    // Generate a concatenated type definition with its dependencies, in the same way than rosbag2 here:
    // https://github.com/ros2/rosbag2/blob/cfb7c2114b76a53e459c7032b7c5d44fb477475d/rosbag2_cpp/include/rosbag2_cpp/message_definitions/local_message_definition_source.hpp#L88
    pub(crate) fn get_mcap_schema(&self, t: &TypeInfo) -> String {
        const SEPARATOR: &str =
            "\n================================================================================\n";

        // Add main type definition
        let mut result = t.definition_content.clone();

        // Add type definitions of dependencies
        for dep in &t
            .type_description
            .type_description_msg
            .referenced_type_descriptions
        {
            let dep_type_name = KeyExpr::try_from(&dep.type_name)
                .expect("Shouldn't happen: all type names are valid keyexpr!");
            match self.types.weight_at(&dep_type_name) {
                Some(dep_info) => {
                    result.push_str(SEPARATOR);

                    result.push_str(dep_info.kind.as_ref());
                    result.push_str(": ");
                    result.push_str(&dep_info.get_short_type_name());
                    result.push('\n');

                    result.push_str(&dep_info.definition_content);
                }
                None => {
                    tracing::warn!(
                        "Dependency {} of type {} not found in registry!",
                        dep_type_name,
                        t.full_name
                    );
                    continue;
                }
            }
        }

        result
    }
}
