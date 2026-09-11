use crate::layout_engine::types::DataContext;
use crate::layout_engine::types::DataValue;

impl DataContext {
    /// Create a new empty data context.
    pub fn new() -> Self {
        Self {
            values: dotzuki_engine::hash::HashMap::default(),
        }
    }

    /// Set a value. Accepts various types via `Into<DataValue>`.
    pub fn set<V: Into<DataValue>>(&mut self, key: &str, value: V) {
        self.values.insert(key.to_string(), value.into());
    }

    /// Get a value by key.
    pub fn get(&self, key: &str) -> Option<&DataValue> {
        self.values.get(key)
    }

    /// The active language locale code (e.g. `"en"` / `"zh"`), read from the
    /// reserved `__lang` key. Callers set it with `ctx.set("__lang", "zh")`
    /// before rendering; it selects the variant for `@t("en", "中文")` text.
    /// Defaults to `"en"` when unset.
    pub fn lang(&self) -> &str {
        match self.values.get("__lang") {
            Some(DataValue::Str(s)) => s.as_str(),
            _ => "en",
        }
    }

    /// Check if variable exists and is truthy (for `visible` field evaluation).
    /// Returns `true` if:
    /// - The value is `Bool(true)`
    /// - The value is a non-zero `Int`
    /// - The value exists and is any other type
    /// Returns `false` if the key is absent or the value is `Bool(false)` / `Int(0)`.
    pub fn is_truthy(&self, key: &str) -> bool {
        match self.values.get(key) {
            Some(DataValue::Bool(b)) => *b,
            Some(DataValue::Int(n)) => *n != 0,
            Some(_) => true,
            None => false,
        }
    }

    /// Resolve a template string by substituting `{var}`, `{var:0W}`, and `{var%N}` patterns.
    ///
    /// # Examples
    ///
    /// ```
    /// # use dotzuki_renderer::layout_engine::types::DataContext;
    /// let mut ctx = DataContext::new();
    /// ctx.set("name", "LEAFKIT");
    /// assert_eq!(ctx.resolve("{name}"), "LEAFKIT");
    ///
    /// ctx.set("num", 1i64);
    /// assert_eq!(ctx.resolve("\u{2116}\u{2022}{num:03}"), "\u{2116}\u{2022}001");
    ///
    /// ctx.set("weight", 150i64);
    /// assert_eq!(ctx.resolve("{weight%10}"), "0");
    /// ```
    pub fn resolve(&self, template: &str) -> String {
        let mut result = String::with_capacity(template.len());
        let mut chars = template.chars();

        while let Some(ch) = chars.next() {
            if ch == '{' {
                let mut expr = String::new();
                let mut found_close = false;

                for inner in chars.by_ref() {
                    if inner == '}' {
                        found_close = true;
                        break;
                    }
                    if inner == '{' {
                        break;
                    }
                    expr.push(inner);
                }

                if found_close && !expr.is_empty() {
                    result.push_str(&self.resolve_expr(&expr));
                } else {
                    result.push('{');
                    result.push_str(&expr);
                    if found_close {
                        result.push('}');
                    }
                }
            } else {
                result.push(ch);
            }
        }

        result
    }

    /// Resolve a single `{...}` expression (without the braces).
    fn resolve_expr(&self, expr: &str) -> String {
        if let Some(colon_pos) = expr.find(':') {
            let var = &expr[..colon_pos];
            let format = &expr[colon_pos + 1..];
            let value = self.values.get(var);
            return self.format_zero_pad(value, format);
        }

        if let Some(pct_pos) = expr.find('%') {
            let var = &expr[..pct_pos];
            let modulo_str = &expr[pct_pos + 1..];
            let value = self.values.get(var);
            return self.format_modulo(value, modulo_str);
        }

        match self.values.get(expr) {
            Some(value) => self.data_value_to_string(value),
            None => "?".to_string(),
        }
    }

    /// Apply zero-pad formatting: `:03` → "001".
    fn format_zero_pad(&self, value: Option<&DataValue>, format: &str) -> String {
        let width: usize = if format.starts_with('0') {
            format[1..].parse().unwrap_or(0)
        } else {
            format.parse().unwrap_or(0)
        };

        match value {
            Some(DataValue::Int(n)) => {
                format!("{:0width$}", n, width = width)
            }
            Some(other) => self.data_value_to_string(other),
            None => "?".to_string(),
        }
    }

    /// Apply modulo formatting: `%10` → last digit.
    fn format_modulo(&self, value: Option<&DataValue>, modulo_str: &str) -> String {
        let modulo: i64 = modulo_str.parse().unwrap_or(1);

        match value {
            Some(DataValue::Int(n)) => {
                let result = if modulo != 0 { n % modulo } else { *n };
                result.to_string()
            }
            Some(other) => self.data_value_to_string(other),
            None => "?".to_string(),
        }
    }

