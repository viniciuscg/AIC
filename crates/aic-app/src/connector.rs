//! Ponte com Notion/Slack. Stub: o "abrir card" e a busca via `claude -p` entram aqui.

use aic_core::Id;
use egui::Context;

use crate::app::{App, ToastKind};
use crate::ui::modals::Modal;

/// Estado do diálogo "Abrir card".
#[derive(Debug, Clone, Default)]
pub struct OpenCardForm {
    pub link: String,
}

pub fn open_card_dialog(app: &mut App) {
    app.modal = Some(Modal::OpenCard(OpenCardForm::default()));
}

pub fn open_card_modal(app: &mut App, _ctx: &Context, _f: OpenCardForm) -> Option<Modal> {
    app.toast("Abrir card: em construção", ToastKind::Info);
    None
}

pub fn refresh_card(app: &mut App, _task: Id) {
    app.toast("Buscar no Notion: em construção", ToastKind::Info);
}

pub fn summary_of(_app: &App, _task: Id) -> Option<String> {
    None
}
