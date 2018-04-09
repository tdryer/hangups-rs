// automatically generated from src/example.proto

#![allow(unused_variables)]

extern crate serde_json;

use pblite::{Message,MessageReader,Result};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct PhoneNumber {
    pub e164: Option<String>,
    pub i18n_data: Option<I18nData>,
}
impl Message for PhoneNumber {
    fn from_value(value: &serde_json::Value) -> Result<Self> {
        let reader = MessageReader::from_value(value)?;
        Ok(PhoneNumber {
            e164: reader.read_string(1)?,
            i18n_data: reader.read_message(2)?,
        })
   }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct I18nData {
    pub region_code: Option<String>,
    pub is_valid: Option<bool>,
    pub country_code: Option<u32>,
}
impl Message for I18nData {
    fn from_value(value: &serde_json::Value) -> Result<Self> {
        let reader = MessageReader::from_value(value)?;
        Ok(I18nData {
            region_code: reader.read_string(1)?,
            is_valid: reader.read_bool(2)?,
            country_code: reader.read_uint32(3)?,
        })
   }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Empty {
}
impl Message for Empty {
    fn from_value(value: &serde_json::Value) -> Result<Self> {
        let reader = MessageReader::from_value(value)?;
        Ok(Empty {
        })
   }
}
