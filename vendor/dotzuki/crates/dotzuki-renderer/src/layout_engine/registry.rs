use dotzuki_engine::hash::HashMap;
use core::fmt::Debug;

use crate::layout_engine::types::{
    DataContext, ElementParams, LayoutElement, RenderContext, RenderError, ScreenLayout,
};
use dotzuki_engine::render::Painter;

/// Value kind a custom-element prop accepts, mirroring the DSL's
/// `component` declaration kinds.
///
/// `Expr` admits anything a data binding can carry in the compiled JSON — a
/// number or a (possibly `"{var}"`-templated) string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropType {
    Int,
    String,
    Bool,
    Color,
    Expr,
}

impl PropType {
    fn matches(self, value: &serde_json::Value) -> bool {
        match self {
            PropType::Int => value.is_i64() || value.is_u64(),
            PropType::String | PropType::Color => value.is_string(),
            PropType::Bool => value.is_boolean(),
            PropType::Expr => value.is_number() || value.is_string(),
        }
    }

    fn name(self) -> &'static str {
        match self {
            PropType::Int => "int",
            PropType::String => "string",
            PropType::Bool => "bool",
            PropType::Color => "color",
            PropType::Expr => "expr",
        }
    }
}

/// One prop of a [`ComponentSchema`].
#[derive(Debug, Clone)]
pub struct PropSpec {
    pub name: &'static str,
    pub ty: PropType,
    pub required: bool,
}

impl PropSpec {
    pub const fn required(name: &'static str, ty: PropType) -> Self {
        Self {
            name,
            ty,
            required: true,
        }
    }

    pub const fn optional(name: &'static str, ty: PropType) -> Self {
        Self {
            name,
            ty,
            required: false,
        }
    }
}

/// The prop schema a [`CustomElement`] expects — the runtime counterpart of
/// the DSL's `component` declaration.
///
/// Layouts are checked against it once at load time
/// ([`ElementRegistry::validate_layout`]), so authoring mistakes surface as
/// one clear error instead of a silently blank element every frame.
#[derive(Debug, Clone, Default)]
pub struct ComponentSchema {
    pub props: Vec<PropSpec>,
}

impl ComponentSchema {
    pub fn new(props: Vec<PropSpec>) -> Self {
        Self { props }
    }
}

/// A schema violation found by [`ElementRegistry::validate_layout`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaViolation {
    /// Element id, or `<type:...>` when the element has no id.
    pub element: String,
    pub message: String,
}

impl core::fmt::Display for SchemaViolation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}: {}", self.element, self.message)
    }
}

/// A custom layout element that can be registered from any game or plugin.
///
/// Implement this trait to provide rendering logic for a new element type
/// (e.g. `custom:monster_sprite`, `custom:hp_bar`). Once registered via
/// [`ElementRegistry::register`], the layout engine will dispatch elements
/// of that type to the implementation's [`render`](CustomElement::render)
/// method.
pub trait CustomElement: Debug + Send + Sync {
    /// Returns the unique element type identifier.
    ///
    /// This string is matched against the `type` field in a
    /// [`LayoutElement`] at dispatch time. Convention: use a colon-prefixed
    /// namespace, e.g. `"custom:monster_sprite"` or `"custom:hp_bar"`.
    fn element_type(&self) -> &'static str;

    /// The prop schema this element expects.
    ///
    /// Used by [`ElementRegistry::validate_layout`] to check layouts at load
    /// time. The default (empty) schema accepts any props — override it to
    /// get required-prop and type checking. Keep it in sync with the
    /// `component` declaration in the game's `.gui` prelude; a unit test
    /// deserialising a schema-shaped params object through the element's own
    /// param struct is the cheapest drift guard.
    fn schema(&self) -> ComponentSchema {
        ComponentSchema::default()
    }

