//! One frame: the epochs, the analyser, the keyboard language before anything is drawn, the app
//! bar, the graph, the meter and the column.

use egui::{Key, Modifiers, Stroke, Ui, vec2};
use mxm_ui::navigation;
use mxm_ui::space::{SPACE_3, SPACE_5};
use mxm_ui::theme::Tokens;
use mxm_ui::visual::EMPHASIS_STROKE;
use nice_plug::context::gui::GuiContext;

use super::{CARDS, EqApp, METER_RAIL, history};
use crate::view::ViewSettings;

/// How long a pause in the wheel ends its gesture.
pub const WHEEL_GESTURE_SECONDS: f64 = 0.4;

impl EqApp {
    /// Whether another surface owns the keyboard this frame: a value being typed or the preset
    /// browser. The cursor reads text fields and open menus itself.
    pub(crate) fn inert(&self) -> bool {
        self.text.values().any(Option::is_some) || self.presets.holds_the_keyboard()
    }

    /// The frame's blocks, after the spectrum has observed the frame's time (`now`), so audio after
    /// a silence restarts its windows.
    fn drain_analyser(&mut self, settings: &ViewSettings, now: f64) {
        let rate = self.telemetry.sample_rate();
        // Any change, the rule `ingest` keeps: a block at another rate is never analysed.
        if rate != self.spectrum.sample_rate() {
            self.spectrum = super::spectrum::Spectrum::new(rate);
            self.unfreeze();
        }
        // A new rate starts a listen again, as it does the display's windows (plan §7.5).
        if rate != self.finder.sample_rate() {
            self.finder = super::finder::Finder::new(rate);
        }
        // Back on, it starts from silence: what was drawn before it went off is not what plays.
        if settings.analyser && !self.analyser_on {
            self.spectrum.restart();
        }
        self.analyser_on = settings.analyser;
        self.spectrum.set_resolution(settings.resolution);
        self.spectrum.observe(now);
        if let Some(consumer) = &mut self.analyser {
            while let Ok(block) = consumer.pop() {
                if settings.analyser {
                    self.spectrum.ingest(&block);
                    if self.find {
                        self.finder.ingest(&block);
                    }
                }
            }
        }
    }

