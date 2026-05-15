use gpui::*;
use adabraka_ui::prelude::*;
use crate::ui::RekaptrWorkspace;
use crate::updater::UpdateState;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

impl RekaptrWorkspace {
    pub(crate) fn render_settings_about(
        &self,
        theme: &Theme,
        view_handle: &WeakEntity<Self>,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        VStack::new()
            .gap_4()
            .max_w(px(800.0))
            .child(
                Card::new().content(
                    VStack::new()
                        .p_12()
                        .items_center()
                        .gap_4()
                        .child(
                            div()
                                .size(px(80.0))
                                .rounded_2xl()
                                .bg(theme.tokens.primary)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(Icon::new("play").size(px(40.0)).text_color(gpui::white()))
                        )
                        .child(
                            VStack::new()
                                .items_center()
                                .child(div().text_2xl().font_weight(FontWeight::BOLD).child("Rekaptr"))
                                .child(div().text_sm().text_color(theme.tokens.muted_foreground).child(format!("Version {CURRENT_VERSION} (Early Access)")))
                        )
                        .child(
                            div()
                                .max_w(px(400.0))
                                .text_center()
                                .text_sm()
                                .text_color(theme.tokens.muted_foreground)
                                .child("A high-performance gaming DVR and instant replay engine built with Rust and GPUI.")
                        )
                        .child(
                            HStack::new()
                                .gap_4()
                                .mt_4()
                                .child(Button::new("about-web", "Website").variant(ButtonVariant::Outline).size(ButtonSize::Sm))
                                .child(Button::new("about-gh", "GitHub").variant(ButtonVariant::Outline).size(ButtonSize::Sm))
                                .child(Button::new("about-discord", "Discord").variant(ButtonVariant::Outline).size(ButtonSize::Sm))
                        )
                )
            )
            .child(self.render_update_card(theme, view_handle))
    }

    fn render_update_card(
        &self,
        theme: &Theme,
        view_handle: &WeakEntity<Self>,
    ) -> impl IntoElement {
        let has_receipt = self.update_has_receipt;
        let state = self.update_state.clone();
        let vh = view_handle.clone();

        let (status_text, status_muted) = match &state {
            UpdateState::Idle => (format!("Current version: {CURRENT_VERSION}"), true),
            UpdateState::Checking => ("Checking for updates…".to_string(), true),
            UpdateState::UpToDate => ("You're on the latest version.".to_string(), true),
            UpdateState::Available { new_version } => (format!("Version {new_version} is available."), false),
            UpdateState::Installing => ("Downloading and installing update…".to_string(), true),
            UpdateState::Installed { new_version } => (format!("Updated to {new_version}. Restart Rekaptr to apply."), false),
            UpdateState::Error(msg) => (format!("Update failed: {msg}"), false),
        };

        let busy = matches!(state, UpdateState::Checking | UpdateState::Installing);
        let show_install = matches!(state, UpdateState::Available { .. });

        let mut card_body = VStack::new()
            .p_6()
            .gap_3()
            .child(div().text_base().font_weight(FontWeight::SEMIBOLD).child("Updates"))
            .child(
                div()
                    .text_sm()
                    .text_color(if status_muted { theme.tokens.muted_foreground } else { theme.tokens.foreground })
                    .child(status_text)
            );

        if !has_receipt {
            card_body = card_body.child(
                div()
                    .text_xs()
                    .text_color(theme.tokens.muted_foreground)
                    .child("Portable build — reinstall via the official installer to enable updates.")
            );
        }

        let check_disabled = !has_receipt || busy;
        let vh_check = vh.clone();
        let mut check_btn = Button::new("updates-check", "Check for updates")
            .variant(ButtonVariant::Outline)
            .size(ButtonSize::Sm);
        if check_disabled {
            check_btn = check_btn.disabled(true);
        } else {
            check_btn = check_btn.on_click(move |_, _, cx| {
                let vh = vh_check.clone();
                let _ = vh.update(cx, |this, cx| {
                    this.update_state = UpdateState::Checking;
                    cx.notify();
                });
                let task = cx.background_spawn(async move {
                    crate::updater::check_for_update()
                });
                let vh = vh_check.clone();
                cx.spawn(|cx: &mut AsyncApp| {
                    let mut cx = cx.clone();
                    async move {
                        let result = task.await;
                        let _ = vh.update(&mut cx, |this, cx| {
                            this.update_state = match result {
                                Ok(Some(v)) => UpdateState::Available { new_version: v },
                                Ok(None) => UpdateState::UpToDate,
                                Err(e) => UpdateState::Error(e),
                            };
                            cx.notify();
                        });
                    }
                }).detach();
            });
        }

        let mut buttons = HStack::new().gap_2().child(check_btn);

        if show_install {
            let vh_install = vh.clone();
            let install_btn = Button::new("updates-install", "Install update")
                .variant(ButtonVariant::Default)
                .size(ButtonSize::Sm)
                .on_click(move |_, _, cx| {
                    let vh = vh_install.clone();
                    let _ = vh.update(cx, |this, cx| {
                        this.update_state = UpdateState::Installing;
                        cx.notify();
                    });
                    let task = cx.background_spawn(async move {
                        crate::updater::install_update()
                    });
                    let vh = vh_install.clone();
                    cx.spawn(|cx: &mut AsyncApp| {
                        let mut cx = cx.clone();
                        async move {
                            let result = task.await;
                            let _ = vh.update(&mut cx, |this, cx| {
                                this.update_state = match result {
                                    Ok(Some(v)) => UpdateState::Installed { new_version: v },
                                    Ok(None) => UpdateState::UpToDate,
                                    Err(e) => UpdateState::Error(e),
                                };
                                cx.notify();
                            });
                        }
                    }).detach();
                });
            buttons = buttons.child(install_btn);
        }

        card_body = card_body.child(buttons);
        Card::new().content(card_body)
    }
}
