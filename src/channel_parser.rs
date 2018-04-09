use serde_json;
use hangouts;
use pblite::Message;

// TODO: Come up with better names for everything here.

error_chain!{}

#[derive(Debug, PartialEq)]
pub struct ContainerArray {
    pub channel_arrays: Vec<ChannelArray>,
}
impl ContainerArray {
    pub fn parse(string: &str) -> Result<Self> {
        let mut value =
            serde_json::from_str::<serde_json::Value>(string).chain_err(|| "failed to parse json")?;
        let array = value.as_array_mut().chain_err(|| "container is not array")?;
        let channel_arrays = array
            .drain(..)
            .map(|val| ChannelArray::parse(val))
            .collect::<Result<Vec<ChannelArray>>>()?;
        Ok(Self {
            channel_arrays: channel_arrays,
        })
    }
}

#[derive(Debug, PartialEq)]
pub struct ChannelArray {
    pub array_id: u64,
    pub payload: ChannelPayload,
}
impl ChannelArray {
    pub fn parse(value: serde_json::Value) -> Result<Self> {
        let inner_array = value.as_array().chain_err(|| "expected array")?;
        let array_id = inner_array
            .get(0)
            .and_then(|val| val.as_u64())
            .chain_err(|| "expected array id")?;
        let raw_data_array = inner_array.get(1).chain_err(|| "expected data array")?;
        let payload = match &raw_data_array[0] {
            &serde_json::Value::String(ref s) if s == "noop" => ChannelPayload::Noop,
            &serde_json::Value::Object(ref m) => {
                let wrapper_str = m.get("p")
                    .and_then(|val| val.as_str())
                    .chain_err(|| "expected string")?;
                let wrapper_val = serde_json::from_str::<serde_json::Value>(wrapper_str)
                    .chain_err(|| "failed to parse json")?;
                let new_client_id_array = wrapper_val.get("3");
                let new_proto_array = wrapper_val.get("2");

                if new_client_id_array.is_some() {
                    let client_id = new_client_id_array
                        .and_then(|value| value.get("2"))
                        .and_then(|value| value.as_str())
                        .chain_err(|| "failed to parse client id")?;
                    ChannelPayload::NewClientID(client_id.to_owned())
                } else if new_proto_array.is_some() {
                    let pblite_str = new_proto_array
                        .and_then(|obj| obj.get("2"))
                        .and_then(|val| val.as_str())
                        .chain_err(|| "failed to extract pblite string")?;
                    let mut pblite_val = serde_json::from_str::<serde_json::Value>(pblite_str)
                        .chain_err(|| "failed to parse pblite string as json")?;
                    let pblite_vec = pblite_val
                        .as_array_mut()
                        .chain_err(|| "pblite string is not array")?;
                    // Remove the pblite "header"
                    // TODO: This can panic, consider moving it to pblite instead.
                    pblite_vec.remove(0);
                    let batch_update = hangouts::BatchUpdate::from_vec(&pblite_vec)
                        .chain_err(|| "failed to parse BatchUpdate")?;
                    ChannelPayload::BatchUpdate(batch_update)
                } else {
                    ChannelPayload::Unknown
                }
            }
            _ => ChannelPayload::Unknown,
        };
        Ok(ChannelArray {
            array_id: array_id,
            payload: payload,
        })
    }
}

#[derive(Debug, PartialEq)]
pub enum ChannelPayload {
    Noop,
    Unknown,
    NewClientID(String),
    BatchUpdate(hangouts::BatchUpdate),
}

#[cfg(test)]
mod tests {

    use hangouts;
    use channel_parser::{ChannelArray, ChannelPayload, ContainerArray};

    #[test]
    fn test_parse_batch_update() {
        let proto = "[[5,[{\"p\":\"{\\\"1\\\":{\\\"1\\\":{\\\"1\\\":{\\\"1\\\":1,\\\"2\\\":1}},\\\"4\\\":\\\"1521002546965\\\",\\\"5\\\":\\\"S4\\\"},\\\"2\\\":{\\\"1\\\":{\\\"1\\\":\\\"babel\\\",\\\"2\\\":\\\"conserver.google.com\\\"},\\\"2\\\":\\\"[\\\\\\\"cbu\\\\\\\",[[[0,null,\\\\\\\"173955572810212329\\\\\\\",null,1521002546845000]\\\\n,null,null,null,null,null,null,null,null,null,null,null,null,[[[\\\\\\\"lcsw_hangouts_E5EC3DFB\\\\\\\",\\\\\\\"2DA6A88554072FCA\\\\\\\"]\\\\n,30]\\\\n]\\\\n]\\\\n]\\\\n]\\\\n\\\"}}\"}]]\n]\n";
        let batch_update = hangouts::BatchUpdate::default();
        let mut container_array = ContainerArray::parse(proto).unwrap();
        // Make the equality comparison simpler:
        container_array.channel_arrays[0].payload = ChannelPayload::BatchUpdate(batch_update);
        assert_eq!(
            container_array,
            ContainerArray {
                channel_arrays: vec![
                    ChannelArray {
                        array_id: 5,
                        payload: ChannelPayload::BatchUpdate(hangouts::BatchUpdate::default()),
                    },
                ],
            }
        );
    }

    #[test]
    fn test_parse_new_client_id() {
        let client_id = "[[2,[{\"p\":\"{\\\"1\\\":{\\\"1\\\":{\\\"1\\\":{\\\"1\\\":1,\\\"2\\\":1}},\\\"4\\\":\\\"1521086182842\\\",\\\"5\\\":\\\"S1\\\"},\\\"3\\\":{\\\"1\\\":{\\\"1\\\":1},\\\"2\\\":\\\"lcsw_hangouts_00BBCF28\\\"}}\"}]]\n]\n";
        let container_array = ContainerArray::parse(client_id).unwrap();
        assert_eq!(
            container_array,
            ContainerArray {
                channel_arrays: vec![
                    ChannelArray {
                        array_id: 2,
                        payload: ChannelPayload::NewClientID("lcsw_hangouts_00BBCF28".to_owned()),
                    },
                ],
            }
        );
    }

    #[test]
    fn test_parse_noop() {
        let noop = "[[6,[\"noop\"]\n]\n]\n";
        let container_array = ContainerArray::parse(noop).unwrap();
        assert_eq!(
            container_array,
            ContainerArray {
                channel_arrays: vec![
                    ChannelArray {
                        array_id: 6,
                        payload: ChannelPayload::Noop,
                    },
                ],
            }
        );
    }
}