    /// Undo and redo, which the keyboard language leaves to the editor: `Command` chords pass it by.
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let (undo, redo) = ctx.input_mut(|input| {
            let redo = input.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)
                || input.consume_key(Modifiers::COMMAND, Key::Y);
            let undo = input.consume_key(Modifiers::COMMAND, Key::Z);
            (undo, redo)
        });
        if redo {
            self.redo();
        } else if undo {
            self.undo();
        }
    }

    /// The whole editor, into a caller's `Ui`: the CLAP window's, or a test's.
    pub fn panel(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
        let tokens = *mxm_ui::theme::tokens(&ctx);

        {
            let params = self.params.clone();
            history::lock(&self.history).settle(|index| params.param(index).normalised());
        }
        let restored = history::lock(&self.history).observe_restores(self.params.view.restores());
        if restored {
            self.abandon_gesture(&ctx);
            // A restore ends a freeze (plan §7.4).
            self.unfreeze();
        }
        let settings = self.params.view.get();
        self.telemetry.set_analyser_on(settings.analyser);
        self.telemetry.set_side_chain_view(settings.side_chain);
        // Find needs the analyser: the analyser off ends a listen, and back on leaves it off
        // (plan §7.5).
        if !settings.analyser && self.find {
            self.end_find();
        }
        let now = ctx.input(|input| input.time);
        self.drain_analyser(&settings, now);
        self.end_stale_freeze(&settings);
        self.update_find(now, &settings);
        self.pointer_solo = None;
        // A band turned Off outside the editor (automation, the host) takes its gesture with it,
        // ended where the host has it.
        let removed = |app: &Self, slot: Option<usize>| slot.is_some_and(|s| !app.band_in_use(s));
        let dragged = self.drag.as_ref().map(|drag| drag.slot);
        let open = history::lock(&self.history).gesture_open() || !self.open.is_empty();
        if (removed(self, self.selected) || removed(self, dragged)) && open {
            self.abandon_gesture(&ctx);
        }
        if removed(self, self.selected) {
            self.selected = None;
            self.edge = None;
        }
        // A pause in the wheel ends its gesture, and so does a press: a click on the preset bar must
        // not land inside it. (A keyboard gesture the press ends through the language.)
        if self.wheel_until.is_some_and(|until| {
            ctx.input(|input| input.time > until || input.pointer.any_pressed())
        }) {
            self.close_open();
        }

        // The keyboard language, before anything is drawn: the cursor over the cards and the bars,
        // then what it handed the graph.
        let inert = self.inert();
        navigation::paged_with_bar(&ctx, &mut self.nav, inert, &CARDS);
        for output in navigation::surface_outputs(&ctx) {
            self.on_surface(output);
        }
        // The narrow layout scrolls, sideways and down the column: the frame the cursor moves, its
        // target (where it was drawn last frame) is scrolled into view.
        let target = (
            self.nav.bar(),
            self.nav.card(),
            self.nav.parameter().map(str::to_owned),
        );
        // So does a change of the window's size or zoom, on that frame and the next: the first still
        // has last frame's layout, the second the new one.
        let view = (ctx.content_rect().size(), ctx.zoom_factor());
        if view != self.last_view {
            self.last_view = view;
            self.rescroll = 2;
        }
        if target != self.last_target || self.rescroll > 0 {
            self.rescroll = self.rescroll.saturating_sub(1);
            if let (None, Some(card), Some(key)) = &target
                && navigation::shown(&ctx)
                && let Some(spot) = navigation::spots(&ctx)
                    .into_iter()
                    .find(|spot| spot.card == *card && spot.key == *key)
            {
                self.scroll_to = Some(spot.rect);
            }
            self.last_target = target;
        }
        if !inert {
            self.handle_shortcuts(&ctx);
        }

        self.app_bar(ui, &tokens);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(tokens.canvas)
                    .inner_margin(egui::Margin::same(SPACE_5 as i8)),
            )
            .show(ui, |ui| self.surface(ui, &tokens));

        self.publish_solo();
    }

    fn app_bar(&mut self, ui: &mut Ui, tokens: &Tokens) {
        let gui = self.gui.clone();
        let setter = gui.as_ref().map(GuiContext::param_setter);
        let (can_undo, can_redo) = {
            let history = history::lock(&self.history);
            (history.can_undo(), history.can_redo())
        };
        let mut undo = false;
        let mut redo = false;
        mxm_ui::AppBar::new(crate::NAME).show_with(
            ui,
            tokens,
            |ui| {
                if let Some(setter) = setter.as_ref() {
                    self.start_replacing();
                    mxm_preset::ui::preset_row(
                        ui,
                        tokens,
                        self.params.as_ref(),
                        setter,
                        &mut self.presets,
                    );
                    self.finish_replacing(ui.ctx());
                }
                undo = ui
                    .add_enabled(can_undo, egui::Button::new("Undo"))
                    .on_hover_text("Takes back the last change to the bands.")
                    .clicked();
                redo = ui
                    .add_enabled(can_redo, egui::Button::new("Redo"))
                    .on_hover_text("Puts back what Undo took back.")
                    .clicked();
            },
            |ui| {
                mxm_ui::shell::zoom_control(ui);
                mxm_ui::shell::editor_theme_control(ui);
            },
        );
        if undo {
            self.undo();
        }
        if redo {
            self.redo();
        }
        if let Some(setter) = setter.as_ref() {
            self.start_replacing();
            mxm_preset::ui::overlays(ui, tokens, self.params.as_ref(), setter, &mut self.presets);
            self.finish_replacing(ui.ctx());
        }
    }

    /// Around the preset bar's calls: whatever it writes replaces the patch, a new undo epoch.
    pub(crate) fn start_replacing(&self) {
        history::lock(&self.history).start_replacing();
    }

    /// Ends [`start_replacing`](Self::start_replacing); a replacement ends any gesture still open,
    /// as a host restore does.
    pub(crate) fn finish_replacing(&mut self, ctx: &egui::Context) {
        let replaced = history::lock(&self.history).finish_replacing();
        if replaced {
            self.abandon_gesture(ctx);
            self.unfreeze();
        }
    }

    fn surface(&mut self, ui: &mut Ui, tokens: &Tokens) {
        let available = ui.available_size();
        let floor = super::GRAPH_FLOOR + SPACE_3 + METER_RAIL + SPACE_3 + super::COLUMNS_WIDTH;
        let width = available.x.max(floor);
        if width > available.x + 0.5 {
            // Narrower than graph, meter and columns: the whole surface scrolls sideways, the
            // columns still beside the graph (plan §5.1, §7.5).
            let height = (available.y - ui.spacing().scroll.bar_width - SPACE_3).max(200.0);
            egui::ScrollArea::horizontal()
                .id_salt("mxm-fx-eq-n-surface")
                .auto_shrink([false, false])
                .show(ui, |ui| self.surface_row(ui, tokens, width, height));
        } else {
            self.surface_row(ui, tokens, width, available.y);
        }
    }

    fn surface_row(&mut self, ui: &mut Ui, tokens: &Tokens, width: f32, height: f32) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let graph_width = width - super::COLUMNS_WIDTH - METER_RAIL - 2.0 * SPACE_3;
            navigation::bar_card(ui, super::GRAPH_CARD, |ui| {
                navigation::at(ui, super::GRAPH_KEY, |ui| {
                    self.graph(ui, tokens, vec2(graph_width, height));
                });
                navigation::paint_card(ui, tokens, super::GRAPH_CARD, self.graph_rect);
                self.graph_rect
            });
            ui.add_space(SPACE_3);
            self.meter_rail(ui, tokens, height);
            ui.add_space(SPACE_3);
            // Two columns of cards (decision 30): the Band card alone, then Output, Analyser and
            // Resonances. They scroll vertically together.
            ui.allocate_ui_with_layout(
                vec2(super::COLUMNS_WIDTH, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(super::COLUMNS_WIDTH);
                    egui::ScrollArea::vertical()
                        .id_salt("mxm-fx-eq-n-column")
                        .auto_shrink([false, false])
                        .max_height(height)
                        .show(ui, |ui| {
                            let column =
                                (super::COLUMNS_WIDTH - ui.spacing().scroll.bar_width - SPACE_3)
                                    / 2.0;
                            ui.horizontal_top(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                for first in [true, false] {
                                    if !first {
                                        ui.add_space(SPACE_3);
                                    }
                                    ui.allocate_ui_with_layout(
                                        vec2(column, 0.0),
                                        egui::Layout::top_down(egui::Align::Min),
                                        |ui| {
                                            ui.set_width(column);
                                            if first {
                                                self.first_column(ui, tokens);
                                            } else {
                                                self.second_column(ui, tokens);
                                            }
                                        },
                                    );
                                }
                            });
                            self.column_extent = (ui.min_rect().height(), height);
                            // A scroll area takes only the requests made inside it, both axes.
                            if let Some(rect) = self.scroll_to {
                                ui.scroll_to_rect(rect, None);
                            }
                        });
                },
            );
            // Again for the surface's sideways area, which the column's has not passed it on to.
            if let Some(rect) = self.scroll_to.take() {
                ui.scroll_to_rect(rect, None);
            }
        });
    }

    /// The Out meter, as tall as the graph: post output gain, so it reads what leaves the plugin.
    fn meter_rail(&mut self, ui: &mut Ui, tokens: &Tokens, height: f32) {
        let now = ui.input(|input| input.time);
        let peaks = self
            .telemetry
            .output
            .each_ref()
            .map(mxm_telemetry::PeakMeter::take);
        self.meters.update(now, peaks);
        let clipped = self
            .telemetry
            .output
            .each_ref()
            .map(mxm_telemetry::PeakMeter::clipped);
        ui.allocate_ui_with_layout(
            vec2(METER_RAIL, height),
            egui::Layout::left_to_right(egui::Align::Min),
            |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for (channel, name) in ["Out left", "Out right"].into_iter().enumerate() {
                    if mxm_ui::shell::vertical_level_meter(
                        ui,
                        tokens,
                        name,
                        self.meters.output[channel],
                        clipped[channel],
                        height,
                    ) {
                        self.telemetry.output[channel].clear_clip();
                    }
                }
            },
        );
    }
}

impl mxm_editor::Panel for EqApp {
    /// The window is open and its setup applied, the keymap newDAWn and every editor share
    /// included: the history's recording context wraps the host's.
    fn opened(&mut self, _window: &egui::Context, host: &GuiContext) {
        self.attach(host.clone());
    }

    fn ui(&mut self, ui: &mut Ui, _host: &GuiContext) {
        self.panel(ui);
    }

    fn closed(&mut self, _host: &GuiContext) {
        self.detach();
    }
}

/// The graph's border: thick where the keyboard's cursor is and showing, never a colour alone.
pub(crate) fn view_border(focused: bool, tokens: &Tokens) -> Stroke {
    if focused {
        Stroke::new(EMPHASIS_STROKE, tokens.focus)
    } else {
        Stroke::new(mxm_ui::visual::AXIS_STROKE, tokens.border)
    }
}
