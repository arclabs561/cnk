use cnk::{
    decompress_set_auto, decompress_set_enveloped, CodecChoice, DeltaVarintCompressor,
    IdCompressionMethod, IdSetCompressor,
};

#[test]
fn envelope_rejects_oversized_payload_length_without_panicking() {
    for (magic, header_len, length_offset) in [(b"CNKENV01", 25, 17), (b"CNKENV02", 33, 21)] {
        let mut bytes = vec![0; header_len];
        bytes[..8].copy_from_slice(magic);
        bytes[length_offset..length_offset + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(decompress_set_enveloped(&bytes).is_err());
    }
}

#[test]
fn delta_zero_count_rejects_trailing_bytes() {
    assert_eq!(
        DeltaVarintCompressor::new()
            .decompress_set(&[0], 100)
            .unwrap(),
        Vec::<u32>::new()
    );
    assert!(DeltaVarintCompressor::new()
        .decompress_set(&[0, 42], 100)
        .is_err());
}

#[test]
fn none_codec_rejects_nonempty_payload() {
    let choice = CodecChoice {
        method: IdCompressionMethod::None,
        partition_block_size: 0,
    };
    assert_eq!(
        decompress_set_auto(choice.clone(), &[], 100).unwrap(),
        Vec::<u32>::new()
    );
    assert!(decompress_set_auto(choice, &[42], 100).is_err());
}

#[cfg(feature = "sbits")]
#[test]
fn elias_fano_rejects_ids_outside_the_declared_universe() {
    let codec = cnk::EliasFanoCompressor::new();
    let mut bytes = codec.compress_set(&[9], 16).unwrap();
    bytes[8..16].copy_from_slice(&9u64.to_le_bytes());
    assert!(codec.decompress_set(&bytes, 9).is_err());
}

#[cfg(feature = "sbits")]
#[test]
fn partitioned_elias_fano_rejects_ids_outside_the_declared_universe() {
    let codec = cnk::PartitionedEliasFanoCompressor::new();
    let mut bytes = codec.compress_set(&[9], 16).unwrap();
    bytes[8..16].copy_from_slice(&9u64.to_le_bytes());
    assert!(codec.decompress_set(&bytes, 9).is_err());
}
