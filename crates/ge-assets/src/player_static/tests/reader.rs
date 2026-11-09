use super::*;

#[test]
fn defaults_read_the_owned_words_and_reject_missing_or_nonfinite_input() {
    let words = [1.5f32.to_bits(), (-2f32).to_bits(), 0u32];
    let mut bytes = words
        .into_iter()
        .flat_map(u32::to_be_bytes)
        .collect::<Vec<_>>();
    let data = Data {
        address: 0x8057_4548,
        bytes: bytes.clone().into_boxed_slice(),
    };
    assert_eq!(ResetDefaults::read(&data).unwrap().vector_bits(), words);
    let short = Data {
        address: data.address,
        bytes: bytes[..11].into(),
    };
    assert!(ResetDefaults::read(&short).is_err());
    bytes[4..8].copy_from_slice(&f32::NAN.to_bits().to_be_bytes());
    let nonfinite = Data {
        address: data.address,
        bytes: bytes.into_boxed_slice(),
    };
    assert!(ResetDefaults::read(&nonfinite).is_err());
    let outside = Data {
        address: 0x8057_4550,
        bytes: vec![0; 12].into_boxed_slice(),
    };
    assert!(ResetDefaults::read(&outside).is_err());
}
