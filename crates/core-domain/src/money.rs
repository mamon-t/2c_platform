//! Точный денежный тип: суммы хранятся в копейках (минимальных единицах валюты).
//!
//! Арифметика выполняется в `i128` без плавающей точки, что исключает ошибки
//! округления в учётных операциях. Сериализованное представление — строка
//! целых копеек (`"12345"`); дробные числа и строки с десятичным разделителем
//! отклоняются, чтобы значение не имело двоякой интерпретации.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::iter::Sum;
use std::ops::{Add, Mul, Neg, Sub};

/// Сумма в копейках.
///
/// Примеры: `Money::from_rubles(10)` — десять рублей, `Money::from_kopecks(-5)` —
/// минус пять копеек.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Money(pub i128);

impl Money {
    /// Нулевая сумма.
    pub const ZERO: Money = Money(0);

    /// Точность: число знаков после запятой в основной денежной единице.
    pub const PRECISION: u32 = 2;

    /// Множитель перевода целых рублей в копейки (`10^PRECISION`).
    pub const SCALE: i128 = 100;

    /// Создаёт сумму из копеек.
    pub const fn from_kopecks(kopecks: i128) -> Self {
        Money(kopecks)
    }

    /// Создаёт сумму из целых рублей.
    ///
    /// # Panics
    ///
    /// Не паникует: результат переполнения лишь оборачивается по модулю, как
    /// обычная целочисленная арифметика Rust.
    pub const fn from_rubles(rubles: i64) -> Self {
        Money((rubles as i128) * Self::SCALE)
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let abs = self.0.unsigned_abs();
        let rubles = abs / Self::SCALE as u128;
        let kopecks = abs % Self::SCALE as u128;
        write!(f, "{sign}{rubles}.{kopecks:02}")
    }
}

impl Add for Money {
    type Output = Money;

    fn add(self, rhs: Money) -> Money {
        Money(self.0 + rhs.0)
    }
}

impl Sub for Money {
    type Output = Money;

    fn sub(self, rhs: Money) -> Money {
        Money(self.0 - rhs.0)
    }
}

impl Neg for Money {
    type Output = Money;

    fn neg(self) -> Money {
        Money(-self.0)
    }
}

impl Mul<i64> for Money {
    type Output = Money;

    fn mul(self, rhs: i64) -> Money {
        Money(self.0 * rhs as i128)
    }
}

impl Sum for Money {
    fn sum<I: Iterator<Item = Money>>(iter: I) -> Money {
        iter.fold(Money::ZERO, Money::add)
    }
}

impl Serialize for Money {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for Money {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MoneyVisitor;

        impl<'de> serde::de::Visitor<'de> for MoneyVisitor {
            type Value = Money;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "целое число копеек или строка целых копеек")
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Money, E> {
                Ok(Money(v as i128))
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Money, E> {
                Ok(Money(v as i128))
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Money, E> {
                v.trim()
                    .parse::<i128>()
                    .map(Money)
                    .map_err(|_| E::invalid_value(serde::de::Unexpected::Str(v), &self))
            }

            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Money, E> {
                Err(E::custom(format!(
                    "дробное число {v:?} недопустимо для Money — сумма должна быть целым числом копеек"
                )))
            }
        }

        deserializer.deserialize_any(MoneyVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn constructors() {
        assert_eq!(Money::from_kopecks(12345), Money(12345));
        assert_eq!(Money::from_rubles(123), Money(12300));
        assert_eq!(Money::from_rubles(-2), Money(-200));
        assert_eq!(Money::ZERO, Money(0));
        assert_eq!((Money::PRECISION, Money::SCALE), (2, 100));
    }

    #[test]
    fn arithmetic() {
        assert_eq!(Money(100) + Money(50), Money(150));
        assert_eq!(Money(100) - Money(150), Money(-50));
        assert_eq!(-Money(55), Money(-55));
        assert_eq!(Money(107) * 3, Money(321));
        let sum: Money = [Money(10), Money(20), Money(30)].into_iter().sum();
        assert_eq!(sum, Money(60));
    }

    #[test]
    fn display() {
        assert_eq!(Money(12345).to_string(), "123.45");
        assert_eq!(Money(5).to_string(), "0.05");
        assert_eq!(Money(0).to_string(), "0.00");
        assert_eq!(Money(-5).to_string(), "-0.05");
        assert_eq!(Money(-12345).to_string(), "-123.45");
    }

    #[test]
    fn serialize_as_kopecks_string() {
        assert_eq!(serde_json::to_string(&Money(12345)).unwrap(), "\"12345\"");
        assert_eq!(serde_json::to_string(&Money::ZERO).unwrap(), "\"0\"");
    }

    #[test]
    fn deserialize_integer_and_string() {
        assert_eq!(serde_json::from_str::<Money>("12345").unwrap(), Money(12345));
        assert_eq!(serde_json::from_str::<Money>("\"12345\"").unwrap(), Money(12345));
        assert_eq!(serde_json::from_str::<Money>("\" -7 \"").unwrap(), Money(-7));
    }

    #[test]
    fn round_trip() {
        for v in [0, 1, 100, 12345, -42] {
            let m = Money(v);
            let s = serde_json::to_string(&m).unwrap();
            assert_eq!(serde_json::from_str::<Money>(&s).unwrap(), m);
        }
    }

    #[test]
    fn reject_fractional() {
        let err_float = serde_json::from_str::<Money>("1.5");
        assert!(err_float.is_err());

        let err_string_fraction = serde_json::from_str::<Money>("\"12.34\"");
        assert!(err_string_fraction.is_err());

        let err_bool = serde_json::from_str::<Money>("true");
        assert!(err_bool.is_err());

        let err_null = serde_json::from_str::<Money>("null");
        assert!(err_null.is_err());
    }

    #[test]
    fn from_json_value() {
        // de через serde_json::Value обходит посетитель тем же путём.
        assert_eq!(serde_json::from_value::<Money>(json!(777)).unwrap(), Money(777));
        assert_eq!(serde_json::from_value::<Money>(json!("777")).unwrap(), Money(777));
        assert!(serde_json::from_value::<Money>(json!(1.1)).is_err());
    }
}