//! Wasm-only DOM/ARIA accessibility sink (browser-reach widening § D7).
//!
//! AccessKit ships **no** web/canvas adapter (`accesskit_web` does not exist), and
//! `accesskit_winit` selects a null platform impl on wasm — so on the web the
//! [`AccessKitAdapterPlugin`](crate::a11y::AccessKitAdapterPlugin) reaches ZERO
//! assistive technology. This sink closes that gap the only actionable way: it
//! mirrors the frame's `A11yNodeView` tree (the SAME winit-free data
//! [`build_tree_update`](crate::a11y::build_tree_update) consumes) into a
//! **visually-hidden, ARIA-annotated DOM subtree** appended next to the `<canvas>`,
//! so a browser screen reader has a real semantic tree over the canvas.
//!
//! **Scope (v1 — outbound/read-only):** role + accessible name + the key states
//! (checked / expanded / selected / disabled / hidden) + focus, rebuilt on change.
//! **Inbound** AT actions (a screen-reader click/focus routed BACK into the app via
//! the existing `ActionRequest` path) are a named follow-up, not v1. The overlay is
//! kept in the AX tree but off-screen (the standard visually-hidden pattern), not
//! `display:none` (which would drop it from the AX tree).
//!
//! Every DOM call is fallible and swallowed to a no-op — an a11y sink must never
//! crash the app (mirrors the `ClipboardProvider` "must not be optional ≠ must be
//! infallible" contract).

use crate::BuiySet;
use crate::a11y::{A11yRole, A11yTreeBuilder};
use crate::focus::FocusedEntity;
use bevy::ecs::system::NonSendMarker;
use bevy::prelude::*;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// The wasm-only DOM/ARIA a11y sink. Registered by [`crate::a11y::A11yPlugin`] on
/// `target_arch = "wasm32"`, alongside (not instead of) the winit adapter sink —
/// the winit one is an inert no-op on the web (empty `ACCESS_KIT_ADAPTERS`).
pub struct WebA11ySinkPlugin;

impl Plugin for WebA11ySinkPlugin {
    fn build(&self, app: &mut App) {
        // After `build_tree` (so the snapshot is current), in the same a11y set as
        // the winit sink. Main-thread-pinned: the DOM is main-thread-only (same
        // rationale as `push_tree_updates` — MT-safety § D2).
        app.add_systems(
            Update,
            mirror_a11y_to_dom
                .in_set(BuiySet::A11yUpdate)
                .after(crate::a11y::build_tree),
        );
    }
}

const CONTAINER_ID: &str = "buiy-a11y-tree";

/// Map a Buiy a11y role to its ARIA `role` token. `None` ⇒ no `role` attribute
/// (plain text / generic grouping nodes carry only their name).
fn aria_role(role: A11yRole) -> Option<&'static str> {
    Some(match role {
        A11yRole::Button => "button",
        A11yRole::Link => "link",
        A11yRole::Image => "img",
        A11yRole::Heading => "heading",
        A11yRole::Dialog => "dialog",
        A11yRole::AlertDialog => "alertdialog",
        A11yRole::Tooltip => "tooltip",
        A11yRole::Checkbox => "checkbox",
        A11yRole::Switch => "switch",
        A11yRole::Slider => "slider",
        A11yRole::TextInput | A11yRole::MultilineTextInput => "textbox",
        A11yRole::Region => "region",
        A11yRole::Group => "group",
        A11yRole::Menu => "menu",
        A11yRole::MenuItem => "menuitem",
        A11yRole::Status => "status",
        A11yRole::Alert => "alert",
        A11yRole::Log => "log",
        A11yRole::Generic | A11yRole::Text => return None,
    })
}

/// Rebuild the DOM overlay only when the semantic tree changes — a stable AX tree
/// (screen readers re-announce on churn). `Local<u64>` holds the last signature.
fn mirror_a11y_to_dom(
    _main_thread: NonSendMarker,
    builder: Res<A11yTreeBuilder>,
    focused: Res<FocusedEntity>,
    mut last_sig: Local<u64>,
) {
    let snapshot = builder.snapshot();
    let sig = signature(snapshot, focused.0);
    if sig == *last_sig {
        return;
    }
    *last_sig = sig;
    // Swallow any DOM error — the a11y sink must not crash the app.
    let _ = rebuild(snapshot, focused.0);
}

