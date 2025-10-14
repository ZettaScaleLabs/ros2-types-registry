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
//

use crate::field_type::FieldTypeId;
use serde::{Deserialize, Serialize};

// Structure compliant with the rso2cli JSON schema defined in
// https://github.com/ros2/rosidl/blob/kilted/rosidl_generator_type_description/resource/HashedTypeDescription.schema.json
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HashedTypeDescription {
    pub type_description_msg: TypeDescription,
    pub type_hashes: Vec<TypeNameAndHash>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypeNameAndHash {
    pub type_name: String,
    pub hash_string: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypeDescription {
    pub type_description: IndividualTypeDescription,
    pub referenced_type_descriptions: Vec<IndividualTypeDescription>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndividualTypeDescription {
    pub type_name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub default_value: Option<String>,
    pub name: String,
    pub r#type: FieldType,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldType {
    pub type_id: FieldTypeId,
    pub capacity: u32,
    pub string_capacity: u32,
    pub nested_type_name: String,
}
