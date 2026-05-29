**Date:** 2026-05-22
**Status:** active
**Subject:** Unity UI — Unity Technologies institutional posture: proprietary license, Runtime Fee saga, Unity 6 family, technical leadership

# Distribution and governance

Unity Technologies (NYSE: U), headquartered in San Francisco, is the commercial steward of both UGUI and UI Toolkit. Unlike Bevy (open-source, Bevy Foundation 501(c)(3)) or Linebender (Apache-licensed volunteer collective), Unity ships a **proprietary** commercial engine. This shapes UI-stack governance in ways Buiy must understand if Unity is consulted as prior art.

## Company

- **Founded:** 2004 (as Over the Edge Entertainment, Denmark; renamed Unity Technologies 2007).
- **Headquarters:** San Francisco; major engineering presence in Copenhagen, Bellevue, Montreal, Brighton, Tel Aviv (post-IronSource).
- **Public:** Listed NYSE September 2020 as `U`.
- **IronSource merger:** July 2022, USD $4.4bn — combined Unity (engine) + IronSource (mobile-ad/UA platform). All IronSource founders departed during a six-month transition period announced January 2024.

## License and pricing

- **Engine license:** Proprietary. Source code is not publicly available (selected modules — UIElements early on, IL2CPP — have had limited source releases).
- **Tiers (post-2024 reset):**
  - **Unity Personal** — free for individuals and small organisations below a revenue/funding threshold; includes Unity Splash Screen.
  - **Unity Pro / Enterprise** — seat-based annual subscriptions; revenue/funding thresholds gate eligibility.
- **No Runtime Fee.** The 2023 attempt to introduce a per-install runtime fee was fully cancelled September 2024 (see saga below). Pricing reverted to seat-based subscriptions, with subscription price increases.

## The Runtime Fee saga (2023-2024)

Critical context for understanding Unity's governance risk profile.

| Date | Event |
|---|---|
| 2023-09-13 | Unity announces Runtime Fee — per-install fee on games above revenue + install thresholds. Top rate $0.20/install. Effective 2024-01-01. |
| 2023-09-15 to 2023-09-21 | Developer revolt. Boycotts, public commitments to leave Unity. Game studios announce migrations to Godot, Unreal. |
| 2023-09-22 | Unity apologises; partial walk-back. Policy modified: applies only to Unity LTS 2023.1+, not retroactive; thresholds raised. Co-founder David Helgason quoted: `"We fucked up on many levels."` |
| 2023-10 | CEO John Riccitiello departs as president, CEO, chairman. |
| 2024 (Jan-May) | Continued community discontent. Marc Whitten (Unity Create CPO) fronts developer engagement. |
| 2024-06-01 | Marc Whitten resigns from Unity Create role; serves as Strategic Advisor through to a transition date. |
| 2024-09-12 | **Unity cancels the Runtime Fee entirely.** Reverts to seat-based subscription model, with price increases. Effective immediately, applies retroactively to all Unity 6 and prior versions. |

The reversal is permanent as of 2026-05-22. The institutional damage — community trust, studio migrations already underway — is not.

## Technical leadership (current, 2026)

Verified leadership context (from public sources; org subject to change):

- **CEO** — Matthew Bromberg (took role 2024 after Riccitiello departure; from Zynga / EA Mobile background).
- **Unity Create (engine + tools division)** — Marc Whitten resigned 2024-06; replacement / acting leadership has shifted; the public face of UI Toolkit communication has been less consistent post-2024.
- **Engine engineering** — Distributed across Copenhagen (engine core), San Francisco (Editor), Bellevue (graphics + render pipeline), Montreal (multiplayer + cloud). UI Toolkit team historically Montreal-led.

## Release cadence

Unity releases on three streams:

- **LTS (Long-Term Support)** — One LTS per year, supported with biweekly fixes for two years + extended security support. Unity 6.3 LTS (December 2025) is current; Unity 2022.3 LTS preceded; Unity 2021.3 LTS earlier.
- **Tech Stream** — Roughly twice-yearly intermediate releases between LTS marks. Newer features land here first.
- **Supported Update** — Newer model (Unity 6.2 framing) for delivering platform / feature updates faster within an LTS family with smoother upgrade paths.

UI Toolkit feature work lands first in Tech Stream → graduates to LTS.

## Distribution surface for Unity UI