/// A cheap structural signature over the fields the DOM mirrors, so an unchanged
/// tree skips the rebuild.
fn signature(nodes: &[crate::a11y::A11yNodeView], focused: Option<Entity>) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    focused.map(|e| e.to_bits()).hash(&mut h);
    nodes.len().hash(&mut h);
    for n in nodes {
        n.entity.to_bits().hash(&mut h);
        (aria_role(n.role).unwrap_or("")).hash(&mut h);
        n.name.hash(&mut h);
        n.description.hash(&mut h);
        n.disabled.hash(&mut h);
        n.hidden.hash(&mut h);
        n.expanded.hash(&mut h);
        n.selected.hash(&mut h);
        // `Toggled` is not Hash — fold a discriminant byte.
        n.toggled
            .map(|t| match t {
                accesskit::Toggled::False => 0u8,
                accesskit::Toggled::True => 1,
                accesskit::Toggled::Mixed => 2,
            })
            .hash(&mut h);
        n.parent.map(|e| e.to_bits()).hash(&mut h);
        n.children.len().hash(&mut h);
    }
    h.finish()
}

fn rebuild(nodes: &[crate::a11y::A11yNodeView], focused: Option<Entity>) -> Option<()> {
    let document = web_sys::window()?.document()?;

    // Get/create the visually-hidden container appended to <body>.
    let container = match document.get_element_by_id(CONTAINER_ID) {
        Some(c) => c,
        None => {
            let c = document.create_element("div").ok()?;
            c.set_id(CONTAINER_ID);
            // Visually hidden but in the AX tree (NOT display:none). Standard clip
            // pattern; `aria-hidden` intentionally absent so the AT reads it.
            let _ = c.set_attribute(
                "style",
                "position:absolute;width:1px;height:1px;margin:-1px;padding:0;\
                 overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap;border:0;",
            );
            let _ = c.set_attribute("role", "application");
            let _ = c.set_attribute("aria-label", "Buiy application");
            document.body()?.append_child(&c).ok()?;
            c
        }
    };

    // Full rebuild (change-gated by the caller). Clear, then re-append the tree.
    container.set_inner_html("");

    // entity -> node index, for parent/children resolution.
    let index: HashMap<Entity, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.entity, i))
        .collect();

    // entity -> its created DOM element, so children append under their parent.
    let mut els: HashMap<Entity, web_sys::Element> = HashMap::default();

    // Create every node's element first (document order = snapshot order).
    for n in nodes {
        let el = document.create_element("div").ok()?;
        if let Some(role) = aria_role(n.role) {
            let _ = el.set_attribute("role", role);
        }
        if !n.name.is_empty() {
            let _ = el.set_attribute("aria-label", &n.name);
        }
        if !n.description.is_empty() {
            let _ = el.set_attribute("aria-description", &n.description);
        }
        if let Some(t) = n.toggled {
            let v = match t {
                accesskit::Toggled::False => "false",
                accesskit::Toggled::True => "true",
                accesskit::Toggled::Mixed => "mixed",
            };
            let _ = el.set_attribute("aria-checked", v);
        }
        if let Some(exp) = n.expanded {
            let _ = el.set_attribute("aria-expanded", if exp { "true" } else { "false" });
        }
        if let Some(sel) = n.selected {
            let _ = el.set_attribute("aria-selected", if sel { "true" } else { "false" });
        }
        if n.disabled {
            let _ = el.set_attribute("aria-disabled", "true");
        }
        if n.hidden {
            let _ = el.set_attribute("aria-hidden", "true");
        }
        // A stable handle for the (future) inbound action bridge + tests.
        let _ = el.set_attribute("data-buiy-entity", &n.entity.to_bits().to_string());
        // Focus signal: mark the focused node (a real focus()/activedescendant
        // bridge is the inbound follow-up; the marker is AX-observable now).
        if focused == Some(n.entity) {
            let _ = el.set_attribute("data-buiy-focused", "true");
        }
        els.insert(n.entity, el);
    }

    // Nest each element under its a11y parent; roots go under the container.
    for n in nodes {
        let Some(el) = els.get(&n.entity) else {
            continue;
        };
        let parent_el = n
            .parent
            .and_then(|p| index.get(&p).and(els.get(&p)))
            .unwrap_or(&container);
        let _ = parent_el.append_child(el);
    }

    Some(())
}
