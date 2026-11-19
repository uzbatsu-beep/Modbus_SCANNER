//! Register value decoding (P3-01/P3-02).
//!
//! Modbus registers are 16-bit big-endian words. Values spanning two registers
//! are affected by both **word order** and **byte order within each word**.
//! The four classic layouts are ABCD, BADC, CDAB and DCBA, named by the order
//! of the four bytes A B C D of the resulting 32-bit value.

use ts_rs::TS;
use serde::{Deserialize, Serialize};

/// Byte/word order for a 32-bit value built from two registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum ByteOrder {
    /// Big-endian words, big-endian bytes: `(w0 << 16) | w1`.
    Abcd,
    /// Big-endian words, bytes swapped inside each word.
    Badc,
    /// Words swapped, big-endian bytes inside each word.
    Cdab,
    /// Words swapped and bytes swapped inside each word.
    Dcba,
}

/// Interpretation to apply to raw registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum DataType {
    Int16,
    Uint16,
    Int32,
    Uint32,
    Float32,
    Ascii,
}

fn one() -> f64 {
    1.0
}

/// User-defined decoding and engineering-unit scaling.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
pub struct DecodeSpec {
    pub data_type: DataType,
    pub byte_order: ByteOrder,
    #[serde(default)]
    pub multiplier: f64,
    #[serde(default = "one")]
    pub divider: f64,
    #[serde(default)]
    pub offset: f64,
}

impl Default for DecodeSpec {
    fn default() -> Self {
        Self {
            data_type: DataType::Uint16,
            byte_order: ByteOrder::Abcd,
            multiplier: 1.0,
            divider: 1.0,
            offset: 0.0,
        }
    }
}

/// Decoded value, ready for display.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DecodedValue {
    Integer {
        #[ts(type = "number")]
        value: i64,
    },
    Float { value: f64 },
    Text { value: String },
}

/// One possible interpretation of the selected registers.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Interpretation {
    pub data_type: DataType,
    pub byte_order: ByteOrder,
    pub value: DecodedValue,
}

/// A labelled hypothesis, never presented as a fact.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Suggestion {
    pub data_type: DataType,
    pub byte_order: ByteOrder,
    pub value: f64,
    pub reason: String,
}

/// Result of the interactive inspector.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DecodeResult {
    pub interpretations: Vec<Interpretation>,
    pub suggestions: Vec<Suggestion>,
}

fn swap16(word: u16) -> u16 {
    word.rotate_left(8)
}

/// Assemble a 32-bit value from two registers in the requested layout.
pub fn u32_from_words(values: &[u16], order: ByteOrder) -> Option<u32> {
    if values.len() < 2 {
        return None;
    }
    let (w0, w1) = (values[0], values[1]);
    Some(match order {
        ByteOrder::Abcd => ((w0 as u32) << 16) | w1 as u32,
        ByteOrder::Badc => ((swap16(w0) as u32) << 16) | swap16(w1) as u32,
        ByteOrder::Cdab => ((w1 as u32) << 16) | w0 as u32,
        ByteOrder::Dcba => ((swap16(w1) as u32) << 16) | swap16(w0) as u32,
    })
}

fn scale(value: f64, spec: &DecodeSpec) -> f64 {
    if spec.divider == 0.0 {
        return f64::NAN;
    }
    value * spec.multiplier / spec.divider + spec.offset
}

fn has_scaling(spec: &DecodeSpec) -> bool {
    spec.multiplier != 1.0 || spec.divider != 1.0 || spec.offset != 0.0
}

/// Decode raw registers with a user-chosen interpretation.
pub fn decode(values: &[u16], spec: &DecodeSpec) -> Option<DecodedValue> {
    if values.is_empty() {
        return None;
    }
    if spec.data_type == DataType::Ascii {
        let mut text = String::new();
        for &word in values {
            for byte in word.to_be_bytes() {
                if (0x20..=0x7E).contains(&byte) {
                    text.push(byte as char);
                }
            }
        }
        return if text.is_empty() {
            None
        } else {
            Some(DecodedValue::Text { value: text })
        };
    }

    let raw = match spec.data_type {
        DataType::Int16 => values[0] as i16 as i64,
        DataType::Uint16 => values[0] as i64,
        DataType::Int32 => u32_from_words(values, spec.byte_order)? as i32 as i64,
        DataType::Uint32 => u32_from_words(values, spec.byte_order)? as i64,
        DataType::Float32 => {
            let bits = u32_from_words(values, spec.byte_order)?;
            let value = scale(f32::from_bits(bits) as f64, spec);
            return if value.is_finite() {
                Some(DecodedValue::Float { value })
            } else {
                None
            };
        }
        DataType::Ascii => unreachable!(),
    };

    if has_scaling(spec) {
        Some(DecodedValue::Float {
            value: scale(raw as f64, spec),
        })
    } else {
        Some(DecodedValue::Integer { value: raw })
    }
}

