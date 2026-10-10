//! Blocking subflows shared by script gifts and repeatable reading menus.
use super::script_bridge::ScriptEffect;
use crate::alloc_prelude::*;

pub fn choice(options: Vec<String>) -> ScriptEffect {
    ScriptEffect::ShowChoice {
        options,
        started: false,
        selected: 0,
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReadingMenu {
    pub options: Vec<String>,
    pub texts: Vec<String>,
    #[serde(default)]
    pub species: Option<Vec<String>>,
    pub selected: u32,
    pub child: Box<ScriptEffect>,
}

impl ReadingMenu {
    pub fn new(options: Vec<String>, texts: Vec<String>) -> Self {
        Self {
            child: Box::new(choice(options.clone())),
            options,
            texts,
            species: None,
            selected: 0,
        }
    }

    pub fn pokemon(options: Vec<String>, species: Vec<String>) -> Self {
        let mut menu = Self::new(options, Vec::new());
        menu.species = Some(species);
        menu
    }

    /// Return true only when the exit heading (or B) was selected.
    pub fn advance(&mut self) -> bool {
        if let ScriptEffect::ShowChoice { selected, .. } = self.child.as_ref() {
            if let Some(species) = &self.species {
                let Some(name) = species.get(*selected as usize) else { return true; };
                self.selected = *selected;
                self.child = Box::new(ScriptEffect::ShowPokedexEntry { species: name.clone(), started: false });
                return false;
            }
            let Some(text) = self.texts.get(*selected as usize) else {
                return true;
            };
            self.selected = *selected;
            self.child = Box::new(ScriptEffect::ShowDialogue { text: text.clone() });
        } else {
            self.child = Box::new(ScriptEffect::ShowChoice {
                options: self.options.clone(),
                started: false,
                selected: self.selected,
            });
        }
        false
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
enum GiftPhase {
    Received,
    Prompt,
    Choice,
    Naming,
    SentToBox,
    Full,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GiftPokemonFlow {
    phase: GiftPhase,
    pub child: Box<ScriptEffect>,
    to_box: bool,
    box_number: u8,
    is_zh: bool,
    display_name: String,
}

impl GiftPokemonFlow {
    pub fn new(
        species: &str,
        player: &str,
        party_count: u8,
        box_count: u8,
        box_number: u8,
        is_zh: bool,
    ) -> Self {
        let display_name = pokered_data::species::Species::from_scene_name(species)
            .map(|s| pokered_data::lang_data::species_name(s, is_zh).to_string())
            .unwrap_or_else(|| species.to_string());
        let full = party_count >= 6 && box_count as usize >= crate::pokemon::pc_box::MONS_PER_BOX;
        let text = if full {
            if is_zh {
                "这个箱子已经装满宝可梦了！".to_string()
            } else {
                "Oops! This Box is\nfull of POKeMON.".to_string()
            }
        } else if is_zh {
            format!("{player}获得了\n{display_name}！")
        } else {
            format!("{player} got\n{display_name}!")
        };
        let child = if full {
            ScriptEffect::ShowDialogue { text }
        } else {
            ScriptEffect::ShowItemDialogue {
                text,
                sound_id: None,
                sound_started: false,
            }
        };
        Self {
            phase: if full {
                GiftPhase::Full
            } else {
                GiftPhase::Received
            },
            child: Box::new(child),
            to_box: party_count >= 6,
            box_number,
            is_zh,
            display_name,
        }
    }

    pub fn advance(&mut self, species: &str, nickname: &mut Option<String>) -> bool {
        match self.phase {
            GiftPhase::Full | GiftPhase::SentToBox => return true,
            GiftPhase::Received => {
                let text = if self.is_zh {
                    format!("要给{}\n取个昵称吗？", self.display_name)
                } else {
                    format!("Do you want to give a\nnickname to {}?", self.display_name)
                };
                self.phase = GiftPhase::Prompt;
                self.child = Box::new(ScriptEffect::PrintFieldText { text });
            }
            GiftPhase::Prompt => {
                self.phase = GiftPhase::Choice;
                self.child = Box::new(choice(if self.is_zh {
                    vec!["是".into(), "否".into()]
                } else {
                    vec!["YES".into(), "NO".into()]
                }));
            }
            GiftPhase::Choice => {
                if !matches!(
                    self.child.as_ref(),
                    ScriptEffect::ShowChoice { selected: 0, .. }
                ) {
                    return self.finish_naming();
                }
                self.phase = GiftPhase::Naming;
                self.child = Box::new(ScriptEffect::NamingScreen {
                    species: species.to_string(),
                    naming_state: None,
                    started: false,
                    result_name: None,
                });
            }
            GiftPhase::Naming => {
                if let ScriptEffect::NamingScreen {
                    result_name: Some(name),
                    ..
                } = self.child.as_ref()
                {
                    if !name.is_empty() {
                        *nickname = Some(name.clone());
                    }
                }
                return self.finish_naming();
            }
        }
        false
    }

    // SendNewMonToBox also calls AskName, before SentToBoxText.
    fn finish_naming(&mut self) -> bool {
        if !self.to_box {
            return true;
        }
        let text = if self.is_zh {
            format!("{}被送到了\n{}号箱子！", self.display_name, self.box_number)
        } else {
            format!(
                "{} was\nsent to BOX {}!",
                self.display_name, self.box_number
            )
        };
        self.phase = GiftPhase::SentToBox;
        self.child = Box::new(ScriptEffect::ShowDialogue { text });
        false
    }
}

#[cfg(test)]
mod fidelity_menu_tests {
    use super::*;
    #[test]
    fn pokemon_entries_reopen_menu_at_previous_cursor_including_after_snapshot() {
        let species = ["Eevee", "Flareon", "Jolteon", "Vaporeon"];
        let mut options: Vec<_> = species.iter().map(|s| s.to_string()).collect();
        options.push("CANCEL".into());
        let mut menu = ReadingMenu::pokemon(options, species.iter().map(|s| s.to_string()).collect());
        for (i, name) in species.iter().enumerate() {
            if let ScriptEffect::ShowChoice { selected, .. } = menu.child.as_mut() { *selected = i as u32; }
            assert!(!menu.advance());
            assert!(matches!(menu.child.as_ref(), ScriptEffect::ShowPokedexEntry { species, .. } if species == name));
            menu = serde_json::from_str(&serde_json::to_string(&menu).unwrap()).unwrap();
            assert!(!menu.advance());
            assert!(matches!(menu.child.as_ref(), ScriptEffect::ShowChoice { selected, .. } if *selected == i as u32));
        }
        if let ScriptEffect::ShowChoice { selected, .. } = menu.child.as_mut() { *selected = 4; }
        assert!(menu.advance());
    }
}