- **Both UGUI and UI Toolkit ship with every Unity installation.** No separate download; built-in modules.
- **Package Manager** lists both under "Built-in" packages. `com.unity.ugui` (UGUI) and `com.unity.ui` (UI Toolkit) are package identifiers but installed by default.
- **App UI** package (`com.unity.dt.app-ui`) — a *first-party widget kit on top of UI Toolkit*, distributed via Package Manager, focused on productivity-app patterns (Material-Design-flavoured + accessibility primitives). Newer (Unity 2023+); aimed at non-game uses of UI Toolkit.
- **Vector Graphics package** — fully integrated into UI Toolkit since Unity 6; SVG import is built-in (no separate package needed).

## Asset Store widget kits

- **DoozyUI** (`Doozy Entertainment`) — UGUI-based, commercial, mature; covers menus, dialogues, popups, animation flows. Used by many indie titles.
- **MoreMountains Feel / NicePack** — feedback / juice / UI animation kit; UGUI-based.
- **UI Accessibility Plugin (UAP)** — Metalpop Games, accessibility bridge predating Unity's official module.
- **NGUI** — original NGUI, predecessor to UGUI; still maintained for legacy projects.

## Implications for Buiy

1. **Proprietary substrate carries governance risk.** The 2023 Runtime Fee was a unilateral pricing change Unity attempted to apply *retroactively*. Open-source Bevy + open-source Buiy cannot do this; the foundation §2.9 commitment to dual MIT/Apache licensing is the structural answer. This is not a hypothetical hedge — Unity's 2023 attempt is the proof.
2. **Closed source forecloses community fixes.** When UI Toolkit lacks Grid, the community cannot ship Grid; only Unity Technologies can. Bevy + Buiy can be patched by anyone. This shows up at the long-tail edge (rarely-used CSS features, accessibility for non-mainstream ATs, mobile-platform-specific bugs).
3. **Single-corporate-steward risk is real.** Roughly 200 days elapsed between Marc Whitten's resignation announcement and the Runtime Fee cancellation — a 200-day window of UI roadmap uncertainty driven by one personnel decision. Bevy's foundation board (cart + 4 named directors) and Linebender's collective lead model both spread this risk.
4. **Asset Store widget ecosystem is a positive analog.** DoozyUI / MoreMountains demonstrate that a third-party widget-kit market is viable on top of an open primitive layer. Buiy's design intent (foundation §1 — Buiy itself ships widgets; third-party widget kits are welcomed) parallels this; the lesson is that *positioning Buiy itself as the canonical kit + open APIs* is structurally compatible with third-party kits emerging.
5. **App UI (`com.unity.dt.app-ui`) — a separate first-party productivity-UI kit on the UI Toolkit base — is the Unity-internal acknowledgment that "comprehensive UI" needs more than the engine ships.** Buiy's `buiy_widgets` crate (foundation §2.8) is the equivalent: a widget catalog sitting on the primitive layer. Don't ship "primitives only and let third parties build widgets" — that's the original UGUI mistake. Ship a canonical kit *with* the primitives.
6. **LTS-cadence prior art is informative.** Unity LTS = yearly, supported two years. Buiy's "rolling-latest-stable Bevy" (foundation §2.9) is structurally different (Bevy minor releases drive Buiy minor releases). Worth occasionally re-examining whether Buiy should adopt an LTS-style cadence for projects that can't migrate per-Bevy-minor; foundation README §5 open question on platform staging is the closest existing place for that debate.
7. **Editor-itself accessibility is unsolved.** Unity Editor a11y is open (community thread cited in [`accessibility.md`](accessibility.md)). Buiy's BSN authoring tool (a separate spec area) inherits this problem class.

## Sources

- Unity Pricing — https://unity.com/products/pricing-updates
- Unity is Canceling the Runtime Fee — https://unity.com/blog/unity-is-canceling-the-runtime-fee
- Unity apologizes, announces revised runtime fee criteria — https://www.theregister.com/2023/09/22/unity_apologizes_announces_revised_runtime/
- Unity scraps runtime fee — https://www.theregister.com/2024/09/12/unity_cancel_runtime_fees/
- Marc Whitten quits Unity — https://mobilegamer.biz/marc-whitten-quits-unity/
- Unity's Marc Whitten Resigns — https://tech.slashdot.org/story/24/05/14/1852236/unitys-marc-whitten-resigns-amid-runtime-fee-controversy
- Unity Runtime Fee is dead — https://mobilegamer.biz/unitys-runtime-fee-is-dead/
- Unity SVP and CMO Carol Carpenter quits — https://mobilegamer.biz/unity-svp-and-cmo-carol-carpenter-quits/
- Unity 6 release support — https://unity.com/releases/unity-6/support
- App UI documentation — https://docs.unity3d.com/Packages/com.unity.dt.app-ui@0.5/manual/accessibility.html
- Buiy foundation architecture §2.9 — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- bevy_ui governance — [`../bevy-ui/governance.md`](../bevy-ui/governance.md)
