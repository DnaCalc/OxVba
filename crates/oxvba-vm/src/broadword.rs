pub fn contains_opcode_word(_word: u64, _opcode: u8) -> bool {
    // Placeholder for SWAR/broadword decoder path.
    false
}

#[allow(unexpected_cfgs)]
#[cfg(kani)]
mod kani_proofs {
    use super::contains_opcode_word;

    #[kani::proof]
    fn broadword_placeholder_is_total() {
        let word: u64 = kani::any();
        let opcode: u8 = kani::any();
        let _ = contains_opcode_word(word, opcode);
    }
}
