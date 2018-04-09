import argparse
import tempfile
import subprocess

from google.protobuf import descriptor_pb2


_FDP = descriptor_pb2.FieldDescriptorProto


HEADER_TEMPLATE = '''
// automatically generated from {path}

#![allow(unused_variables)]
#![allow(unused_mut)]

extern crate serde_json;

use pblite;
use pblite::{{Enum, Message}};
'''.strip('\n')
STRUCT_TEMPLATE = '''
#[derive(Debug, Default, PartialEq, Clone, Serialize)]
pub struct {name} {{
{fields}
}}
impl Message for {name} {{
    fn get_name(&self) -> &str {{
        "{name}"
    }}
    fn set_field(&mut self, number: usize, field_value: &serde_json::Value) -> pblite::Result<()> {{
        match number {{
{matches}
            _ => {{}}
        }};
        Ok(())
    }}
}}
'''.rstrip('\n')
FIELD_TEMPLATE = '''
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "{raw_name}")]
    pub {name}: {type},
'''.strip('\n')
OPTIONAL_MATCH_TEMPLATE = '''
            {number} => self.{name} = pblite::read_optional(field_value, &{method})?,
'''.strip('\n')
REPEATED_MATCH_TEMPLATE = '''
            {number} => self.{name} = pblite::read_array(field_value, &{method})?,
'''.strip('\n')
ENUM_TEMPLATE = '''
#[derive(Debug, PartialEq, Clone, Serialize)]
pub enum {name} {{
{values}
}}
impl Enum for {name} {{
    fn from_u32(value: u32) -> pblite::Result<Self> {{
        match value {{
{matches}
            _ => Ok({name}::{default_value_name}),
        }}
    }}
}}
'''.rstrip('\n')
ENUM_VALUE_TEMPLATE = '    {value_name},'
ENUM_MATCH_TEMPLATE = '            {number} => Ok({name}::{value_name}),'


def _gen_struct(descriptor):
    return STRUCT_TEMPLATE.format(
        name=descriptor.name,
        fields='\n'.join(_gen_field(field) for field in descriptor.field),
        matches='\n'.join(_gen_match(field) for field in descriptor.field),
    )


def _gen_field(field):
    return FIELD_TEMPLATE.format(
        raw_name=field.name,
        name=sanitize_name(field.name),
        type=get_rust_type(field),
    )


def _gen_match(field):
    return {
        _FDP.LABEL_OPTIONAL: OPTIONAL_MATCH_TEMPLATE,
        _FDP.LABEL_REPEATED: REPEATED_MATCH_TEMPLATE,
    }[field.label].format(
        number=field.number - 1,
        name=sanitize_name(field.name),
        method=get_read_method(field),
    )


def _gen_enum(descriptor):
    return ENUM_TEMPLATE.format(
        name=descriptor.name,
        values='\n'.join(_gen_enum_value(value) for value in descriptor.value),
        matches='\n'.join(_gen_enum_match(descriptor, value) for value in descriptor.value),
        default_value_name=get_enum_name(descriptor.value[0].name),
    )

def _gen_enum_value(value):
    return ENUM_VALUE_TEMPLATE.format(
        value_name=get_enum_name(value.name)
    )

def _gen_enum_match(descriptor, value):
    return ENUM_MATCH_TEMPLATE.format(
        number=value.number,
        name=descriptor.name,
        value_name=get_enum_name(value.name),
    )


def get_rust_type(field):
    rust_type = {
        _FDP.TYPE_STRING: 'String',
        _FDP.TYPE_BYTES: 'Vec<u8>',
        _FDP.TYPE_BOOL: 'bool',
        _FDP.TYPE_UINT32: 'u32',
        _FDP.TYPE_UINT64: 'u64',
        _FDP.TYPE_DOUBLE: 'f64',
        _FDP.TYPE_MESSAGE: field.type_name.lstrip('.'),
        _FDP.TYPE_ENUM: field.type_name.lstrip('.'),
    }[field.type]
    return {
        _FDP.LABEL_REPEATED: f'Option<Vec<{rust_type}>>',
        _FDP.LABEL_OPTIONAL: f'Option<{rust_type}>',
        _FDP.LABEL_REQUIRED: rust_type,
    }[field.label]


def get_read_method(field):
    type_name = {
        _FDP.TYPE_STRING: 'string',
        _FDP.TYPE_BYTES: 'bytes',
        _FDP.TYPE_BOOL: 'bool',
        _FDP.TYPE_UINT32: 'uint32',
        _FDP.TYPE_UINT64: 'uint64',
        _FDP.TYPE_DOUBLE: 'double',
        _FDP.TYPE_MESSAGE: 'message',
        _FDP.TYPE_ENUM: 'enum',
    }[field.type]
    return f'pblite::read_{type_name}'


def get_file_descriptor_proto(proto_file_path):
    out_file = tempfile.mkstemp()[1]
    subprocess.check_output([
        'protoc', '--include_source_info', '--descriptor_set_out',
        out_file, proto_file_path
    ])
    with open(out_file, 'rb') as proto_file:
        file_descriptor_proto = descriptor_pb2.FileDescriptorSet.FromString(
            proto_file.read()
        ).file[0]
    return file_descriptor_proto


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('protofilepath')
    args = parser.parse_args()

    file_descriptor_proto = get_file_descriptor_proto(args.protofilepath)

    print(HEADER_TEMPLATE.format(path=args.protofilepath))

    for descriptor_proto in file_descriptor_proto.message_type:
        print(_gen_struct(descriptor_proto))

    for enum_descriptor_proto in file_descriptor_proto.enum_type:
        print(_gen_enum(enum_descriptor_proto))


def get_enum_name(name):
    words = name.split('_')
    return ''.join(word[0].upper() + word[1:].lower() for word in words)


assert get_enum_name('EXAMPLE_ENUM_NAME') == 'ExampleEnumName'


RUST_KEYWORDS = {'type'}


def sanitize_name(name):
    if name in RUST_KEYWORDS:
        return f'{name}_pb'
    else:
        return name


assert sanitize_name('foo') == 'foo'
assert sanitize_name('type') == 'type_pb'


if __name__ == '__main__':
    main()
