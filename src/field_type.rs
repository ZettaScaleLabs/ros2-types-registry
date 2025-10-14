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

use serde::{
    de::{Error, Visitor},
    Deserialize, Deserializer, Serialize,
};
use std::fmt;
use std::str::FromStr;
use strum::{EnumString, FromRepr, VariantNames};

// Structure compliant FIELD_TYPE constants defined in
// https://github.com/ros2/rosidl/blob/kilted/rosidl_generator_type_description/rosidl_generator_type_description/__init__.py
#[derive(Debug, Clone, Copy, EnumString, FromRepr, Serialize, PartialEq, Eq, VariantNames)]
#[repr(u64)]
pub enum FieldTypeId {
    NotSet = 0,

    // Nested type defined in other .msg/.idl files.
    NestedType = 1,

    // Basic Types
    // Integer Types
    Int8 = 2,
    UInt8 = 3,
    Int16 = 4,
    UInt16 = 5,
    Int32 = 6,
    UInt32 = 7,
    Int64 = 8,
    UInt64 = 9,

    // Floating-Point Types
    Float = 10,
    Double = 11,
    LongDouble = 12,

    // Char and WChar Types
    Char = 13,
    WChar = 14,

    // Boolean Type
    Boolean = 15,

    // Byte/Octet Type
    Byte = 16,

    // String Types
    String = 17,
    WString = 18,

    // Fixed String Types
    FixedString = 19,
    FixedWString = 20,

    // Bounded String Types
    BoundedString = 21,
    BoundedWString = 22,

    // Fixed Sized Array Types
    NestedTypeArray = 49,
    Int8Array = 50,
    UInt8Array = 51,
    Int16Array = 52,
    UInt16Array = 53,
    Int32Array = 54,
    UInt32Array = 55,
    Int64Array = 56,
    UInt64Array = 57,
    FloatArray = 58,
    DoubleArray = 59,
    LongDoubleArray = 60,
    CharArray = 61,
    WCharArray = 62,
    BooleanArray = 63,
    ByteArray = 64,
    StringArray = 65,
    WStringArray = 66,
    FixedStringArray = 67,
    FixedWStringArray = 68,
    BoundedStringArray = 69,
    BoundedWStringArray = 70,

    // Bounded Sequence Types
    NestedTypeBoundedSequence = 97,
    Int8BoundedSequence = 98,
    UInt8BoundedSequence = 99,
    Int16BoundedSequence = 100,
    UInt16BoundedSequence = 101,
    Int32BoundedSequence = 102,
    UInt32BoundedSequence = 103,
    Int64BoundedSequence = 104,
    UInt64BoundedSequence = 105,
    FloatBoundedSequence = 106,
    DoubleBoundedSequence = 107,
    LongDoubleBoundedSequence = 108,
    CharBoundedSequence = 109,
    WCharBoundedSequence = 110,
    BooleanBoundedSequence = 111,
    ByteBoundedSequence = 112,
    StringBoundedSequence = 113,
    WStringBoundedSequence = 114,
    FixedStringBoundedSequence = 115,
    FixedWStringBoundedSequence = 116,
    BoundedStringBoundedSequence = 117,
    BoundedWStringBoundedSequence = 118,

    // Unbounded Sequence Types
    NestedTypeUnboundedSequence = 145,
    Int8UnboundedSequence = 146,
    UInt8UnboundedSequence = 147,
    Int16UnboundedSequence = 148,
    UInt16UnboundedSequence = 149,
    Int32UnboundedSequence = 150,
    UInt32UnboundedSequence = 151,
    Int64UnboundedSequence = 152,
    UInt64UnboundedSequence = 153,
    FloatUnboundedSequence = 154,
    DoubleUnboundedSequence = 155,
    LongDoubleUnboundedSequence = 156,
    CharUnboundedSequence = 157,
    WCharUnboundedSequence = 158,
    BooleanUnboundedSequence = 159,
    ByteUnboundedSequence = 160,
    StringUnboundedSequence = 161,
    WStringUnboundedSequence = 162,
    FixedStringUnboundedSequence = 163,
    FixedWStringUnboundedSequence = 164,
    BoundedStringUnboundedSequence = 165,
    BoundedWStringUnboundedSequence = 166,
}

struct FieldTypeIdVisitor;

impl<'de> Visitor<'de> for FieldTypeIdVisitor {
    type Value = FieldTypeId;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string or an integer")
    }

    fn visit_str<E>(self, value: &str) -> Result<FieldTypeId, E>
    where
        E: Error,
    {
        FieldTypeId::from_str(value)
            .map_err(|_| Error::unknown_variant(value, FieldTypeId::VARIANTS))
    }

    fn visit_u64<E>(self, value: u64) -> Result<FieldTypeId, E>
    where
        E: Error,
    {
        FieldTypeId::from_repr(value).ok_or(Error::invalid_value(
            serde::de::Unexpected::Unsigned(value),
            &"a valid FieldTypeId integer value",
        ))
    }
}

impl<'de> Deserialize<'de> for FieldTypeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(FieldTypeIdVisitor)
        // deserialize_field_type_id(deserializer)
    }
}
