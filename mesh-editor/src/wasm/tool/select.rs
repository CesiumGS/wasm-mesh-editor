//! JS adapters for [`SelectTool`].

use serde::Deserialize;
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

use crate::{
    MeshEditor, SelectAction, SelectModifiersPolicy, SelectTool, ToolActivationError, ToolId,
};

/// The `SelectAction` for each modifier. Omitted modifiers keep their defaults: replacing, and
/// toggling with Shift or Ctrl/Cmd.
#[derive(Deserialize, Tsify, Default)]
#[serde(default, rename_all = "camelCase")]
#[tsify(rename = "SelectModifiersPolicy")]
pub struct SelectModifiersPolicyJs {
    #[tsify(optional, type = "SelectAction")]
    no_modifier: Option<u8>,
    #[tsify(optional, type = "SelectAction")]
    shift: Option<u8>,
    /// Ctrl or Meta (Cmd). Takes precedence over `shift`.
    #[tsify(optional, type = "SelectAction")]
    control: Option<u8>,
}

#[wasm_bindgen]
impl SelectTool {
    /// Adds a new, inactive select tool to `editor` and returns its ID.
    /// Each call adds a separate tool. Throws for an unknown `SelectAction`.
    #[wasm_bindgen(js_name = addTo)]
    pub fn add_to(
        editor: &mut MeshEditor,
        #[wasm_bindgen(js_name = modifiersPolicy)] modifiers_policy: Option<
            Ts<SelectModifiersPolicyJs>,
        >,
    ) -> Result<ToolId, JsError> {
        let modifiers_policy = modifiers_policy_from_js(modifiers_policy.as_ref())?;

        Ok(editor.add_tool(SelectTool { modifiers_policy }).id())
    }

    /// Replaces the modifiers policy of select tool `id` in `editor`, like `addTo`. Throws if
    /// `id` isn't a select tool in `editor`, or for an unknown `SelectAction`.
    #[wasm_bindgen(js_name = setModifiersPolicy)]
    pub fn set_modifiers_policy(
        editor: &mut MeshEditor,
        id: ToolId,
        #[wasm_bindgen(js_name = modifiersPolicy)] modifiers_policy: Option<
            Ts<SelectModifiersPolicyJs>,
        >,
    ) -> Result<(), JsError> {
        let modifiers_policy = modifiers_policy_from_js(modifiers_policy.as_ref())?;

        let Some(handle) = editor.tool_handle::<SelectTool>(id) else {
            return Err(match editor.tool(id) {
                Some(_) => JsError::new(&format!("tool {id:?} is not a select tool")),
                None => ToolActivationError::NotFound { id }.into(),
            });
        };

        if let Some(tool) = editor.tool_config_mut(&handle) {
            tool.modifiers_policy = modifiers_policy;
        }

        Ok(())
    }
}

/// Converts the JS policy, keeping defaults for omitted modifiers.
fn modifiers_policy_from_js(
    policy: Option<&Ts<SelectModifiersPolicyJs>>,
) -> Result<SelectModifiersPolicy, JsError> {
    // `Tsify::from_js` rather than `Ts::to_rust`, whose errors name internal Rust types.
    let policy = match policy {
        Some(policy) => SelectModifiersPolicyJs::from_js(policy.js_value())?,
        None => SelectModifiersPolicyJs::default(),
    };
    let defaults = SelectModifiersPolicy::default();

    Ok(SelectModifiersPolicy {
        no_modifier: select_action(policy.no_modifier, defaults.no_modifier, "noModifier")?,
        shift: select_action(policy.shift, defaults.shift, "shift")?,
        control: select_action(policy.control, defaults.control, "control")?,
    })
}

/// `action` as a `SelectAction`, or `default` if omitted.
fn select_action(
    action: Option<u8>,
    default: SelectAction,
    name: &str,
) -> Result<SelectAction, JsError> {
    const ACTIONS: [SelectAction; 4] = [
        SelectAction::Replace,
        SelectAction::Add,
        SelectAction::Remove,
        SelectAction::Toggle,
    ];

    let Some(action) = action else {
        return Ok(default);
    };

    ACTIONS
        .into_iter()
        .find(|&known| known as u8 == action)
        .ok_or_else(|| JsError::new(&format!("modifiersPolicy.{name} must be a SelectAction")))
}
