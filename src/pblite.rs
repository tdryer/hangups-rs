extern crate base64;
extern crate serde_json;

use serde_json::Value;

error_chain!{
    errors {
        InvalidMessage(name: String) {
            description("invalid message"),
            display("message '{}' is invalid", name),
        }
        InvalidField(number: usize) {
            description("invalid field"),
            display("field {} is invalid", number),
        }
        ExpectedValue(expected: &'static str, actual: Value) {
            description("expected value"),
            display("expected {} value but got: '{}'", expected, actual),
        }
    }
}

fn expected_value(expected: &'static str, actual: &Value) -> Error {
    ErrorKind::ExpectedValue(expected, actual.clone()).into()
}

pub trait Message: Sized + Default {
    fn get_name(&self) -> &str;

    fn set_field(&mut self, number: usize, field_value: &Value) -> Result<()>;

    fn from_vec(array: &Vec<Value>) -> Result<Self> {
        let mut message = Self::default();
        for (number, field_value) in array.iter().enumerate() {
            message
                .set_field(number, field_value)
                .chain_err(|| ErrorKind::InvalidField(number))
                .chain_err(|| ErrorKind::InvalidMessage(message.get_name().to_owned()))?;
        }
        Ok(message)
    }

    fn from_pblite(text: &str) -> Result<Self> {
        serde_json::from_str(text)
            .chain_err(|| "invalid json")
            .and_then(|v| read_message(&v))
            .chain_err(|| ErrorKind::InvalidMessage(Self::default().get_name().to_owned()))
    }
}

pub trait Enum: Sized {
    fn from_u32(value: u32) -> Result<Self>;
}

pub fn read_string(value: &Value) -> Result<String> {
    value
        .as_str()
        .ok_or(expected_value("string", value))
        .map(|s| s.to_owned())
}

pub fn read_bool(value: &Value) -> Result<bool> {
    value
        .as_u64()
        .ok_or(expected_value("u64", value))
        .and_then(|n| match n {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(expected_value("0 or 1", value)),
        })
}

pub fn read_enum<E: Enum>(value: &Value) -> Result<E> {
    E::from_u32(read_uint32(value)?)
}

pub fn read_uint32(value: &Value) -> Result<u32> {
    value
        .as_u64()
        .ok_or(expected_value("u64", value))
        .and_then(|n| match n {
            n if n > ::std::u32::MAX as u64 => Err(expected_value("u32", value)),
            _ => Ok(n as u32),
        })
}

pub fn read_uint64(value: &Value) -> Result<u64> {
    match value {
        // uint64 may be converted to string since JavaScript numbers lack precision.
        &Value::Number(ref n) => n.as_u64().ok_or(expected_value("u64", value)),
        &Value::String(ref s) => s.parse::<u64>().or(Err(expected_value("u64", value))),
        _ => Err(expected_value("number or string", value)),
    }
}

pub fn read_double(value: &Value) -> Result<f64> {
    value.as_f64().ok_or(expected_value("f64", value))
}

pub fn read_bytes(value: &Value) -> Result<Vec<u8>> {
    value
        .as_str()
        .ok_or(expected_value("string", value))
        .and_then(|s| base64::decode(s).or(Err(expected_value("base64", value))))
}

pub fn read_message<M: Message>(value: &Value) -> Result<M> {
    value
        .as_array()
        .ok_or(expected_value("array", value))
        .and_then(|vec| M::from_vec(vec))
}

pub fn read_array<A>(value: &Value, read_elem: &Fn(&Value) -> Result<A>) -> Result<Option<Vec<A>>> {
    match value {
        &Value::Array(ref vec) => Ok(Some(vec.iter()
            .map(|val| read_elem(val))
            .collect::<Result<_>>()?)),
        &Value::Null => Ok(None),
        _ => Err(expected_value("array", value)),
    }
}

pub fn read_optional<A>(value: &Value, read_inner: &Fn(&Value) -> Result<A>) -> Result<Option<A>> {
    match value {
        &Value::Null => Ok(None),
        value => Ok(Some(read_inner(value)?)),
    }
}

#[cfg(test)]
mod tests {

    use example;
    use pblite::Message;

    #[test]
    fn test_i18n_data() {
        let i18n_data = example::I18nData::from_pblite("[\"CA\", 1, 123]").unwrap();
        assert_eq!(
            i18n_data,
            example::I18nData {
                region_code: Some("CA".to_owned()),
                is_valid: Some(true),
                country_code: Some(123),
            }
        );
    }

    #[test]
    fn test_i18n_data_empty() {
        let i18n_data = example::I18nData::from_pblite("[]").unwrap();
        assert_eq!(
            i18n_data,
            example::I18nData {
                region_code: None,
                is_valid: None,
                country_code: None,
            }
        );
    }

    #[test]
    fn test_phone_number() {
        let phone_number =
            example::PhoneNumber::from_pblite("[\"16067624137\",[\"CA\", 1, 123]]").unwrap();
        assert_eq!(
            phone_number,
            example::PhoneNumber {
                e164: Some("16067624137".to_owned()),
                i18n_data: Some(example::I18nData {
                    region_code: Some("CA".to_owned()),
                    is_valid: Some(true),
                    country_code: Some(123),
                }),
            }
        );
    }

    #[test]
    fn test_empty() {
        let empty = example::Empty::from_pblite("[]").unwrap();
        assert_eq!(empty, example::Empty {});
    }

