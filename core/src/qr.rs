use url::Url;

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct DeviceApprovalQrPayload {
    pub owner_input: String,
    pub device_input: String,
}

const DEVICE_APPROVAL_QR_SCHEME: &str = "ndrdemo";
const DEVICE_APPROVAL_QR_HOST: &str = "device-link";

#[uniffi::export]
pub fn encode_device_approval_qr(owner_input: String, device_input: String) -> String {
    let owner = owner_input.trim();
    let device = device_input.trim();
    if owner.is_empty() || device.is_empty() {
        return String::new();
    }

    let mut url = Url::parse("ndrdemo://device-link").expect("valid device approval base url");
    url.query_pairs_mut()
        .append_pair("owner", owner)
        .append_pair("device", device);
    url.to_string()
}

#[uniffi::export]
pub fn decode_device_approval_qr(raw: String) -> Option<DeviceApprovalQrPayload> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parsed = Url::parse(trimmed).ok()?;
    if !parsed
        .scheme()
        .eq_ignore_ascii_case(DEVICE_APPROVAL_QR_SCHEME)
    {
        return None;
    }
    if !parsed
        .host_str()
        .is_some_and(|host| host.eq_ignore_ascii_case(DEVICE_APPROVAL_QR_HOST))
    {
        return None;
    }

    let mut owner_input = None;
    let mut device_input = None;
    for (key, value) in parsed.query_pairs() {
        match key.as_ref() {
            "owner" => {
                let trimmed_value = value.trim();
                if !trimmed_value.is_empty() {
                    owner_input = Some(trimmed_value.to_string());
                }
            }
            "device" => {
                let trimmed_value = value.trim();
                if !trimmed_value.is_empty() {
                    device_input = Some(trimmed_value.to_string());
                }
            }
            _ => {}
        }
    }

    Some(DeviceApprovalQrPayload {
        owner_input: owner_input?,
        device_input: device_input?,
    })
}

#[cfg(test)]
mod tests {
    use super::{decode_device_approval_qr, encode_device_approval_qr, DeviceApprovalQrPayload};

    #[test]
    fn device_approval_qr_round_trip() {
        let encoded = encode_device_approval_qr("npub-owner".into(), "npub-device".into());
        let decoded = decode_device_approval_qr(encoded).expect("decode");
        assert_eq!(
            decoded,
            DeviceApprovalQrPayload {
                owner_input: "npub-owner".into(),
                device_input: "npub-device".into(),
            }
        );
    }

    #[test]
    fn device_approval_qr_rejects_wrong_inputs() {
        assert!(decode_device_approval_qr("".into()).is_none());
        assert!(decode_device_approval_qr("npub1plainvalue".into()).is_none());
        assert!(decode_device_approval_qr("https://example.com".into()).is_none());
        assert!(
            decode_device_approval_qr("ndrdemo://device-link?owner=npub1owneronly".into())
                .is_none()
        );
        assert!(
            decode_device_approval_qr("ndrdemo://device-link?device=npub1deviceonly".into())
                .is_none()
        );
    }
}
