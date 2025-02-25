use std::collections::HashMap;
use std::str::FromStr;

use dojo_types::primitive::Primitive;
use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};
use serde::{Deserialize, Serialize};
use starknet::core::types::{
    ContractStorageDiffItem, FromByteSliceError, FromStrError, StateDiff, StateUpdate, StorageEntry,
};
use starknet_crypto::FieldElement;

use crate::client::Error as ClientError;
use crate::proto::{self};

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct Entity {
    pub key: FieldElement,
    pub models: Vec<Model>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct Model {
    pub name: String,
    pub members: Vec<Member>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct Query {
    pub clause: Option<Clause>,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub enum Clause {
    Keys(KeysClause),
    Member(MemberClause),
    Composite(CompositeClause),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct KeysClause {
    pub model: String,
    pub keys: Vec<FieldElement>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct MemberClause {
    pub model: String,
    pub member: String,
    pub operator: ComparisonOperator,
    pub value: Value,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct CompositeClause {
    pub model: String,
    pub operator: LogicalOperator,
    pub clauses: Vec<Clause>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub enum LogicalOperator {
    And,
    Or,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub enum ComparisonOperator {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct Value {
    pub primitive_type: Primitive,
    pub value_type: ValueType,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub enum ValueType {
    String(String),
    Int(i64),
    UInt(u64),
    Bool(bool),
    Bytes(Vec<u8>),
}

impl TryFrom<proto::types::ModelMetadata> for dojo_types::schema::ModelMetadata {
    type Error = FromStrError;
    fn try_from(value: proto::types::ModelMetadata) -> Result<Self, Self::Error> {
        let schema: Ty = serde_json::from_slice(&value.schema).unwrap();
        let layout: Vec<FieldElement> = value.layout.into_iter().map(FieldElement::from).collect();
        Ok(Self {
            schema,
            layout,
            name: value.name,
            packed_size: value.packed_size,
            unpacked_size: value.unpacked_size,
            class_hash: FieldElement::from_str(&value.class_hash)?,
        })
    }
}

impl TryFrom<proto::types::WorldMetadata> for dojo_types::WorldMetadata {
    type Error = FromStrError;
    fn try_from(value: proto::types::WorldMetadata) -> Result<Self, Self::Error> {
        let models = value
            .models
            .into_iter()
            .map(|component| Ok((component.name.clone(), component.try_into()?)))
            .collect::<Result<HashMap<_, dojo_types::schema::ModelMetadata>, _>>()?;

        Ok(dojo_types::WorldMetadata {
            models,
            world_address: FieldElement::from_str(&value.world_address)?,
            world_class_hash: FieldElement::from_str(&value.world_class_hash)?,
            executor_address: FieldElement::from_str(&value.executor_address)?,
            executor_class_hash: FieldElement::from_str(&value.executor_class_hash)?,
        })
    }
}

impl From<Query> for proto::types::Query {
    fn from(value: Query) -> Self {
        Self { clause: value.clause.map(|c| c.into()), limit: value.limit, offset: value.offset }
    }
}

impl From<Clause> for proto::types::Clause {
    fn from(value: Clause) -> Self {
        match value {
            Clause::Keys(clause) => {
                Self { clause_type: Some(proto::types::clause::ClauseType::Keys(clause.into())) }
            }
            Clause::Member(clause) => {
                Self { clause_type: Some(proto::types::clause::ClauseType::Member(clause.into())) }
            }
            Clause::Composite(clause) => Self {
                clause_type: Some(proto::types::clause::ClauseType::Composite(clause.into())),
            },
        }
    }
}

impl From<KeysClause> for proto::types::KeysClause {
    fn from(value: KeysClause) -> Self {
        Self {
            model: value.model,
            keys: value.keys.iter().map(|k| k.to_bytes_be().into()).collect(),
        }
    }
}

impl TryFrom<proto::types::KeysClause> for KeysClause {
    type Error = FromByteSliceError;

    fn try_from(value: proto::types::KeysClause) -> Result<Self, Self::Error> {
        let keys = value
            .keys
            .into_iter()
            .map(|k| FieldElement::from_byte_slice_be(&k))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { model: value.model, keys })
    }
}

impl TryFrom<proto::types::Entity> for Entity {
    type Error = ClientError;
    fn try_from(entity: proto::types::Entity) -> Result<Self, Self::Error> {
        Ok(Self {
            key: FieldElement::from_byte_slice_be(&entity.key).map_err(ClientError::SliceError)?,
            models: entity
                .models
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl TryFrom<proto::types::Model> for Model {
    type Error = ClientError;
    fn try_from(model: proto::types::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            name: model.name,
            members: model
                .members
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl From<MemberClause> for proto::types::MemberClause {
    fn from(value: MemberClause) -> Self {
        Self {
            model: value.model,
            member: value.member,
            operator: value.operator as i32,
            value: Some(value.value.into()),
        }
    }
}

impl From<CompositeClause> for proto::types::CompositeClause {
    fn from(value: CompositeClause) -> Self {
        Self {
            model: value.model,
            operator: value.operator as i32,
            clauses: value.clauses.into_iter().map(|clause| clause.into()).collect(),
        }
    }
}

impl From<Value> for proto::types::Value {
    fn from(value: Value) -> Self {
        let value_type = match value.value_type {
            ValueType::String(val) => Some(proto::types::value::ValueType::StringValue(val)),
            ValueType::Int(val) => Some(proto::types::value::ValueType::IntValue(val)),
            ValueType::UInt(val) => Some(proto::types::value::ValueType::UintValue(val)),
            ValueType::Bool(val) => Some(proto::types::value::ValueType::BoolValue(val)),
            ValueType::Bytes(val) => Some(proto::types::value::ValueType::ByteValue(val)),
        };

        Self { value_type }
    }
}

impl From<proto::types::EnumOption> for EnumOption {
    fn from(option: proto::types::EnumOption) -> Self {
        EnumOption { name: option.name, ty: Ty::Tuple(vec![]) }
    }
}

impl From<proto::types::Enum> for Enum {
    fn from(r#enum: proto::types::Enum) -> Self {
        Enum {
            name: r#enum.name.clone(),
            option: Some(r#enum.option as u8),
            options: r#enum.options.into_iter().map(Into::into).collect::<Vec<_>>(),
        }
    }
}

impl TryFrom<proto::types::Struct> for Struct {
    type Error = ClientError;
    fn try_from(r#struct: proto::types::Struct) -> Result<Self, Self::Error> {
        Ok(Struct {
            name: r#struct.name,
            children: r#struct
                .children
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl From<proto::types::Struct> for proto::types::Model {
    fn from(r#struct: proto::types::Struct) -> Self {
        Self { name: r#struct.name, members: r#struct.children }
    }
}

// FIX: weird catch-22 issue - prost Enum has `try_from` trait we can use, however, using it results
// in wasm compile err about From<i32> missing. Implementing that trait results in clippy error
// about duplicate From<i32>... Workaround is to use deprecated `from_i32` and allow deprecation
// warning.
#[allow(deprecated)]
impl TryFrom<proto::types::Primitive> for Primitive {
    type Error = ClientError;
    fn try_from(primitive: proto::types::Primitive) -> Result<Self, Self::Error> {
        let primitive_type = primitive.r#type;
        let value_type = primitive
            .value
            .ok_or(ClientError::MissingExpectedData)?
            .value_type
            .ok_or(ClientError::MissingExpectedData)?;

        let primitive = match &value_type {
            proto::types::value::ValueType::BoolValue(bool) => Primitive::Bool(Some(*bool)),
            proto::types::value::ValueType::UintValue(int) => {
                match proto::types::PrimitiveType::from_i32(primitive_type) {
                    Some(proto::types::PrimitiveType::U8) => Primitive::U8(Some(*int as u8)),
                    Some(proto::types::PrimitiveType::U16) => Primitive::U16(Some(*int as u16)),
                    Some(proto::types::PrimitiveType::U32) => Primitive::U32(Some(*int as u32)),
                    Some(proto::types::PrimitiveType::U64) => Primitive::U64(Some(*int)),
                    Some(proto::types::PrimitiveType::Usize) => Primitive::USize(Some(*int as u32)),
                    _ => return Err(ClientError::UnsupportedType),
                }
            }
            proto::types::value::ValueType::ByteValue(bytes) => {
                match proto::types::PrimitiveType::from_i32(primitive_type) {
                    Some(proto::types::PrimitiveType::U128)
                    | Some(proto::types::PrimitiveType::Felt252)
                    | Some(proto::types::PrimitiveType::ClassHash)
                    | Some(proto::types::PrimitiveType::ContractAddress) => {
                        Primitive::Felt252(Some(
                            FieldElement::from_byte_slice_be(bytes)
                                .map_err(ClientError::SliceError)?,
                        ))
                    }
                    _ => return Err(ClientError::UnsupportedType),
                }
            }
            proto::types::value::ValueType::StringValue(_string) => {
                match proto::types::PrimitiveType::from_i32(primitive_type) {
                    Some(proto::types::PrimitiveType::U256) => {
                        // TODO: Handle u256
                        Primitive::U256(None)
                    }
                    _ => return Err(ClientError::UnsupportedType),
                }
            }
            _ => {
                return Err(ClientError::UnsupportedType);
            }
        };

        Ok(primitive)
    }
}

impl TryFrom<proto::types::Ty> for Ty {
    type Error = ClientError;
    fn try_from(ty: proto::types::Ty) -> Result<Self, Self::Error> {
        match ty.ty_type.ok_or(ClientError::MissingExpectedData)? {
            proto::types::ty::TyType::Primitive(primitive) => {
                Ok(Ty::Primitive(primitive.try_into()?))
            }
            proto::types::ty::TyType::Struct(r#struct) => Ok(Ty::Struct(r#struct.try_into()?)),
            proto::types::ty::TyType::Enum(r#enum) => Ok(Ty::Enum(r#enum.into())),
        }
    }
}

impl TryFrom<proto::types::Member> for Member {
    type Error = ClientError;
    fn try_from(member: proto::types::Member) -> Result<Self, Self::Error> {
        Ok(Member {
            name: member.name,
            ty: member.ty.ok_or(ClientError::MissingExpectedData)?.try_into()?,
            key: member.key,
        })
    }
}

impl TryFrom<proto::types::StorageEntry> for StorageEntry {
    type Error = FromStrError;
    fn try_from(value: proto::types::StorageEntry) -> Result<Self, Self::Error> {
        Ok(Self {
            key: FieldElement::from_str(&value.key)?,
            value: FieldElement::from_str(&value.value)?,
        })
    }
}

impl TryFrom<proto::types::StorageDiff> for ContractStorageDiffItem {
    type Error = FromStrError;
    fn try_from(value: proto::types::StorageDiff) -> Result<Self, Self::Error> {
        Ok(Self {
            address: FieldElement::from_str(&value.address)?,
            storage_entries: value
                .storage_entries
                .into_iter()
                .map(|entry| entry.try_into())
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl TryFrom<proto::types::EntityDiff> for StateDiff {
    type Error = FromStrError;
    fn try_from(value: proto::types::EntityDiff) -> Result<Self, Self::Error> {
        Ok(Self {
            nonces: vec![],
            declared_classes: vec![],
            replaced_classes: vec![],
            deployed_contracts: vec![],
            deprecated_declared_classes: vec![],
            storage_diffs: value
                .storage_diffs
                .into_iter()
                .map(|diff| diff.try_into())
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl TryFrom<proto::types::EntityUpdate> for StateUpdate {
    type Error = FromStrError;
    fn try_from(value: proto::types::EntityUpdate) -> Result<Self, Self::Error> {
        Ok(Self {
            new_root: FieldElement::ZERO,
            old_root: FieldElement::ZERO,
            block_hash: FieldElement::from_str(&value.block_hash)?,
            state_diff: value.entity_diff.expect("must have").try_into()?,
        })
    }
}
