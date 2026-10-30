//! The keyword table: a virtualized `DataTable` over the selected app's
//! keywords in one storefront.

use appdoctor_store::{KeywordRow, RankChange};
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::table::{Column, ColumnSort, TableDelegate, TableState};
use gpui_kit::component::{ActiveTheme as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::icons::IconCache;
use crate::rank_check::now;
use crate::state::AppState;
use crate::theme::meter;
use crate::ui::widgets::{self, app_icon};

const ICONS_SHOWN: usize = 5;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Col {
    Keyword,
    Popularity,
    Difficulty,
    Position,
    Change,
    Apps,
    Updated,
}

const COLUMNS: [Col; 7] = [
    Col::Keyword,
    Col::Popularity,
    Col::Difficulty,
    Col::Position,
    Col::Change,
    Col::Apps,
    Col::Updated,
];

pub struct KeywordTable {
    state: Entity<AppState>,
    icons: Entity<IconCache>,
    rows: Vec<KeywordRow>,
    sort: Option<(Col, ColumnSort)>,
}

impl KeywordTable {
    pub fn new(state: Entity<AppState>, icons: Entity<IconCache>) -> Self {
        Self {
            state,
            icons,
            rows: Vec::new(),
            sort: Some((Col::Popularity, ColumnSort::Descending)),
        }
    }

    pub fn set_rows(&mut self, rows: Vec<KeywordRow>) {
        self.rows = rows;
        self.apply_sort();
    }

    fn apply_sort(&mut self) {
        let Some((col, sort)) = self.sort else {
            return;
        };
        let key = |row: &KeywordRow| -> Option<f64> {
            match col {
                Col::Keyword => None,
                Col::Change => Some(change_order(row.change()) as f64),
                Col::Popularity => row.popularity.map(f64::from),
                Col::Difficulty => row.latest.and_then(|check| check.difficulty).map(f64::from),
                Col::Position => row.rank().map(f64::from),
                Col::Apps => row.latest.map(|check| check.results_count as f64),
                Col::Updated => row.latest.map(|check| check.checked_at as f64),
            }
        };
        if col == Col::Keyword {
            self.rows.sort_by(|a, b| a.term.cmp(&b.term));
            if matches!(sort, ColumnSort::Descending) {
                self.rows.reverse();
            }
            return;
        }
        // Rows without a value (never checked, no popularity) go last in
        // both directions, alphabetically.
        let (mut valued, mut missing): (Vec<_>, Vec<_>) = std::mem::take(&mut self.rows)
            .into_iter()
            .partition(|row| key(row).is_some());
        valued.sort_by(|a, b| {
            let order = key(a).unwrap_or_default().total_cmp(&key(b).unwrap_or_default());
            let order = if matches!(sort, ColumnSort::Descending) {
                order.reverse()
            } else {
                order
            };
            order.then_with(|| a.term.cmp(&b.term))
        });
        missing.sort_by(|a, b| a.term.cmp(&b.term));
        valued.extend(missing);
        self.rows = valued;
    }
}

fn change_order(change: RankChange) -> i64 {
    match change {
        RankChange::Up(n) => 1_000 + n as i64,
        RankChange::Entered => 900,
        RankChange::First | RankChange::Same => 0,
        RankChange::Unchecked => -1,
        RankChange::Dropped => -900,
        RankChange::Down(n) => -1_000 - n as i64,
    }
}

impl TableDelegate for KeywordTable {
    fn columns_count(&self, _: &App) -> usize {
        COLUMNS.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        let column = match COLUMNS[col_ix] {
            Col::Keyword => Column::new("keyword", "Keyword").width(px(260.)).min_width(px(160.)),
            Col::Popularity => Column::new("popularity", "Popularity").width(px(120.)),
            Col::Difficulty => Column::new("difficulty", "Difficulty").width(px(120.)),
            Col::Position => Column::new("position", "Position").width(px(92.)),
            Col::Change => Column::new("change", "Change").width(px(84.)),
            Col::Apps => Column::new("apps", "Apps in ranking").width(px(190.)),
            Col::Updated => Column::new("updated", "Last update").width(px(110.)),
        };
        let column = column.sortable();
        match self.sort {
            Some((col, ColumnSort::Descending)) if col == COLUMNS[col_ix] => column.descending(),
            Some((col, ColumnSort::Ascending)) if col == COLUMNS[col_ix] => column.ascending(),
            _ => column,
        }
    }

    fn perform_sort(&mut self, col_ix: usize, sort: ColumnSort, _: &mut Window, cx: &mut Context<TableState<Self>>) {
        self.sort = match sort {
            ColumnSort::Default => None,
            sort => Some((COLUMNS[col_ix], sort)),
        };
        self.apply_sort();
        cx.notify();
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(row) = self.rows.get(row_ix) else {
            return div().into_any_element();
        };
        let muted = cx.theme().muted_foreground;
        let dash = || div().text_color(muted).child("—").into_any_element();
        match COLUMNS[col_ix] {
            Col::Keyword => div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .truncate()
                .child(row.term.clone())
                .into_any_element(),
            Col::Popularity => match row.popularity {
                Some(score) => widgets::meter(score as f32, meter::popularity(score), cx).into_any_element(),
                None => dash(),
            },
            Col::Difficulty => match row.latest.and_then(|check| check.difficulty) {
                Some(score) => widgets::meter(score, meter::difficulty(score), cx).into_any_element(),
                None => dash(),
            },
            Col::Position => match row.latest {
                Some(check) => match check.rank {
                    Some(rank) => h_flex()
                        .gap_1()
                        .items_baseline()
                        .child(div().text_xs().text_color(muted).child("#"))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .when(rank <= 10, |this| this.text_color(cx.theme().primary))
                                .child(rank.to_string()),
                        )
                        .into_any_element(),
                    None => div()
                        .text_sm()
                        .text_color(muted)
                        .child(format!("{}+", check.results_count.max(1)))
                        .into_any_element(),
                },
                None => dash(),
            },
            Col::Change => render_change(row.change(), cx),
            Col::Apps => {
                let urls: Vec<String> = row
                    .top_apps
                    .iter()
                    .take(ICONS_SHOWN)
                    .filter_map(|app| app.icon_url.clone())
                    .collect();
                if urls.is_empty() {
                    return dash();
                }
                let images: Vec<_> = self
                    .icons
                    .update(cx, |icons, cx| urls.iter().map(|url| icons.get(url, cx)).collect());
                let more = row
                    .latest
                    .map(|check| check.results_count as usize)
                    .unwrap_or(0)
                    .saturating_sub(ICONS_SHOWN);
                h_flex()
                    .gap_1()
                    .items_center()
                    .children(images.into_iter().map(|image| app_icon(image, px(20.), cx)))
                    .when(more > 0, |this| {
                        this.child(div().pl_1().text_xs().text_color(muted).child(format!("+{more}")))
                    })
                    .into_any_element()
            }
            Col::Updated => match row.latest {
                Some(check) => div()
                    .text_sm()
                    .text_color(muted)
                    .child(widgets::relative_time(check.checked_at, now()))
                    .into_any_element(),
                None => div().text_sm().text_color(muted).child("Never").into_any_element(),
            },
        }
    }

    fn cell_label(&self, row_ix: usize, col_ix: usize, _: &App) -> Option<SharedString> {
        let row = self.rows.get(row_ix)?;
        Some(match COLUMNS[col_ix] {
            Col::Keyword => row.term.clone().into(),
            Col::Position => row
                .rank()
                .map_or("Not ranked".into(), |rank| format!("Rank {rank}").into()),
            _ => return None,
        })
    }

    fn context_menu(
        &mut self,
        row_ix: usize,
        menu: PopupMenu,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> PopupMenu {
        let Some(row) = self.rows.get(row_ix) else {
            return menu;
        };
        let (id, term) = (row.id, row.term.clone());
        let state = self.state.clone();
        menu.item(
            PopupMenuItem::new(format!("Delete “{term}”")).on_click(move |_, _, cx| {
                state.update(cx, |state, cx| {
                    if let Err(error) = state.delete_keyword(id, cx) {
                        tracing::error!(%error, "deleting keyword");
                    }
                });
            }),
        )
    }
}

fn render_change(change: RankChange, cx: &App) -> AnyElement {
    let (text, color) = widgets::change_label(change, cx);
    div()
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .child(text)
        .into_any_element()
}
