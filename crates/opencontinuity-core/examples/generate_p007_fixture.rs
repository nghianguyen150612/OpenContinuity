use opencontinuity_core::gatt_wire::{ControlCommand, encode_control, encode_frame};
use serde_json::json;
use sha2::{Digest, Sha256};

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn main() {
    let id = *b"01234567";
    let challenge: Vec<u8> = [vec![1], id.to_vec(), (0u8..16).collect()].concat();
    let payload = b"hello rust";
    let first = encode_frame(id, 0, true, false, payload.len(), &payload[..5]).unwrap();
    let second = encode_frame(id, 1, false, true, 0, &payload[5..]).unwrap();
    let complete = encode_control(id, ControlCommand::Complete);
    let cancel = encode_control(id, ControlCommand::Cancel);
    let success = [
        vec![1, 0],
        id.to_vec(),
        (payload.len() as u16).to_le_bytes().to_vec(),
        Sha256::digest(payload).to_vec(),
    ]
    .concat();
    let cancel_response = [
        vec![1, 2],
        id.to_vec(),
        (9u16).to_le_bytes().to_vec(),
        Sha256::digest(b"cancelled").to_vec(),
    ]
    .concat();
    let error = [vec![1, 0x80], id.to_vec(), vec![1]].concat();

    let fixture = json!({
        "service": "8e6f0001-6f4e-4c4f-9b4f-4f70656e436f",
        "version_uuid": "8e6f0002-6f4e-4c4f-9b4f-4f70656e436f",
        "challenge_uuid": "8e6f0003-6f4e-4c4f-9b4f-4f70656e436f",
        "exchange_uuid": "8e6f0004-6f4e-4c4f-9b4f-4f70656e436f",
        "response_uuid": "8e6f0005-6f4e-4c4f-9b4f-4f70656e436f",
        "control_uuid": "8e6f0006-6f4e-4c4f-9b4f-4f70656e436f",
        "session_id": hex(&id),
        "challenge": hex(&challenge),
        "payload": hex(payload),
        "frames": [hex(&first), hex(&second)],
        "complete": hex(&complete),
        "cancel": hex(&cancel),
        "success": hex(&success),
        "cancel_response": hex(&cancel_response),
        "error": hex(&error)
    });
    println!("{}", serde_json::to_string_pretty(&fixture).unwrap());
}