    #[test]
    fn test_empty_unexpected_field() {
        let empty = example::Empty::from_pblite("[1]").unwrap();
        assert_eq!(empty, example::Empty {});
    }

    #[test]
    fn test_invalid_json() {
        let empty = example::Empty::from_pblite("[");
        assert_eq!(
            empty.err().expect("expected error").description(),
            "invalid message"
        );
    }

    #[test]
    fn test_null_json() {
        let empty = example::Empty::from_pblite("null");
        println!("{:?}", empty);
        assert_eq!(
            empty.err().expect("expected error").description(),
            "invalid message"
        );
    }

    #[test]
    fn test_expected_array() {
        let empty = example::Empty::from_pblite("1");
        assert_eq!(
            empty.err().expect("expected error").description(),
            "invalid message"
        );
    }

    #[test]
    fn test_expected_array_nested() {
        let phone_number = example::PhoneNumber::from_pblite("[null,1]");
        assert_eq!(
            phone_number.err().expect("expected error").description(),
            "invalid message"
        );
    }

    #[test]
    fn test_string_expected_string() {
        let phone_number = example::PhoneNumber::from_pblite("[1]");
        assert_eq!(
            phone_number.err().expect("expected error").description(),
            "invalid message"
        );
    }

    #[test]
    fn test_bytes_expected_string() {
        let example = example::Example::from_pblite("[null,null,null,null,null,1]");
        assert_eq!(
            example.err().expect("expected error").description(),
            "invalid message"
        );
    }

    #[test]
    fn test_expected_bool() {
        let i18n_data = example::I18nData::from_pblite("[null,\"foo\"]");
        assert_eq!(
            i18n_data.err().expect("expected error").description(),
            "invalid message"
        );
    }

    #[test]
    fn test_uint32_expected_number() {
        let i18n_data = example::I18nData::from_pblite("[null,null,\"\"]");
        assert_eq!(
            i18n_data.err().expect("expected error").description(),
            "invalid message"
        );
    }

    #[test]
    fn test_uint64_expected_string() {
        let example = example::Example::from_pblite("[null,false]");
        assert_eq!(
            example.err().expect("expected error").description(),
            "invalid message"
        );
    }

    #[test]
    fn test_double_expected_number() {
        let example = example::Example::from_pblite("[\"\"]");
        assert_eq!(
            example.err().expect("expected error").description(),
            "invalid message"
        );
    }

    #[test]
    fn test_enum_expected_number() {
        let example = example::Example::from_pblite("[null,null,null,null,null,null,\"\"]");
        assert_eq!(
            example.err().expect("expected error").description(),
            "invalid message"
        );
    }

    #[test]
    fn test_enum_unknown_value() {
        let example = example::Example::from_pblite("[null,null,null,null,null,null,100]");
        assert_matches!(
            example.unwrap().test_enum,
            Some(example::ExampleEnum::ExampleEnumValue1)
        );
    }

    #[test]
    fn test_uint64_as_string() {
        let example = example::Example::from_pblite("[null,\"64\"]");
        assert_matches!(example.unwrap().test_uint64, Some(64));
    }

    #[test]
    fn test_uint64_as_number() {
        let example = example::Example::from_pblite("[null,64]");
        assert_matches!(example.unwrap().test_uint64, Some(64));
    }

    #[test]
    fn test_all_types() {
        let example = example::Example::from_pblite("[3.14159,\"64\",32,1,\"foo\",\"AAEC\",2,[],[3.14159,1.1],[64,65],[32,33],[1,0],[\"foo\",\"bar\"],[\"AAEC\",\"AgEA\"],[2,3],[[],[]]]").unwrap();
        assert_eq!(
            example,
            example::Example {
                test_double: Some(3.14159),
                test_uint64: Some(64),
                test_uint32: Some(32),
                test_bool: Some(true),
                test_string: Some("foo".to_owned()),
                test_bytes: Some(vec![0, 1, 2]),
                test_enum: Some(example::ExampleEnum::ExampleEnumValue2),
                test_message: Some(example::Empty {}),

                test_repeated_double: Some(vec![3.14159, 1.1]),
                test_repeated_uint64: Some(vec![64, 65]),
                test_repeated_uint32: Some(vec![32, 33]),
                test_repeated_bool: Some(vec![true, false]),
                test_repeated_string: Some(vec!["foo".to_owned(), "bar".to_owned()]),
                test_repeated_bytes: Some(vec![vec![0, 1, 2], vec![2, 1, 0]]),
                test_repeated_enum: Some(vec![
                    example::ExampleEnum::ExampleEnumValue2,
                    example::ExampleEnum::ExampleEnumValue3,
                ]),
                test_repeated_message: Some(vec![example::Empty {}, example::Empty {}]),
            }
        );
    }

    #[test]
    fn test_all_types_null() {
        let example = example::Example::from_pblite("[null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null]").unwrap();
        assert_eq!(
            example,
            example::Example {
                test_double: None,
                test_uint64: None,
                test_uint32: None,
                test_bool: None,
                test_string: None,
                test_bytes: None,
                test_enum: None,
                test_message: None,

                test_repeated_double: None,
                test_repeated_uint64: None,
                test_repeated_uint32: None,
                test_repeated_bool: None,
                test_repeated_string: None,
                test_repeated_bytes: None,
                test_repeated_enum: None,
                test_repeated_message: None,
            }
        );
    }
}
