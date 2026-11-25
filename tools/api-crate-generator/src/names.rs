//! How a C name becomes a Rust name.

/// Rust's keywords that a C name may turn into.
const KEYWORDS: &[&str] = &[
    "as", "async", "await", "box", "break", "const", "continue", "crate", "do", "dyn", "else", "enum", "extern",
    "false", "final", "fn", "for", "gen", "if", "impl", "in", "let", "loop", "macro", "match", "mod", "move", "mut",
    "override", "priv", "pub", "ref", "return", "self", "static", "struct", "super", "trait", "true", "try", "type",
    "typeof", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

/// A name that is a Rust keyword gets a trailing underscore (`Type` to `type_`), not a raw identifier, which the
/// linter's view of an interface does not read.
pub fn escaped(name: String) -> String {
    if KEYWORDS.contains(&name.as_str()) { format!("{name}_") } else { name }
}

/// `IASetVertexBuffers` to `ia_set_vertex_buffers`, `CreateTexture2D` to `create_texture_2d`, `ByteWidth` to
/// `byte_width`: a word starts at a capital after a small letter or a digit, at the last capital of a run of
/// capitals followed by a small letter, and at a digit after a letter; a `D` right after a digit (2D) stays with it.
pub fn snake_case(name: &str) -> String {
    let characters: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (position, &character) in characters.iter().enumerate() {
        let previous = position.checked_sub(1).map(|before| characters[before]);
        let next = characters.get(position + 1).copied();
        let starts_word = match previous {
            None => false,
            Some('_') => false,
            Some(before) => {
                if character == '_' {
                    false
                } else if character.is_ascii_uppercase() {
                    if before.is_ascii_digit() {
                        // 2D, 3D: the D belongs to its number; 2DArray: "Array" starts a word.
                        !(character == 'D' && next.is_none_or(|after| !after.is_ascii_lowercase()))
                    } else {
                        before.is_ascii_lowercase()
                            || (before.is_ascii_uppercase() && next.is_some_and(|after| after.is_ascii_lowercase()))
                    }
                } else if character.is_ascii_digit() {
                    before.is_ascii_alphabetic() && next.is_some_and(|after| after == 'D')
                } else {
                    false
                }
            }
        };
        if starts_word && !out.ends_with('_') {
            out.push('_');
        }
        out.push(character.to_ascii_lowercase());
    }
    out
}

/// `D3D11_BUFFER_DESC` less its prefix `D3D11_` to `BufferDesc`; `TEXTURE2D_DESC` to `Texture2DDesc`.
pub fn pascal_case_of_upper_snake(name: &str) -> String {
    let mut out = String::new();
    for word in name.split('_').filter(|word| !word.is_empty()) {
        let mut previous_digit = false;
        for (position, character) in word.chars().enumerate() {
            if position == 0 {
                out.push(character.to_ascii_uppercase());
            } else if previous_digit && character == 'D' {
                out.push('D');
            } else {
                out.push(character.to_ascii_lowercase());
            }
            previous_digit = character.is_ascii_digit();
        }
    }
    out
}

/// A field or parameter name less Hungarian prefixes the headers use (`pDesc`, `ppBuffer`, `pSysMem`).
pub fn value_name(name: &str) -> String {
    let stripped = ["pp", "p"]
        .iter()
        .find_map(|prefix| {
            name.strip_prefix(prefix).filter(|rest| rest.chars().next().is_some_and(|first| first.is_ascii_uppercase()))
        })
        .unwrap_or(name);
    escaped(snake_case(stripped))
}

/// `ID3D11Device` with its prefix `ID3D11` to `IID_ID3D11_DEVICE`: the prefix kept as the header writes it.
pub fn interface_id_name(prefix: &str, rest: &str) -> String {
    format!("IID_{}_{}", prefix.to_ascii_uppercase(), snake_case(rest).to_ascii_uppercase())
}

pub fn strip_first<'a>(name: &'a str, prefixes: &[&str]) -> &'a str {
    prefixes.iter().find_map(|prefix| name.strip_prefix(prefix)).unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_as_the_spike_wrote_them() {
        assert_eq!(snake_case("IASetVertexBuffers"), "ia_set_vertex_buffers");
        assert_eq!(snake_case("CreateTexture2D"), "create_texture_2d");
        assert_eq!(snake_case("OMSetRenderTargetsAndUnorderedAccessViews"), "om_set_render_targets_and_unordered_access_views");
        assert_eq!(snake_case("GetDesc1"), "get_desc1");
        assert_eq!(snake_case("ByteWidth"), "byte_width");
        assert_eq!(snake_case("Texture2DArray"), "texture_2d_array");
        assert_eq!(value_name("pInitialData"), "initial_data");
        assert_eq!(value_name("ppBuffer"), "buffer");
        assert_eq!(value_name("pSysMem"), "sys_mem");
        assert_eq!(pascal_case_of_upper_snake("TEXTURE2D_DESC"), "Texture2DDesc");
        assert_eq!(pascal_case_of_upper_snake("BUFFER_DESC"), "BufferDesc");
        assert_eq!(interface_id_name("ID3D11", "Device"), "IID_ID3D11_DEVICE");
    }
}
