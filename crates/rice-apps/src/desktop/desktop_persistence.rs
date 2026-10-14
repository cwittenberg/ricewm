//! Bounded persistence for workspace-specific desktop icon cells.

use std::collections::BTreeMap;

use rice_desktop::{Cell, DesktopItem, DesktopItemId, DesktopItemKind, WorkspaceId};
use rice_native_app_protocol::{
    AppId, Message, RequestId, SettingEntry, SettingsOperation, SettingsRequest, SettingsResponse,
    SettingsResult,
};

use super::{DesktopActorError, DesktopCommand, RiceDesktop, replace_item_patches};

pub(super) const POSITIONS_SETTING: &str = "desktop.icon_positions";
const MAX_RECORDS: usize = 512;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct DesktopPositionStore {
    cells: BTreeMap<(String, String), Cell>,
}

impl DesktopPositionStore {
    pub(super) fn decode(value: &str) -> Self {
        let mut cells = BTreeMap::new();
        for line in value.lines().take(MAX_RECORDS) {
            let mut fields = line.split('\t');
            let (Some(workspace), Some(item), Some(column), Some(row), None) = (
                fields.next().and_then(decode_hex),
                fields.next().and_then(decode_hex),
                fields.next().and_then(|value| value.parse::<u16>().ok()),
                fields.next().and_then(|value| value.parse::<u16>().ok()),
                fields.next(),
            ) else {
                continue;
            };
            if !workspace.is_empty() && !item.is_empty() {
                cells.insert((workspace, item), Cell::new(column, row));
            }
        }
        Self { cells }
    }

