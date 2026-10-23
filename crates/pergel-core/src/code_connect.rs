//! Code Connect: a main component (or component set) mapped to a component of the user's
//! codebase, so design to code writes `<Button variant="primary">Buy</Button>` instead of
//! rebuilding the component from divs.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::scene::{NodeId, Scene};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CodeLink {
    /// The component's name in code: "Button".
    pub component: String,
    /// How to import it: `import { Button } from "@/components/ui/button"`.
    pub import: String,
    /// Variant property → (value → JSX attributes): { "Property 1": { "Primary": "variant=\"default\"" } }.
    pub props: BTreeMap<String, BTreeMap<String, String>>,
    /// Text layer name → prop that receives its characters (`"children"` puts it inside the tag).
    pub text: BTreeMap<String, String>,
    /// Source file, for the agent's reference.
    pub source: String,
}

impl Scene {
    /// The code link for an instance: its main's own link, else its component set's.
    pub fn code_link_of(&self, main: NodeId) -> Option<&CodeLink> {
        let m = self.node(main)?;
        if let Some(l) = m.code.as_deref() {
            return Some(l);
        }
        self.variant_set_of(main).and_then(|s| self.node(s)).and_then(|s| s.code.as_deref())
    }

    /// JSX for an instance through its code link: attributes from its variant and text.
    pub fn code_connect_jsx(&self, inst: NodeId) -> Option<String> {
        let main = self.main_of(inst)?;
        let link = self.code_link_of(main)?;
        let variant = crate::parse_variant(&self.node(main)?.name);
        let mut attrs: Vec<String> = Vec::new();
        for (prop, value) in &variant {
            if let Some(a) = link.props.get(prop).and_then(|m| m.get(value)).filter(|a| !a.is_empty()) {
                attrs.push(a.clone());
            }
        }
        let mut children = String::new();
        for id in self.subtree(inst) {
            let Some(n) = self.node(id) else { continue };
            let Some(t) = n.text.as_deref() else { continue };
            let Some(prop) = link.text.get(&n.name) else { continue };
            let text = t.content.replace('&', "&amp;").replace('<', "&lt;").replace('{', "&#123;");
            if prop == "children" {
                children = text;
            } else {
                attrs.push(format!("{prop}=\"{}\"", text.replace('"', "&quot;")));
            }
        }
        // Component properties become props: a text property a string, a boolean one {true}/{false}.
        for def in self.prop_defs(main) {
            let prop = crate::component_props::camel(&def.name);
            if attrs.iter().any(|a| a.starts_with(&format!("{prop}="))) || prop.is_empty() {
                continue;
            }
            match self.prop_value(inst, &def) {
                crate::component_props::PropValue::Bool(b) => attrs.push(format!("{prop}={{{b}}}")),
                crate::component_props::PropValue::Text(t) if !link.text.values().any(|p| *p == prop) => {
                    attrs.push(format!("{prop}=\"{}\"", t.replace('"', "&quot;")));
                }
                crate::component_props::PropValue::Swap(m) => {
                    if let Some(c) = self.code_link_of(m) {
                        attrs.push(format!("{prop}={{<{} />}}", c.component));
                    } else if let Some(n) = self.node(m) {
                        attrs.push(format!("{prop}=\"{}\"", n.name.replace('"', "&quot;")));
                    }
                }
                _ => {}
            }
        }
        let open = if attrs.is_empty() { link.component.clone() } else { format!("{} {}", link.component, attrs.join(" ")) };
        Some(if children.is_empty() { format!("<{open} />") } else { format!("<{open}>{children}</{}>", link.component) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::Component;
    use crate::scene::tests::frame;
    use crate::{Color, Node, Text};

    #[test]
    fn instances_become_codebase_components() {
        let mut scene = Scene::default();
        let mut mains = Vec::new();
        for (i, name) in ["Button/Primary", "Button/Ghost"].iter().enumerate() {
            let f = scene.add(frame(name, 0.0, i as f32 * 60.0, 120.0, 40.0));
            scene.update(f, |n| n.component = Some(Component::Main));
            scene.insert(Node::text(Text::new("Label", 14.0, 600), 10.0, i as f32 * 60.0 + 10.0, Color(0xff000000)), Some(f), None);
            mains.push(f);
        }
        let label = scene.node(mains[0]).unwrap().children[0];
        scene.update(label, |n| {
            n.name = "Label".into();
            n.text.as_mut().unwrap().auto_name = false;
        });
        let label2 = scene.node(mains[1]).unwrap().children[0];
        scene.update(label2, |n| {
            n.name = "Label".into();
            n.text.as_mut().unwrap().auto_name = false;
        });
        let set = scene.combine_as_variants(&mains).unwrap();
        let mut link = CodeLink { component: "Button".into(), import: "import { Button } from \"@/components/ui/button\"".into(), ..Default::default() };
        link.props.insert("Property 1".into(), [("Primary".to_string(), "variant=\"default\"".to_string()), ("Ghost".into(), "variant=\"ghost\"".into())].into());
        link.text.insert("Label".into(), "children".into());
        scene.update(set, |n| n.code = Some(Box::new(link)));
        let inst = scene.create_instance(mains[1], None, None, (400.0, 400.0)).unwrap();
        let text = scene.node(inst).unwrap().children[0];
        scene.update(text, |n| n.text.as_mut().unwrap().content = "Cancel".into());
        assert_eq!(scene.code_connect_jsx(inst).unwrap(), "<Button variant=\"ghost\">Cancel</Button>");
    }
}