    /// Convert a DataValue to its string representation.
    fn data_value_to_string(&self, value: &DataValue) -> String {
        match value {
            DataValue::Str(s) => s.clone(),
            DataValue::Int(n) => n.to_string(),
            DataValue::Float(f) => f.to_string(),
            DataValue::Bool(b) => b.to_string(),
            DataValue::TileId(t) => t.to_string(),
            DataValue::List(v) => v
                .iter()
                .map(|dv| self.data_value_to_string(dv))
                .collect::<Vec<_>>()
                .join(", "),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_engine::types::DataValue;

    #[test]
    fn test_simple_substitution() {
        let mut ctx = DataContext::new();
        ctx.set("name", "LEAFKIT");
        assert_eq!(ctx.resolve("{name}"), "LEAFKIT");
    }

    #[test]
    fn test_zero_padded() {
        let mut ctx = DataContext::new();
        ctx.set("num", 1i64);
        assert_eq!(ctx.resolve("№•{num:03}"), "№•001");
    }

    #[test]
    fn test_multiple_vars() {
        let mut ctx = DataContext::new();
        ctx.set("feet", 2i64);
        ctx.set("inches", 4i64);
        assert_eq!(ctx.resolve("{feet}′{inches:02}″"), "2′04″");
    }

    #[test]
    fn test_modulo() {
        let mut ctx = DataContext::new();
        ctx.set("weight", 150i64);
        assert_eq!(ctx.resolve("{weight%10}"), "0");
    }

    #[test]
    fn test_modulo_non_zero() {
        let mut ctx = DataContext::new();
        ctx.set("weight", 157i64);
        assert_eq!(ctx.resolve("{weight%10}"), "7");
    }

    #[test]
    fn test_unknown_var_placeholder() {
        let ctx = DataContext::new();
        assert_eq!(ctx.resolve("{unknown}"), "?");
    }

    #[test]
    fn test_plain_text_no_vars() {
        let ctx = DataContext::new();
        assert_eq!(ctx.resolve("Hello World"), "Hello World");
    }

    #[test]
    fn test_mixed_text_and_vars() {
        let mut ctx = DataContext::new();
        ctx.set("item", "POTION");
        ctx.set("count", 3i64);
        assert_eq!(ctx.resolve("{item} x{count}"), "POTION x3");
    }

    #[test]
    fn test_bool_value() {
        let mut ctx = DataContext::new();
        ctx.set("flag", true);
        assert_eq!(ctx.resolve("{flag}"), "true");

        ctx.set("flag", false);
        assert_eq!(ctx.resolve("{flag}"), "false");
    }

    #[test]
    fn test_is_truthy() {
        let mut ctx = DataContext::new();

        assert!(!ctx.is_truthy("missing"));

        ctx.set("active", true);
        assert!(ctx.is_truthy("active"));

        ctx.set("active", false);
        assert!(!ctx.is_truthy("active"));

        ctx.set("count", 5i64);
        assert!(ctx.is_truthy("count"));

        ctx.set("count", 0i64);
        assert!(!ctx.is_truthy("count"));

        ctx.set("name", "test");
        assert!(ctx.is_truthy("name"));
    }

    #[test]
    fn test_empty_braces_passthrough() {
        let ctx = DataContext::new();
        assert_eq!(ctx.resolve("before{}after"), "before{}after");
    }

    #[test]
    fn test_malformed_brace_passthrough() {
        let ctx = DataContext::new();
        assert_eq!(ctx.resolve("Hello {unclosed"), "Hello {unclosed");
    }

    #[test]
    fn test_get_value() {
        let mut ctx = DataContext::new();
        ctx.set("key", "value");
        assert_eq!(ctx.get("key"), Some(&DataValue::Str("value".to_string())));
        assert_eq!(ctx.get("missing"), None);
    }

    #[test]
    fn test_zero_pad_large_number() {
        let mut ctx = DataContext::new();
        ctx.set("num", 42i64);
        assert_eq!(ctx.resolve("{num:05}"), "00042");
    }

    #[test]
    fn test_zero_pad_exact_width() {
        let mut ctx = DataContext::new();
        ctx.set("num", 999i64);
        assert_eq!(ctx.resolve("{num:03}"), "999");
    }

    #[test]
    fn test_tile_id_value() {
        let mut ctx = DataContext::new();
        ctx.set("tile", 42u16);
        assert_eq!(ctx.resolve("Tile:{tile}"), "Tile:42");
    }

    #[test]
    fn test_float_value() {
        let mut ctx = DataContext::new();
        ctx.set("ratio", DataValue::Float(1.5));
        assert_eq!(ctx.resolve("{ratio}"), "1.5");
    }

    #[test]
    fn test_list_value() {
        let mut ctx = DataContext::new();
        ctx.set(
            "items",
            DataValue::List(vec![
                DataValue::Str("A".into()),
                DataValue::Str("B".into()),
                DataValue::Str("C".into()),
            ]),
        );
        assert_eq!(ctx.resolve("{items}"), "A, B, C");
    }
}
