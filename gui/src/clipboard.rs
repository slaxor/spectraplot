use eframe::egui;

/// Maps the classic clipboard keys, which egui only knows on Windows, onto its own
/// requests: Shift+Insert pastes, Ctrl+Insert copies, Shift+Delete cuts.
pub fn classic_keys(ctx: &egui::Context) {
    let requests = [
        (
            egui::Modifiers::SHIFT,
            egui::Key::Insert,
            egui::ViewportCommand::RequestPaste,
        ),
        (
            egui::Modifiers::CTRL,
            egui::Key::Insert,
            egui::ViewportCommand::RequestCopy,
        ),
        (
            egui::Modifiers::SHIFT,
            egui::Key::Delete,
            egui::ViewportCommand::RequestCut,
        ),
    ];
    for (modifiers, key, request) in requests {
        if ctx.input_mut(|input| input.consume_key(modifiers, key)) {
            ctx.send_viewport_cmd(request);
            ctx.request_repaint();
        }
    }
}

/// Adds a right-click entry called `label` that puts `text` on the clipboard.
pub fn copy_menu(response: &egui::Response, label: &str, text: &str) {
    response.context_menu(|ui| {
        if ui.button(label).clicked() {
            ui.ctx().copy_text(text.to_string());
            ui.close();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn requests_for(modifiers: egui::Modifiers, key: egui::Key) -> Vec<String> {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..egui::RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| classic_keys(ui.ctx()));
        output.textures_delta.clear();
        output
            .viewport_output
            .values()
            .flat_map(|viewport| {
                viewport
                    .commands
                    .iter()
                    .map(|command| format!("{command:?}"))
            })
            .filter(|command| command.starts_with("Request"))
            .collect()
    }

    #[test]
    fn classic_keys_become_requests() {
        let cases = [
            (
                "shift insert",
                egui::Modifiers::SHIFT,
                egui::Key::Insert,
                vec!["RequestPaste"],
            ),
            (
                "ctrl insert",
                egui::Modifiers::CTRL,
                egui::Key::Insert,
                vec!["RequestCopy"],
            ),
            (
                "shift delete",
                egui::Modifiers::SHIFT,
                egui::Key::Delete,
                vec!["RequestCut"],
            ),
            (
                "plain insert",
                egui::Modifiers::NONE,
                egui::Key::Insert,
                vec![],
            ),
            ("ctrl c", egui::Modifiers::CTRL, egui::Key::C, vec![]),
        ];
        for (name, modifiers, key, want) in cases {
            assert_eq!(requests_for(modifiers, key), want, "{name}");
        }
    }
}
