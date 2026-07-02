//! The **scaling / composition** demo, authored in the `buiy_view` surface — the PW2 showcase
//! for the three composition shapes that PR1's logic tests exercised individually:
//!
//! 1. **Child sub-components + message-lifting.** The embedded [`counter`] is realized TWICE
//!    (left + right), each holding its own [`counter::Counter`] state as a FIELD of the one
//!    parent [`ScalingApp`] — never an independent entity-model. `counter::view(&s.left).map(..)`
//!    lifts the child view's messages into the parent `Msg` (the Elm `Html.map`), and
//!    `counter::update(..).map(Msg::Left)` lifts the child reducer's returned [`Cmd`] (the
//!    [`Cmd::map`] companion). One model, one reducer, one `view` — so the whole-UI record/replay
//!    property (all state on one on-log model) is preserved.
//!
//! 2. **Conditional view.** [`when`] shows/hides a details panel at a stable slot, and the async
//!    status slot swaps `text`↔`button` at a fixed position (a kind-change) — the reconciler
//!    despawn+spawns the changed slot without churning its siblings.
//!
//! 3. **Async as a value.** "Load" returns [`Cmd::task`] straight from `update`: the reducer
//!    launches the effect AS A VALUE, the drain spawns it on the compute pool (non-blocking, so
//!    the counters stay live during the load), and the result folds back through the funnel as a
//!    recorded `Msg::Loaded`. There is **NO** hand-written launch/poll system pair — the exact
//!    DX-3 gap the prototype had to work around, now closed by `Cmd::task` (design §3 #15).
//!
//! Shared by the windowed `scaling_view` bin and the headless `capture_scaling_view` bin.

use std::time::Duration;

use bevy::prelude::*;
use buiy::view::{BuiyViewAppExt, Element, Space, button, column, row, text, when};
use buiy_core::mvu::{Cmd, Model};

/// The reused child component: a plain Counter with its own state, reducer, and view. It is a
/// `Model` in its own right, but here it is embedded as **parent-owned sub-state** (a field), so
/// there is no per-child entity/routing/bind — the parent composes over it.
pub mod counter {
    use super::*;

    #[derive(Default, Debug, Clone, PartialEq, Reflect)]
    pub struct Counter {
        pub count: i32,
    }

    #[derive(Clone, Debug, PartialEq, Reflect)]
    pub enum Msg {
        Inc,
        Dec,
    }

    /// The child reducer. It never emits here, but returning a real [`Cmd`] is what makes the
    /// parent's `.map(Msg::Left)` lift meaningful (and future-proof if it ever does emit).
    pub fn update(c: &mut Counter, m: Msg) -> Cmd<Msg> {
        match m {
            Msg::Inc => c.count += 1,
            Msg::Dec => c.count -= 1,
        }
        Cmd::none()
    }

    /// The child view — reused verbatim by BOTH embeddings.
    pub fn view(c: &Counter) -> Element<Msg> {
        row![
            button("-").on_press(Msg::Dec),
            text!("{}", c.count).size(28.0),
            button("+").on_press(Msg::Inc),
        ]
        .gap(Space::Sm)
        .align_center()
    }
}

/// MODEL — the whole app in ONE component. The two child Counters are parent-owned sub-state
/// (fields), so the parent stays a single `view(&Model)` and the message log stays one on-log
/// model (replay-friendly).
#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct ScalingApp {
    /// Left child Counter (reused component, own state).
    pub left: counter::Counter,
    /// Right child Counter (independent state).
    pub right: counter::Counter,
    /// Drives the [`when`]-gated details panel.
    pub show_details: bool,
    /// Async in-flight flag (the status slot reflects it).
    pub loading: bool,
    /// The last async result (`None` until a load completes / after Clear).
    pub loaded: Option<String>,
    /// How many loads have completed (proves the async result folded exactly once).
    pub loads: u32,
}

impl Model for ScalingApp {
    type Msg = Msg;
}

/// The parent messages. `Left`/`Right` are the child-message lift targets.
#[derive(Clone, Debug, PartialEq, Reflect)]
pub enum Msg {
    /// A message from the LEFT child Counter, lifted via `.map(Msg::Left)`.
    Left(counter::Msg),
    /// A message from the RIGHT child Counter, lifted via `.map(Msg::Right)`.
    Right(counter::Msg),
    /// Toggle the conditional details panel.
    ToggleDetails,
    /// Kick off the async load (flips `loading` and returns the effect as a value).
    Load,
    /// The async result, folded back through the funnel (recorded `Origin::Command`).
    Loaded(String),
    /// Discard the loaded result (drives the button→text kind-swap back).
    Clear,
}