    /// Render this custom element into the framebuffer.
    ///
    /// # Arguments
    ///
    /// * `element` — The layout element definition (contains id, rect,
    ///   element-specific params, etc.).
    /// * `ctx` — Per-frame mutable data context for template variables.
    /// * `render_ctx` — Shared immutable rendering state (screen, theme,
    ///   fonts, tilesets).
    /// * `painter` — The painter used to draw into the framebuffer.
    fn render(
        &self,
        element: &LayoutElement,
        ctx: &DataContext,
        render_ctx: &RenderContext,
        painter: &mut dyn Painter,
    ) -> Result<(), RenderError>;
}

/// A global registry of custom element types.
///
/// Games or plugins register their custom element implementations here
/// before layout rendering begins. The layout engine looks up elements
/// by their `type` string at render time and dispatches to the
/// corresponding [`CustomElement`] if found.
///
/// # Example
///
/// ```ignore
/// let mut registry = ElementRegistry::new();
/// registry.register(Box::new(MyMonsterSpriteElement));
///
/// if let Some(element) = registry.get("custom:monster_sprite") {
///     element.render(&layout_elem, &ctx, &render_ctx, &mut painter)?;
/// }
/// ```
pub struct ElementRegistry {
    elements: HashMap<String, Box<dyn CustomElement>>,
}

impl ElementRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            elements: HashMap::default(),
        }
    }

    /// Register a custom element implementation.
    ///
    /// The element's [`element_type`](CustomElement::element_type) is used
    /// as the lookup key. If an element with the same type is already
    /// registered, it is replaced.
    pub fn register(&mut self, element: Box<dyn CustomElement>) {
        let type_name = element.element_type().to_string();
        self.elements.insert(type_name, element);
    }

    /// Look up a custom element by its type name.
    ///
    /// Returns `None` if no element with the given type has been registered.
    pub fn get(&self, type_name: &str) -> Option<&dyn CustomElement> {
        self.elements.get(type_name).map(|e| e.as_ref())
    }

    /// Check whether a custom element type has been registered.
    pub fn contains(&self, type_name: &str) -> bool {
        self.elements.contains_key(type_name)
    }

    /// Validate every `custom:*` element of `layout` against the registered
    /// schemas. Call once when a layout is loaded — render-time dispatch does
    /// no checking.
    ///
    /// Checks per element (recursing into `border`/`group` children):
    /// - the element type is registered;
    /// - every `required` schema prop is present;
    /// - present schema props match their declared [`PropType`].
    ///
    /// Props outside the schema are NOT flagged: the DSL compiler already
    /// rejects undeclared props at build time, and standard layout props
    /// (`align`, `padding`, …) may legitimately accompany any element.
    pub fn validate_layout(&self, layout: &ScreenLayout) -> Result<(), Vec<SchemaViolation>> {
        let mut violations = Vec::new();
        for element in &layout.elements {
            self.validate_element(element, &mut violations);
        }
        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations)
        }
    }

    fn validate_element(&self, element: &LayoutElement, out: &mut Vec<SchemaViolation>) {
        // Recurse into containers first.
        match &element.params {
            ElementParams::Border(b) => {
                for child in &b.children {
                    self.validate_element(child, out);
                }
            }
            ElementParams::Group(g) => {
                for child in &g.children {
                    self.validate_element(child, out);
                }
            }
            _ => {}
        }

        if !element.element_type.starts_with("custom:") {
            return;
        }
        let label = if element.id.is_empty() {
            format!("<type:{}>", element.element_type)
        } else {
            element.id.clone()
        };

        let Some(custom) = self.get(&element.element_type) else {
            out.push(SchemaViolation {
                element: label,
                message: format!(
                    "element type '{}' is not registered (registered: {})",
                    element.element_type,
                    if self.elements.is_empty() {
                        "none".to_string()
                    } else {
                        self.elements.keys().cloned().collect::<Vec<_>>().join(", ")
                    }
                ),
            });
            return;
        };

        let schema = custom.schema();
        if schema.props.is_empty() {
            return;
        }
        let empty = serde_json::Map::new();
        let params = match &element.params {
            ElementParams::Custom(serde_json::Value::Object(map)) => map,
            _ => &empty,
        };
        for spec in &schema.props {
            match params.get(spec.name) {
                Some(value) => {
                    if !spec.ty.matches(value) {
                        out.push(SchemaViolation {
                            element: label.clone(),
                            message: format!(
                                "prop '{}' expects a {} value, got {}",
                                spec.name,
                                spec.ty.name(),
                                value
                            ),
                        });
                    }
                }
                None if spec.required => {
                    out.push(SchemaViolation {
                        element: label.clone(),
                        message: format!(
                            "missing required prop '{}' ({})",
                            spec.name,
                            spec.ty.name()
                        ),
                    });
                }
                None => {}
            }
        }
    }
}

