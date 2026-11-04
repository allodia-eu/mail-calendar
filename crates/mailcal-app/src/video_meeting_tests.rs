use super::*;

#[test]
fn base32_is_rfc_4648_lowercase_and_unpadded() {
    // RFC 4648 §10's test vectors, lowercased, without their `=` padding.
    for (input, expected) in [
        ("", ""),
        ("f", "my"),
        ("fo", "mzxq"),
        ("foo", "mzxw6"),
        ("foob", "mzxw6yq"),
        ("fooba", "mzxw6ytb"),
        ("foobar", "mzxw6ytboi"),
    ] {
        assert_eq!(base32(input.as_bytes()), expected, "{input:?}");
    }
}

#[test]
fn a_minted_key_is_128_bits_in_the_shortest_key_there_is() {
    let key = mint_key().unwrap();
    assert_eq!(key.as_str().len(), 26);
    assert_ne!(mint_key().unwrap(), key);
    assert_eq!(base32(&[0xff; 16]).len(), 26);
}
