//! C2 § 5 step 1 / audit § 4: `EditCommand` is reachable from the `buiy` prelude —
//! the editor's seed/set verbs (`Insert`, `SelectAll`) apps/C4 call. C2 adds no
//! `SetValue` variant (agent-interface-owned `EditCommand` surface, umbrella § 2.7).
#[test]
fn edit_command_is_in_the_prelude() {
    use buiy::prelude::*;
    // Compile-only: name the existing seed verb through the prelude path.
    let _cmd: EditCommand = EditCommand::Insert(String::from("x"));
    let _sel: EditCommand = EditCommand::SelectAll;
}