impl Default for ElementRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_empty() {
        let registry = ElementRegistry::new();
        assert!(!registry.contains("custom:test"));
        assert!(registry.get("custom:test").is_none());
    }

    #[test]
    fn test_register_and_get() {
        #[derive(Debug)]
        struct TestElement;

        impl CustomElement for TestElement {
            fn element_type(&self) -> &'static str {
                "custom:test"
            }

            fn render(
                &self,
                _element: &LayoutElement,
                _ctx: &DataContext,
                _render_ctx: &RenderContext,
                _painter: &mut dyn Painter,
            ) -> Result<(), RenderError> {
                Ok(())
            }
        }

        let mut registry = ElementRegistry::new();
        registry.register(Box::new(TestElement));

        assert!(registry.contains("custom:test"));
        assert!(registry.get("custom:test").is_some());
    }

    #[test]
    fn test_register_replaces_existing() {
        #[derive(Debug)]
        struct FirstElement;

        impl CustomElement for FirstElement {
            fn element_type(&self) -> &'static str {
                "custom:replace_me"
            }

            fn render(
                &self,
                _element: &LayoutElement,
                _ctx: &DataContext,
                _render_ctx: &RenderContext,
                _painter: &mut dyn Painter,
            ) -> Result<(), RenderError> {
                Ok(())
            }
        }

        #[derive(Debug)]
        struct SecondElement;

        impl CustomElement for SecondElement {
            fn element_type(&self) -> &'static str {
                "custom:replace_me"
            }

            fn render(
                &self,
                _element: &LayoutElement,
                _ctx: &DataContext,
                _render_ctx: &RenderContext,
                _painter: &mut dyn Painter,
            ) -> Result<(), RenderError> {
                Ok(())
            }
        }

        let mut registry = ElementRegistry::new();
        registry.register(Box::new(FirstElement));
        registry.register(Box::new(SecondElement));

        // The second registration should replace the first
        assert!(registry.contains("custom:replace_me"));
    }

    #[test]
    fn test_multiple_custom_elements() {
        #[derive(Debug)]
        struct SpriteElement;
        impl CustomElement for SpriteElement {
            fn element_type(&self) -> &'static str {
                "custom:sprite"
            }
            fn render(
                &self,
                _element: &LayoutElement,
                _ctx: &DataContext,
                _render_ctx: &RenderContext,
                _painter: &mut dyn Painter,
            ) -> Result<(), RenderError> {
                Ok(())
            }
        }

        #[derive(Debug)]
        struct HpBarElement;
        impl CustomElement for HpBarElement {
            fn element_type(&self) -> &'static str {
                "custom:hp_bar"
            }
            fn render(
                &self,
                _element: &LayoutElement,
                _ctx: &DataContext,
                _render_ctx: &RenderContext,
                _painter: &mut dyn Painter,
            ) -> Result<(), RenderError> {
                Ok(())
            }
        }

        let mut registry = ElementRegistry::new();
        registry.register(Box::new(SpriteElement));
        registry.register(Box::new(HpBarElement));

        assert!(registry.contains("custom:sprite"));
        assert!(registry.contains("custom:hp_bar"));
        assert!(!registry.contains("custom:unknown"));
    }

    // ── validate_layout ───────────────────────────────────────────────

    /// `custom:gauge` expecting `current`/`max` (expr, required) and
    /// `segments` (int, optional).
    #[derive(Debug)]
    struct GaugeElement;

    impl CustomElement for GaugeElement {
        fn element_type(&self) -> &'static str {
            "custom:gauge"
        }

        fn schema(&self) -> ComponentSchema {
            ComponentSchema::new(vec![
                PropSpec::required("current", PropType::Expr),
                PropSpec::required("max", PropType::Expr),
                PropSpec::optional("segments", PropType::Int),
            ])
        }

        fn render(
            &self,
            _element: &LayoutElement,
            _ctx: &DataContext,
            _render_ctx: &RenderContext,
            _painter: &mut dyn Painter,
        ) -> Result<(), RenderError> {
            Ok(())
        }
    }

    fn gauge_registry() -> ElementRegistry {
        let mut registry = ElementRegistry::new();
        registry.register(Box::new(GaugeElement));
        registry
    }

    fn layout_with(element_json: &str) -> ScreenLayout {
        let json = format!(
            r##"{{
                "schema_version": 2,
                "screen": "test",
                "theme": {{ "bg_color": "#FFFFFF", "default_font": "default" }},
                "elements": [{element_json}]
            }}"##
        );
        crate::layout_engine::deserialize::parse_layout(&json).expect("layout should parse")
    }

    #[test]
    fn validate_accepts_well_formed_custom_element() {
        let layout = layout_with(
            r#"{ "type": "custom:gauge", "rect": { "tx": 1, "ty": 2, "tw": 6, "th": 1 },
                 "current": "{hp}", "max": 20, "segments": 4 }"#,
        );
        assert!(gauge_registry().validate_layout(&layout).is_ok());
    }

    #[test]
    fn validate_flags_unregistered_custom_type() {
        let layout = layout_with(
            r#"{ "type": "custom:sparkline", "rect": { "tx": 0, "ty": 0, "tw": 1, "th": 1 } }"#,
        );
        let violations = gauge_registry().validate_layout(&layout).unwrap_err();
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("not registered"));
        assert!(
            violations[0].message.contains("custom:gauge"),
            "lists registered types"
        );
    }

    #[test]
    fn validate_flags_missing_required_prop() {
        let layout = layout_with(
            r#"{ "id": "hp", "type": "custom:gauge",
                 "rect": { "tx": 0, "ty": 0, "tw": 6, "th": 1 }, "current": "{hp}" }"#,
        );
        let violations = gauge_registry().validate_layout(&layout).unwrap_err();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].element, "hp");
        assert!(violations[0]
            .message
            .contains("missing required prop 'max'"));
    }

    #[test]
    fn validate_flags_prop_type_mismatch() {
        let layout = layout_with(
            r#"{ "type": "custom:gauge", "rect": { "tx": 0, "ty": 0, "tw": 6, "th": 1 },
                 "current": "{hp}", "max": 20, "segments": "four" }"#,
        );
        let violations = gauge_registry().validate_layout(&layout).unwrap_err();
        assert_eq!(violations.len(), 1);
        assert!(violations[0]
            .message
            .contains("'segments' expects a int value"));
    }

    #[test]
    fn validate_recurses_into_border_children() {
        let layout = layout_with(
            r#"{ "type": "border", "rect": { "tx": 0, "ty": 0, "tw": 20, "th": 18 },
                 "children": [
                   { "type": "custom:gauge", "rect": { "tx": 1, "ty": 1, "tw": 6, "th": 1 } }
                 ] }"#,
        );
        let violations = gauge_registry().validate_layout(&layout).unwrap_err();
        assert_eq!(
            violations.len(),
            2,
            "missing current AND max: {violations:?}"
        );
    }

    #[test]
    fn validate_ignores_extra_and_standard_props() {
        // Undeclared props are the DSL compiler's concern; the runtime check
        // stays lenient so standard layout props never false-positive.
        let layout = layout_with(
            r#"{ "type": "custom:gauge", "rect": { "tx": 0, "ty": 0, "tw": 6, "th": 1 },
                 "current": "{hp}", "max": 20, "align": "center", "padding": [1] }"#,
        );
        assert!(gauge_registry().validate_layout(&layout).is_ok());
    }
}