    pub(super) fn encode(&self) -> String {
        self.cells
            .iter()
            .take(MAX_RECORDS)
            .map(|((workspace, item), cell)| {
                format!(
                    "{}\t{}\t{}\t{}",
                    encode_hex(workspace),
                    encode_hex(item),
                    cell.column,
                    cell.row
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub(super) fn cells_for(
        &self,
        workspace: &WorkspaceId,
        items: &[DesktopItem],
    ) -> BTreeMap<DesktopItemId, Cell> {
        items
            .iter()
            .filter_map(|item| {
                let key = item_key(item);
                self.cells
                    .get(&(workspace.as_str().to_owned(), key))
                    .copied()
                    .map(|cell| (item.id, cell))
            })
            .collect()
    }

    pub(super) fn replace_workspace(
        &mut self,
        workspace: &WorkspaceId,
        items: &[DesktopItem],
        positions: &[(DesktopItemId, Cell)],
    ) {
        let workspace_name = workspace.as_str().to_owned();
        self.cells
            .retain(|(stored_workspace, _), _| stored_workspace != &workspace_name);
        let keys = items
            .iter()
            .map(|item| (item.id, item_key(item)))
            .collect::<BTreeMap<_, _>>();
        for (id, cell) in positions {
            if let Some(key) = keys.get(id) {
                self.cells
                    .insert((workspace_name.clone(), key.clone()), *cell);
            }
        }
    }
}

pub(super) fn load_request(app: AppId, request: RequestId) -> Message {
    Message::SettingsRequest(SettingsRequest {
        request,
        app,
        operation: SettingsOperation::Load,
    })
}

pub(super) fn save_request(
    app: AppId,
    request: RequestId,
    positions: &DesktopPositionStore,
) -> Message {
    Message::SettingsRequest(SettingsRequest {
        request,
        app,
        operation: SettingsOperation::Set {
            key: POSITIONS_SETTING.to_owned(),
            value: positions.encode(),
        },
    })
}

pub(super) fn value_from_entries(entries: &[SettingEntry]) -> Option<&str> {
    entries
        .iter()
        .find(|entry| entry.key == POSITIONS_SETTING)
        .map(|entry| entry.value.as_str())
}

impl RiceDesktop {
    pub(crate) fn handle_settings_response(
        &mut self,
        response: &SettingsResponse,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if self.auto_arrange_saves.remove(&response.request) {
            return match &response.result {
                SettingsResult::Applied(entry) if entry.key == "desktop.icons.auto_arrange" => {
                    Ok(Vec::new())
                }
                SettingsResult::Error(_) => Ok(Vec::new()),
                _ => Err(DesktopActorError::UnexpectedSettingsResponse),
            };
        }
        if self.positions_load == Some(response.request) {
            self.positions_load = None;
            return match &response.result {
                SettingsResult::Values(entries) => {
                    if let Some(value) = entries
                        .iter()
                        .find(|entry| entry.key == "desktop.icons.auto_arrange")
                        .map(|entry| entry.value.as_str())
                    {
                        self.auto_arrange = value == "true";
                    }
                    self.positions = DesktopPositionStore::decode(
                        value_from_entries(entries).unwrap_or_default(),
                    );
                    self.reapply_persisted_positions()
                }
                SettingsResult::Error(_) => Ok(Vec::new()),
                _ => Err(DesktopActorError::UnexpectedSettingsResponse),
            };
        }
        if !self.position_saves.remove(&response.request) {
            return Err(DesktopActorError::UnexpectedSettingsResponse);
        }
        match &response.result {
            SettingsResult::Applied(entry) if entry.key == POSITIONS_SETTING => Ok(Vec::new()),
            SettingsResult::Error(_) => Ok(Vec::new()),
            _ => Err(DesktopActorError::UnexpectedSettingsResponse),
        }
    }

    pub(super) fn reapply_persisted_positions(
        &mut self,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if self.items.is_empty() {
            return Ok(Vec::new());
        }
        let persisted = self
            .positions
            .cells_for(self.model.workspace(), &self.items);
        for (item, cell) in persisted {
            self.model
                .execute(DesktopCommand::MoveIcon { item, cell })?;
        }
        if self.auto_arrange {
            self.model.execute(DesktopCommand::ArrangeIcons)?;
        }
        let ordered = self.model.grid().ordered_items();
        self.rendered_items = ordered;
        let patches = replace_item_patches(
            &self.items,
            self.model.grid(),
            self.model.selection(),
            &self.thumbnail_cache,
        );
        self.emit(&patches).map(|message| vec![message])
    }
}

fn item_key(item: &DesktopItem) -> String {
    item.path.clone().unwrap_or_else(|| match &item.kind {
        DesktopItemKind::Trash { .. } => "virtual:trash".to_owned(),
        DesktopItemKind::Mount { mount_id } => format!("mount:{mount_id}"),
        _ => format!("virtual:{}", item.label),
    })
}

fn encode_hex(value: &str) -> String {
    value.as_bytes().iter().fold(
        String::with_capacity(value.len() * 2),
        |mut output, byte| {
            use std::fmt::Write as _;
            let _ = write!(output, "{byte:02x}");
            output
        },
    )
}

fn decode_hex(value: &str) -> Option<String> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    let bytes = value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).ok()?;
            u8::from_str_radix(text, 16).ok()
        })
        .collect::<Option<Vec<_>>>()?;
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: u64, path: Option<&str>, kind: DesktopItemKind) -> DesktopItem {
        DesktopItem {
            id: DesktopItemId(id),
            label: format!("item-{id}"),
            icon_id: "document".to_owned(),
            path: path.map(str::to_owned),
            kind,
            read_only: false,
            executable: false,
            available: true,
        }
    }

    #[test]
    fn round_trip_uses_workspace_and_stable_item_identity() {
        let work = WorkspaceId::new("work");
        let items = vec![
            item(8, Some("/root/Desktop/a b.txt"), DesktopItemKind::File),
            item(900, None, DesktopItemKind::Trash { empty: true }),
        ];
        let mut store = DesktopPositionStore::default();
        store.replace_workspace(
            &work,
            &items,
            &[
                (DesktopItemId(8), Cell::new(2, 3)),
                (DesktopItemId(900), Cell::new(0, 1)),
            ],
        );
        let restored = DesktopPositionStore::decode(&store.encode());
        assert_eq!(restored.cells_for(&work, &items).len(), 2);
        let renamed_ids = vec![item(
            88,
            Some("/root/Desktop/a b.txt"),
            DesktopItemKind::File,
        )];
        assert_eq!(
            restored.cells_for(&work, &renamed_ids)[&DesktopItemId(88)],
            Cell::new(2, 3)
        );
    }

    #[test]
    fn malformed_and_excess_records_are_ignored() {
        let decoded = DesktopPositionStore::decode(
            "bad\n776f726b\t2f726f6f742f4465736b746f702f61\t4\t5\textra",
        );
        assert_eq!(decoded, DesktopPositionStore::default());
    }
}
