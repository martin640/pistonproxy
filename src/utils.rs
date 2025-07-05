pub fn bytes_as_hex(data: &[u8]) -> String {
    let bytes = data.to_vec();
    bytes.iter()
        .map(|b| format!("{:02x}", b).to_string())
        .collect::<Vec<String>>()
        .join(" ")
}