/// The fake-load delay: a real off-thread wall-clock wait so the `loading` window spans several
/// frames (the spinner is observable, the counters stay responsive) — a stand-in for a
/// network/db call. Runs INSIDE the `Cmd::task` future, on the compute pool.
const LOAD_DELAY: Duration = Duration::from_millis(400);

/// UPDATE — the pure reducer. Child messages delegate to the reused `counter::update` on the
/// owned sub-state and LIFT the returned command with [`Cmd::map`]; `Load` returns the async
/// effect AS A VALUE ([`Cmd::task`]) — no out-of-band launch/poll systems.
pub fn update(s: &mut ScalingApp, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::Left(cm) => counter::update(&mut s.left, cm).map(Msg::Left),
        Msg::Right(cm) => counter::update(&mut s.right, cm).map(Msg::Right),
        Msg::ToggleDetails => {
            s.show_details = !s.show_details;
            Cmd::none()
        }
        Msg::Load => {
            s.loading = true;
            s.loaded = None;
            // Async-as-a-value: the drain spawns this, the poll folds `Loaded` back. The
            // counters stay live meanwhile — the effect does not block the funnel.
            Cmd::task(
                async {
                    std::thread::sleep(LOAD_DELAY);
                    "42 rows".to_string()
                },
                Msg::Loaded,
            )
        }
        Msg::Loaded(data) => {
            s.loading = false;
            s.loaded = Some(data);
            s.loads += 1;
            Cmd::none()
        }
        Msg::Clear => {
            s.loaded = None;
            Cmd::none()
        }
    }
}

/// VIEW — one declarative description exercising all three shapes.
pub fn view(s: &ScalingApp) -> Element<Msg> {
    column![
        text("Scaling demo").size(40.0),
        // (1) Two embedded Counter child-components, each own sub-state, message-lifted into the
        //     ONE parent model. `counter::view` + `counter::update` are reused verbatim; `.map`
        //     re-tags both the child's messages (view side) and its Cmd (reducer side).
        row![
            column![
                text("Left").size(16.0),
                counter::view(&s.left).map(Msg::Left)
            ]
            .gap(Space::Sm)
            .align_center(),
            column![
                text("Right").size(16.0),
                counter::view(&s.right).map(Msg::Right)
            ]
            .gap(Space::Sm)
            .align_center(),
        ]
        .gap(Space::Xl)
        .align_center(),
        // (2a) Conditional present↔absent — a plain `if` on the label + a `when` slot.
        button(if s.show_details {
            "Hide details"
        } else {
            "Show details"
        })
        .on_press(Msg::ToggleDetails),
        when(s.show_details, details_panel(s)),
        // (2b) kind-swap + (3) async — Load flips `loading`; the status slot swaps text↔button
        //      across idle→loading→loaded; the Load button (its sibling) keeps identity.
        row![
            button("Load").on_press_maybe((!s.loading).then_some(Msg::Load)),
            async_status(s),
        ]
        .gap(Space::Md)
        .align_center(),
    ]
    .gap(Space::Lg)
    .padding(Space::Xl)
    .align_center()
}

/// The conditional details panel (only realized when `show_details`). Reads the LIVE child
/// sub-state, proving the parent view composes over its children's fields.
fn details_panel(s: &ScalingApp) -> Element<Msg> {
    column![
        text("Details").size(20.0),
        text!("Left + Right = {}", s.left.count + s.right.count).size(18.0),
    ]
    .gap(Space::Sm)
    .padding(Space::Md)
    .align_center()
}

/// The async status slot — a plain Rust branch returning **different kinds** at one position:
/// `Text` while idle/loading, `Button` once loaded. The Text→Button change is the kind-swap the
/// reconciler despawn+spawns without disturbing its sibling (the Load button).
fn async_status(s: &ScalingApp) -> Element<Msg> {
    if s.loading {
        text("Loading…").size(24.0)
    } else if let Some(data) = &s.loaded {
        button(format!("Result: {data}  (Clear)")).on_press(Msg::Clear)
    } else {
        text("(idle — press Load)").size(24.0)
    }
}

/// Install the scaling demo onto an app already carrying the Buiy plugins. The async effect
/// needs no extra systems — `Cmd::task` rides the existing MVU drain + poll.
pub fn install(app: &mut App) -> &mut App {
    app.ui(ScalingApp::default(), update, view)
}