/// All interpretations for the selected registers (for the inspector table).
pub fn interpret_all(values: &[u16]) -> DecodeResult {
    let mut interpretations = Vec::new();
    let mut suggestions = Vec::new();
    if values.is_empty() {
        return DecodeResult {
            interpretations,
            suggestions,
        };
    }

    let types16 = [DataType::Int16, DataType::Uint16, DataType::Ascii];
    for data_type in types16 {
        if let Some(value) = decode(values, &DecodeSpec {
            data_type,
            ..Default::default()
        }) {
            interpretations.push(Interpretation {
                data_type,
                byte_order: ByteOrder::Abcd,
                value,
            });
        }
    }

    if values.len() >= 2 {
        for byte_order in [ByteOrder::Abcd, ByteOrder::Badc, ByteOrder::Cdab, ByteOrder::Dcba] {
            for data_type in [DataType::Int32, DataType::Uint32, DataType::Float32] {
                let spec = DecodeSpec {
                    data_type,
                    byte_order,
                    ..Default::default()
                };
                if let Some(value) = decode(values, &spec) {
                    if data_type == DataType::Float32 {
                        if let DecodedValue::Float { value: v } = &value {
                            // A non-integral, physically plausible float is a
                            // useful hypothesis — but still only a hypothesis.
                            if v.is_finite()
                                && v.abs() >= 1e-3
                                && v.abs() <= 1e7
                                && v.fract().abs() > 1e-6
                            {
                                suggestions.push(Suggestion {
                                    data_type,
                                    byte_order,
                                    value: *v,
                                    reason: "plausible float32 value".into(),
                                });
                            }
                        }
                    }
                    interpretations.push(Interpretation {
                        data_type,
                        byte_order,
                        value,
                    });
                }
            }
        }
    }

    DecodeResult {
        interpretations,
        suggestions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float32_all_byte_orders() {
        let words = [0x4367, 0x3333]; // ≈ 231.2 in ABCD
        let value = decode(
            &words,
            &DecodeSpec {
                data_type: DataType::Float32,
                byte_order: ByteOrder::Abcd,
                ..Default::default()
            },
        )
        .unwrap();
        match value {
            DecodedValue::Float { value: v } => assert!((v - 231.2).abs() < 0.01),
            other => panic!("expected float, got {other:?}"),
        }
        // CDAB interprets the swapped pair and yields a different number.
        assert_ne!(
            u32_from_words(&words, ByteOrder::Abcd),
            u32_from_words(&words, ByteOrder::Cdab)
        );
    }

    #[test]
    fn int32_twos_complement() {
        let value = decode(
            &[0xFFFF, 0xFFFE],
            &DecodeSpec {
                data_type: DataType::Int32,
                byte_order: ByteOrder::Abcd,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(value, DecodedValue::Integer { value: -2 });
    }

    #[test]
    fn ascii_and_scaling() {
        let text = decode(
            &[0x4142, 0x4300],
            &DecodeSpec {
                data_type: DataType::Ascii,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(text, DecodedValue::Text { value: "ABC".into() });

        let scaled = decode(
            &[1000],
            &DecodeSpec {
                data_type: DataType::Uint16,
                multiplier: 1.0,
                divider: 10.0,
                offset: 2.0,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(scaled, DecodedValue::Float { value: 102.0 });
    }

    #[test]
    fn suggestions_are_labelled_hypotheses() {
        let result = interpret_all(&[0x4367, 0x3333]);
        assert!(result.suggestions.iter().any(|s| s.byte_order == ByteOrder::Abcd));
        assert!(result.suggestions.iter().all(|s| !s.reason.is_empty()));
    }
}
