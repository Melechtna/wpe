pub fn urlencoding_decode(s: &str) -> Result<String, ()> {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2 {
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                    continue;
                }
            }
            return Err(());
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }

    Ok(result)
}
