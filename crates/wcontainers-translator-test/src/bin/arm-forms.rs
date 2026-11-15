//! Turns Arm's machine readable `Instructions.json` into `arm/a64-forms.txt`: one line per A64 encoding.
//!
//! Usage: `arm-forms <Instructions.json> <output>`.

use serde_json::Value;
use std::fmt::Write as _;

const NOTICE: &str = "\
# Every A64 instruction encoding, taken from Arm's machine readable specification
# (AARCHMRS, BSD release {release}) by `cargo run -p wcontainers-translator-test --bin arm-forms`.
#
# Copyright 2010-2026 Arm Limited or its affiliates. Redistribution and use in source and binary forms, with
# or without modification, are permitted provided that the following conditions are met: Redistributions of
# source code must retain the above copyright notice, this list of conditions and the following disclaimer.
# Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the
# following disclaimer in the documentation and/or other materials provided with the distribution. Neither the
# name of the copyright holder nor the names of its contributors may be used to endorse or promote products
# derived from this software without specific prior written permission. THIS SOFTWARE IS PROVIDED BY THE
# COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED
# TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT
# SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
# EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
# SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
# LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY
# WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
#
# Columns: name, group path, features (a condition over FEAT_ names, or true), fixed-bit mask, fixed-bit
# value, should-be mask, should-be value, then each named field as name:lowest-bit:width.
";

fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    let [_, input, output] = arguments.as_slice() else {
        panic!("usage: arm-forms <Instructions.json> <output>");
    };
    let text = std::fs::read_to_string(input).expect("read the specification");
    let root: Value = serde_json::from_str(&text).expect("parse the specification");
    let release = root["_meta"]["version"]["ref"]
        .as_str()
        .unwrap_or("unknown");
    let a64 = root["instructions"]
        .as_array()
        .and_then(|sets| sets.iter().find(|set| set["name"] == "A64"))
        .expect("the A64 instruction set");

    let mut out = NOTICE.replace("{release}", release);
    walk(a64, &mut Vec::new(), &mut out);
    std::fs::write(output, out).expect("write the forms");
}

fn walk<'a>(node: &'a Value, path: &mut Vec<&'a str>, out: &mut String) {
    let name = node["name"].as_str().unwrap_or("");
    if node["_type"] == "Instruction.Instruction" {
        line(node, name, &path[1..].join("/"), out);
        return;
    }
    path.push(name);
    for child in node["children"].as_array().into_iter().flatten() {
        walk(child, path, out);
    }
    path.pop();
}

fn line(node: &Value, name: &str, group: &str, out: &mut String) {
    let (mut mask, mut value, mut should_mask, mut should_value) = (0u32, 0u32, 0u32, 0u32);
    let mut fields = String::new();
    for part in node["encoding"]["values"]
        .as_array()
        .expect("encoding values")
    {
        let start = part["range"]["start"].as_u64().expect("start") as u32;
        let width = part["range"]["width"].as_u64().expect("width") as u32;
        let bits = quoted(&part["value"]["value"]);
        let should = quoted(&part["should_be_mask"]["value"]);
        assert_eq!(bits.len() as u32, width, "{name}: value width");
        for (index, (bit, should_bit)) in bits.chars().zip(should.chars()).enumerate() {
            let at = start + width - 1 - index as u32;
            match (bit, should_bit) {
                ('0' | '1', '1') => {
                    should_mask |= 1 << at;
                    should_value |= u32::from(bit == '1') << at;
                }
                ('0' | '1', '0') => {
                    mask |= 1 << at;
                    value |= u32::from(bit == '1') << at;
                }
                ('x', _) => {}
                other => panic!("{name}: unexpected bit {other:?}"),
            }
        }
        if let Some(field) = part["name"].as_str() {
            write!(fields, " {field}:{start}:{width}").unwrap();
        }
    }
    let features = condition(&node["condition"]);
    writeln!(
        out,
        "{name} {group} {features} {mask:#010x} {value:#010x} {should_mask:#010x} {should_value:#010x}{fields}"
    )
    .unwrap();
}

fn quoted(value: &Value) -> &str {
    value.as_str().expect("bit string").trim_matches('\'')
}

/// The feature condition as text without spaces, such as `(FEAT_SVE||FEAT_SME)`.
fn condition(node: &Value) -> String {
    match node["_type"].as_str() {
        Some("AST.Bool") => node["value"].to_string(),
        Some("AST.Identifier" | "Values.Value") => {
            node["value"].as_str().expect("identifier").to_string()
        }
        Some("AST.Function") if node["name"] == "IsFeatureImplemented" => {
            condition(&node["arguments"][0])
        }
        Some("AST.BinaryOp") => format!(
            "({}{}{})",
            condition(&node["left"]),
            node["op"].as_str().expect("operator"),
            condition(&node["right"])
        ),
        Some("AST.UnaryOp") => format!(
            "{}{}",
            node["op"].as_str().expect("operator"),
            condition(&node["expr"])
        ),
        Some("AST.Set") => {
            let items: Vec<String> = node["values"]
                .as_array()
                .expect("set values")
                .iter()
                .map(condition)
                .collect();
            format!("{{{}}}", items.join(","))
        }
        other => panic!("condition node {other:?}: {node}"),
    }
}
