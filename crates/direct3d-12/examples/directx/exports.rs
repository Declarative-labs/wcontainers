//! The libraries' exports, given to the linker as directives in the object file.

/// Places linker directives (`/EXPORT:...`) in the `.drectve` section, which the linker reads.
macro_rules! linker_directives {
    ($text:expr) => {
        #[used]
        #[unsafe(link_section = ".drectve")]
        static DIRECTIVES: [u8; $text.len()] = {
            let mut out = [0; $text.len()];
            let bytes = $text.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                out[i] = bytes[i];
                i += 1;
            }
            out
        };
    };
}
