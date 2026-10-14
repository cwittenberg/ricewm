//! Shell configuration operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub fn replace_wm_menu_entries(&mut self, entries: Vec<(u32, u8, u8, Vec<u8>)>) -> bool {
        if entries.len() > WM_MENU_MAX_APPLICATIONS
            || entries.iter().any(|(action, _, category, label)| {
                *action == 0
                    || *category > 9
                    || label.is_empty()
                    || label.len() > WM_MENU_MAX_LABEL_BYTES
                    || label.iter().any(u8::is_ascii_control)
            })
        {
            return false;
        }
        self.wm_menu_entries = entries
            .into_iter()
            .map(|(action, icon, category, label)| WmMenuEntry {
                action,
                icon,
                category,
                folder: None,
                uninstallable: false,
                provisionable: false,
                hydrating: false,
                label,
            })
            .collect();
        self.wm_menu_folders.clear();
        self.wm_menu_structured = false;
        self.wm_menu_level = self.wm_menu_initial_level();
        self.wm_menu_offset = 0;
        self.wm_menu_hover_row = None;
        if self.config.native_window_manager {
            self.relayout_wm_menu();
        }
        true
    }

    /// Replace the menu from user-controlled directory paths. Folder paths
    /// are already validated and bounded by the display ABI.
    pub fn replace_wm_structured_menu_entries(
        &mut self,
        entries: Vec<(u32, u8, bool, bool, bool, Vec<Vec<u8>>, Vec<u8>)>,
    ) -> bool {
        if entries.len() > WM_MENU_MAX_APPLICATIONS
            || entries.iter().any(|(action, _, _, _, _, path, label)| {
                *action == 0
                    || path.len() > 8
                    || label.is_empty()
                    || label.len() > WM_MENU_MAX_LABEL_BYTES
                    || label.iter().any(u8::is_ascii_control)
                    || path.iter().any(|part| {
                        part.is_empty()
                            || part.len() > WM_MENU_MAX_LABEL_BYTES
                            || part
                                .iter()
                                .any(|byte| byte.is_ascii_control() || *byte == b'/')
                    })
            })
        {
            return false;
        }
        let mut folders = Vec::<WmMenuFolder>::new();
        let mut mapped = Vec::with_capacity(entries.len());
        for (action, icon, uninstallable, provisionable, hydrating, path, label) in entries {
            let mut folder = None;
            for part in path {
                let index = folders
                    .iter()
                    .position(|candidate| candidate.parent == folder && candidate.label == part)
                    .unwrap_or_else(|| {
                        folders.push(WmMenuFolder {
                            parent: folder,
                            label: part,
                        });
                        folders.len() - 1
                    });
                folder = Some(u8::try_from(index).unwrap_or(u8::MAX));
            }
            mapped.push(WmMenuEntry {
                action,
                icon,
                category: 0,
                folder,
                uninstallable,
                provisionable,
                hydrating,
                label,
            });
        }
        if folders.len() > usize::from(u8::MAX) {
            return false;
        }
        self.wm_menu_entries = mapped;
        self.wm_menu_folders = folders;
        self.wm_menu_structured = true;
        self.wm_menu_level = self.wm_menu_initial_level();
        self.wm_menu_offset = 0;
        self.wm_menu_hover_row = None;
        if self.config.native_window_manager {
            self.relayout_wm_menu();
        }
        true
    }

    pub fn replace_wm_themes(&mut self, themes: Vec<WmThemeRecord>) -> bool {
        let selected_name = self.wm_themes.get(usize::from(self.wm_theme)).map(|theme| {
            theme
                .name
                .iter()
                .map(u8::to_ascii_lowercase)
                .collect::<Vec<_>>()
        });
        let mut names = BTreeSet::new();
        if themes.is_empty()
            || themes.len() > WM_THEME_MAX_COUNT
            || themes.iter().any(|(name, _, _, _, taskbar, menu, frame)| {
                name.is_empty()
                    || name.len() > WM_THEME_MAX_NAME_BYTES
                    || !name[0].is_ascii_alphanumeric()
                    || name
                        .iter()
                        .skip(1)
                        .any(|byte| !byte.is_ascii_alphanumeric() && !b" ._()+&'-".contains(byte))
                    || !names.insert(name.iter().map(u8::to_ascii_lowercase).collect::<Vec<_>>())
                    || taskbar
                        .iter()
                        .chain(menu)
                        .chain(frame)
                        .any(|color| *color > 0x00ff_ffff)
            })
        {
            return false;
        }
        self.wm_themes = themes
            .into_iter()
            .map(
                |(
                    name,
                    backdrop_material,
                    menu_backdrop_material,
                    chrome_profile,
                    taskbar,
                    menu,
                    frame,
                )| WmThemePalette {
                    name,
                    backdrop_material,
                    menu_backdrop_material,
                    chrome_profile,
                    taskbar,
                    menu,
                    frame,
                },
            )
            .collect();
        self.wm_theme = selected_name
            .and_then(|selected| {
                self.wm_themes.iter().position(|theme| {
                    theme
                        .name
                        .iter()
                        .map(u8::to_ascii_lowercase)
                        .eq(selected.iter().copied())
                })
            })
            .and_then(|index| u8::try_from(index).ok())
            .unwrap_or(0);
        self.wm_menu_offset = 0;
        self.wm_menu_hover_row = None;
        if self.config.native_window_manager {
            self.relayout_wm_menu();
            self.publish_gtk_theme_settings();
            self.repaint_wm_chrome();
        }
        true
    }

    /// Select a validated theme from the current bounded catalog.
    pub fn select_wm_theme(&mut self, theme: u8) -> bool {
        if usize::from(theme) >= self.wm_themes.len() {
            return false;
        }
        self.wm_theme = theme;
        self.wm_menu_hover_row = None;
        if self.config.native_window_manager {
            self.publish_gtk_theme_settings();
            self.repaint_wm_chrome();
        }
        true
    }

    #[must_use]
    pub const fn current_workspace(&self) -> u32 {
        self.current_workspace
    }

    /// Atomically replace browser-native application entries in the shared
    /// `RiceWM` taskbar. Native windows keep their browser compositor surfaces,
    /// while this bounded metadata lets the WM own task switching and
    /// workspace presentation for X11 and native applications alike.
    pub fn replace_native_tasks(&mut self, tasks: Vec<WmNativeTask>) -> bool {
        let mut ids = BTreeSet::new();
        if !self.config.native_window_manager
            || tasks.len() > WM_NATIVE_TASK_MAX_COUNT
            || tasks.iter().any(|task| {
                task.id == 0
                    || task.workspace >= if self.scrolling_workspaces {128}else{self.workspace_count}
                    || task.title.is_empty()
                    || task.title.len() > WM_NATIVE_TASK_MAX_TITLE_BYTES
                    || task.title.iter().any(u8::is_ascii_control)
                    || !ids.insert(task.id)
            })
        {
            return false;
        }
        if tasks == self.native_tasks {
            return true;
        }
        self.native_tasks = tasks;
        self.native_output_positions.retain(|id,_|self.native_tasks.iter().any(|t|t.id==*id));
        self.refresh_scrolling_workspaces();
        if self.native_task_owns_input() {
            // A shell/taskbar activation ends a background application's
            // capture, including delayed Chromium XI2 grabs after mouse-up.
            self.pointer_grab = None;
            self.keyboard_grab = None;
        }
        self.wm_chrome_hover = None;
        self.wm_chrome_pressed = None;
        self.repaint_wm_chrome();
        true
    }

    #[must_use]
    pub fn pending_wm_actions(&self) -> usize {
        self.wm_actions.len()
    }

    pub(super) fn native_task_owns_input(&self) -> bool {
        self.config.native_window_manager && self.native_tasks.iter().any(|task| {
            task.workspace == self.current_workspace && task.active && !task.minimized
        })
    }

    pub fn drain_wm_relaunch_command(&mut self, output: &mut [u8]) -> usize {
        let Some(command)=self.wm_relaunch_commands.pop_front() else {return 0;};
        let length=command.len().min(output.len()); output[..length].copy_from_slice(&command[..length]); length
    }

    pub(super) fn request_wm_new_instance(&mut self, target: WmTaskTarget) {
        let member=match target {WmTaskTarget::Group(group)=>self.wm_task_group(group).first().copied(), _=>Some(target),
        };
        match member {
            Some(WmTaskTarget::Native(id))=>{
                self.wm_actions.push_back(0xffe8_0000 | u32::from(id))}
            Some(WmTaskTarget::X11(client))=>{
                let command=self.windows.get(&client.0).and_then(|window|{
                        window.properties.iter().find_map(|(atom,property)| {
                            (self.atoms.name(*atom)==Some("WM_COMMAND") && property.format==8 && property.data.len()<=4096).then(||property.data.clone())})
                    }).unwrap_or_default();
                if self.wm_relaunch_commands.len()<16 {self.wm_relaunch_commands.push_back(command);self.wm_actions.push_back(0xffff_ff12);}
            }
            _=>{}
        }
    }

    pub fn drain_wm_action(&mut self) -> Option<u32> {
        self.wm_actions.pop_front()
    }
}
