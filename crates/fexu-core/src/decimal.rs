//! Exakte Dezimalwerte über `rust_decimal`.
//!
//! Geldbeträge werden nie als `f32` oder `f64` geführt. Ein Wert muss bereits zur
//! deklarierten Precision und Scale passen. Zusätzliche Nachkommastellen sind ein
//! Validierungsfehler, es gibt keine stillen Rundung.
//!
//! Die Datenbank speichert den unskalierten Koeffizienten als 16 Byte in
//! vorzeichenbehafteter, big-endian Ordnung (das höchste Bit ist invertiert).
//! SQLite legt das als `BLOB` ab, PostgreSQL als `BYTEA`. `ORDER BY` und
//! Vergleiche folgen damit der numerischen Ordnung, nicht einer Text- oder
//! Float-Sortierung. Scale und Precision stehen in den Spaltenmetadaten.
//! JSON verwendet immer einen kanonischen Dezimalstring, nie eine JSON-Zahl.

use rust_decimal::Decimal;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecimalError {
    pub message: &'static str,
}

impl std::fmt::Display for DecimalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

pub fn validate_decimal_meta(precision: u8, scale: u8) -> Result<(), DecimalError> {
    if (1..=28).contains(&precision) && scale <= precision {
        Ok(())
    } else {
        Err(DecimalError {
            message: "decimal precision must be 1..=28 and scale must be <= precision",
        })
    }
}

/// Prüft Precision und Scale und liefert den auf `scale` normierten Wert.
///
/// `1e2` ist erlaubt und wird zu einem exakten Koeffizienten. `1.005` bei Scale 2
/// wird abgelehnt, weil die dritte Nachkommastelle nicht null ist.
pub fn prepare_decimal(value: Decimal, precision: u8, scale: u8) -> Result<Decimal, DecimalError> {
    validate_decimal_meta(precision, scale)?;
    if value.normalize().scale() > u32::from(scale) {
        return Err(DecimalError {
            message: "decimal has more fractional digits than the column scale",
        });
    }
    let mut exact = value;
    exact.rescale(u32::from(scale));
    let digits = exact.mantissa().unsigned_abs().to_string().len();
    if digits > usize::from(precision) {
        return Err(DecimalError {
            message: "decimal exceeds the column precision",
        });
    }
    Ok(exact)
}

pub fn parse_decimal(text: &str, precision: u8, scale: u8) -> Result<Decimal, DecimalError> {
    let value = Decimal::from_str(text.trim()).map_err(|_| DecimalError {
        message: "value is not a decimal",
    })?;
    prepare_decimal(value, precision, scale)
}

pub fn encode_decimal(value: Decimal, precision: u8, scale: u8) -> Result<[u8; 16], DecimalError> {
    let exact = prepare_decimal(value, precision, scale)?;
    Ok(order_key(exact.mantissa()))
}

pub fn decode_decimal(bytes: &[u8], scale: u8) -> Result<Decimal, DecimalError> {
    if scale > 28 || bytes.len() != 16 {
        return Err(DecimalError {
            message: "decimal storage is not a 16-byte coefficient",
        });
    }
    let mut raw = [0_u8; 16];
    raw.copy_from_slice(bytes);
    let coefficient = (u128::from_be_bytes(raw) ^ (1_u128 << 127)) as i128;
    Decimal::try_from_i128_with_scale(coefficient, u32::from(scale)).map_err(|_| DecimalError {
        message: "decimal coefficient does not fit the column scale",
    })
}

pub fn format_decimal(value: Decimal, scale: u8) -> Result<String, DecimalError> {
    let exact = prepare_decimal(value, 28, scale)?;
    let coefficient = exact.mantissa();
    let negative = coefficient.is_negative();
    let digits = coefficient.unsigned_abs().to_string();
    let places = usize::from(scale);
    let body = if places == 0 {
        digits
    } else if digits.len() <= places {
        format!("0.{:0>width$}", digits, width = places)
    } else {
        let split = digits.len() - places;
        format!("{}.{}", &digits[..split], &digits[split..])
    };
    Ok(if negative { format!("-{body}") } else { body })
}

pub fn format_decimal_text(text: &str, precision: u8, scale: u8) -> Result<String, DecimalError> {
    format_decimal(parse_decimal(text, precision, scale)?, scale)
}

/// SQL-Literal für einen Backfill. SQLite nutzt `X'..'`, PostgreSQL `'\x..'::bytea`.
pub fn decimal_sql_literal(
    postgres: bool,
    text: &str,
    precision: u8,
    scale: u8,
) -> Result<String, DecimalError> {
    let bytes = encode_decimal(parse_decimal(text, precision, scale)?, precision, scale)?;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02X}")).collect();
    Ok(if postgres {
        format!("'\\x{hex}'::bytea")
    } else {
        format!("X'{hex}'")
    })
}

fn order_key(coefficient: i128) -> [u8; 16] {
    ((coefficient as u128) ^ (1_u128 << 127)).to_be_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tenth_plus_two_tenths_is_three_tenths() {
        let left = Decimal::from_str("0.1").unwrap();
        let right = Decimal::from_str("0.2").unwrap();
        assert_eq!(left + right, Decimal::from_str("0.3").unwrap());
        assert_ne!(
            format!("{}", left + right),
            format!("{}", 0.1_f64 + 0.2_f64)
        );
    }

    #[test]
    fn scale_precision_negatives_and_large_values_roundtrip() {
        let price = parse_decimal("-10.50", 12, 2).unwrap();
        let bytes = encode_decimal(price, 12, 2).unwrap();
        assert_eq!(decode_decimal(&bytes, 2).unwrap(), price);
        assert_eq!(format_decimal(price, 2).unwrap(), "-10.50");
        assert!(parse_decimal("1.005", 12, 2).is_err());
        assert!(parse_decimal("10000000000.00", 12, 2).is_err());
        let huge = "12345678901234567890123456.78";
        let wide = parse_decimal(huge, 28, 2).unwrap();
        let stored = encode_decimal(wide, 28, 2).unwrap();
        assert_eq!(
            format_decimal(decode_decimal(&stored, 2).unwrap(), 2).unwrap(),
            huge
        );
        assert!(validate_decimal_meta(0, 0).is_err());
        assert!(validate_decimal_meta(4, 5).is_err());
    }

    #[test]
    fn byte_order_matches_numeric_order() {
        let keys = ["-2", "-0.10", "0", "0.10", "9", "10"]
            .map(|text| encode_decimal(parse_decimal(text, 12, 2).unwrap(), 12, 2).unwrap());
        let mut sorted = keys;
        sorted.sort();
        assert_eq!(keys, sorted);
    }

    #[test]
    fn json_string_is_canonical() {
        let value = parse_decimal("0.1", 12, 2).unwrap();
        assert_eq!(format_decimal(value, 2).unwrap(), "0.10");
        let json = serde_json::to_string(&value).unwrap();
        assert!(json.starts_with('"'), "{json}");
        let parsed: Decimal = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, value);
        assert_eq!(format_decimal_text("0.1", 12, 2).unwrap(), "0.10");
        let from_number: Decimal = serde_json::from_str("0.1").unwrap();
        assert_eq!(
            from_number,
            Decimal::from_str("0.1").unwrap(),
            "json number token {from_number}"
        );
    }
}
