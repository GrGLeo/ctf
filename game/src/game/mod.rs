pub mod algorithms;
pub mod animation;
pub mod board;
pub mod buffs;
pub mod cell;
pub mod entities;
pub mod minion_manager;
pub mod monster_manager;
pub mod projectile_manager;
pub mod spell;

use crate::config::GameConfig;
use crate::packet::board_packet::BoardPacket;
use animation::{AnimationCommand, AnimationTrait};
pub use board::Board;
use buffs::Buff;
use bytes::BytesMut;
use cell::Team;
pub use cell::{BaseTerrain, Cell, CellContent, MinionId, PlayerId, TowerId};
pub use entities::champion::{Action, Champion};
use entities::{
    AttackAction, Fighter, Target,
    base::Base,
    projectile::GameplayEffect,
    tower::{Tower, generate_tower_id},
};
use minion_manager::MinionManager;
use monster_manager::MonsterManager;
use projectile_manager::ProjectileManager;
use spell::Spell;
use tokio::sync::mpsc;

use std::{
    collections::HashMap,
    mem::take,
    time::{Duration, Instant},
    usize, vec,
};

pub type ClientMessage = BytesMut;

pub struct GameManager {
    players_count: usize,
    max_players: usize,
    pub game_started: bool,
    player_action: HashMap<PlayerId, Action>,
    champions: HashMap<PlayerId, Champion>,
    towers: HashMap<TowerId, Tower>,
    red_base: Base,
    blue_base: Base,
    minion_manager: MinionManager,
    monster_manager: MonsterManager,
    projectile_manager: ProjectileManager,
    animations: Vec<Box<dyn AnimationTrait>>,
    pub client_channel: HashMap<PlayerId, mpsc::Sender<ClientMessage>>,
    board: Board,
    pub tick: u64,
    dead_minion_positions: Vec<(u16, u16, Team)>,
    config: GameConfig,
    game_start_time: Option<Instant>,
    initial_monsters_spawned: bool,
}

impl GameManager {
    pub fn new(config: GameConfig) -> Self {
        println!("Initializing GameManager...");
        let file_path = "game/assets/map.json";
        let mut board = match Board::from_json(file_path) {
            Ok(board) => board,
            Err(e) => {
                eprintln!("Failed to initialize the board from {}: {}", file_path, e);
                std::process::exit(1);
            }
        };
        let mut towers: HashMap<TowerId, Tower> = HashMap::new();
        // Tower placement
        {
            let placement = vec![
                (196, 150),
                (39, 7),
                (115, 82),
                (191, 79),
                (120, 8),
                (148, 67),
            ];
            // Bottom t1
            placement.into_iter().for_each(|place| {
                let id = generate_tower_id().unwrap();
                let tower_blue = Tower::new(id, Team::Blue, place.0, place.1, config.tower.clone());
                tower_blue.place_tower(&mut board);
                let id = generate_tower_id().unwrap();
                let tower_red = Tower::new(id, Team::Red, place.1, place.0, config.tower.clone());
                tower_red.place_tower(&mut board);
                towers.insert(tower_blue.tower_id, tower_blue);
                towers.insert(tower_red.tower_id, tower_red);
            });
        }

        let red_base = Base::new(Team::Red, (190, 10), config.base.clone());
        let blue_base = Base::new(Team::Blue, (10, 190), config.base.clone());

        for i in 0..3 {
            for j in 0..3 {
                board.place_cell(
                    CellContent::Base(Team::Blue),
                    (red_base.position.0 + i) as usize,
                    (red_base.position.1 + j) as usize,
                );
                board.place_cell(
                    CellContent::Base(Team::Red),
                    (blue_base.position.0 + i) as usize,
                    (blue_base.position.1 + j) as usize,
                );
            }
        }

        let minion_manager = MinionManager::new(config.minion.clone());
        let monster_manager = MonsterManager::new(config.neutral_monsters.clone());
        let projectile_manager = ProjectileManager::new();

        GameManager {
            players_count: 0,
            max_players: 1,
            game_started: false,
            config,
            player_action: HashMap::new(),
            champions: HashMap::new(),
            towers,
            red_base,
            blue_base,
            minion_manager,
            monster_manager,
            projectile_manager,
            animations: Vec::new(),
            client_channel: HashMap::new(),
            board,
            tick: 20,
            dead_minion_positions: Vec::new(),
            game_start_time: None,
            initial_monsters_spawned: false,
        }
    }

    pub fn print_game_state(&self) {
        println!(
            "Player connected: {}/{}",
            self.players_count, self.max_players
        );
        if self.player_action.is_empty() {
            println!("No action received");
        } else {
            for (player_id, action) in &self.player_action {
                println!("Player: {} / Action: {:?}", player_id, action);
            }
        }
        println!("Board size: {}.{}", self.board.rows, self.board.cols);
    }

    pub fn clear_action(&mut self) {
        self.player_action.clear();
    }

    pub fn add_player(&mut self, spell1_id: u8, spell2_id: u8) -> Option<PlayerId> {
        if self.players_count < self.max_players {
            self.players_count += 1;
            let player_id = self.players_count;
            // Assign Champion to player, and place it on the board
            {
                // We get the choosen spell
                let mut selected_spell: HashMap<u8, Box<dyn Spell>> = HashMap::new();
                if let Some(spell_stats) = self.config.spells.get(&spell1_id) {
                    selected_spell.insert(
                        spell1_id,
                        spell::create_spell_from_id(spell1_id, spell_stats.clone()),
                    );
                }
                if let Some(spell_stats) = self.config.spells.get(&spell2_id) {
                    selected_spell.insert(
                        spell2_id,
                        spell::create_spell_from_id(spell2_id, spell_stats.clone()),
                    );
                }
                let row = 199;
                let col = 0;
                let champion = Champion::new(
                    player_id,
                    Team::Blue,
                    row,
                    col,
                    self.config.champion.clone(),
                    selected_spell,
                );
                self.champions.insert(player_id, champion);
                self.board.place_cell(
                    cell::CellContent::Champion(player_id, Team::Blue),
                    row as usize,
                    col as usize,
                );
            }

            // We check if we can start the game and send a Start to each player
            if self.players_count == self.max_players {
                self.game_started = true;
                self.game_start_time = Some(Instant::now());
                self.minion_manager.wave_creation_time = Instant::now() + Duration::from_secs(30);
            }
            Some(player_id)
        } else {
            None
        }
    }

    pub fn remove_player(&mut self, player_id: &PlayerId) {
        if self.players_count > 0 {
            self.players_count -= 1;
            self.player_action.remove(&player_id);
            self.client_channel.remove(&player_id);
            println!(
                "Player {} disconnected. Total player now: {}/{}",
                player_id, self.players_count, self.max_players
            );
            if self.game_started && self.players_count < self.max_players {
                self.game_started = false;
            }
        } else {
            println!("Warning: Tried to remove player, but player count already at 0.");
        }
    }

    pub fn store_player_action(&mut self, player_id: PlayerId, action_value: u8) {
        let action = match action_value {
            1 => Action::MoveUp,
            2 => Action::MoveDown,
            3 => Action::MoveLeft,
            4 => Action::MoveRight,
            5 => Action::Action1,
            6 => Action::Action2,
            _other => Action::InvalidAction,
        };
        self.player_action.insert(player_id, action);
    }

    pub async fn send_to_player(&self, player_id: PlayerId, message: ClientMessage) {
        println!("Send_to_player message lenght: {}", message.len());
        if let Some(sender) = self.client_channel.get(&player_id) {
            let sender_clone = sender.clone();
            // We use spawn to send without blocking the game manager lock
            tokio::spawn(async move {
                if let Err(e) = sender_clone.send(message).await {
                    eprintln!("Error sending message to player {}: {}", player_id, e);
                }
            });
        } else {
            eprintln!(
                "Attempted to send message to disconnected or non-existent player {}",
                player_id
            );
        }
    }

    pub fn game_tick(&mut self) -> HashMap<PlayerId, ClientMessage> {
        if let Some(start_time) = self.game_start_time {
            if !self.initial_monsters_spawned && start_time.elapsed() >= Duration::from_secs(5) {
                self.monster_manager.spawn_initial_monsters(&mut self.board);
                self.initial_monsters_spawned = true;
            }
        }

        self.tick = self.tick.saturating_add(1);
        println!("---- Game Tick -----");
        self.print_game_state();

        let mut updates = HashMap::new();
        let mut new_animations: Vec<Box<dyn AnimationTrait>> = Vec::new();
        let mut animation_commands_executable: Vec<AnimationCommand> = Vec::new();
        let mut pending_effects: Vec<(Option<PlayerId>, Target, Vec<GameplayEffect>)> = Vec::new();
        let mut xp_rewards: Vec<(PlayerId, u8)> = Vec::new();

        // --- Game Logic ---
        // Buff checks on all entities
        // Champions
        for (_, champ) in self.champions.iter_mut() {
            let current_buff = take(&mut champ.active_buffs);
            let mut kept_buff: HashMap<String, Box<dyn Buff>> = HashMap::new();
            for (id, mut buff) in current_buff.into_iter() {
                if buff.on_tick(champ) {
                    buff.on_remove(champ);
                } else {
                    kept_buff.insert(id, buff);
                }
            }
            champ.active_buffs = kept_buff;
        }

        // Minions
        for (_, minion) in self.minion_manager.minions.iter_mut() {
            let current_buff = take(&mut minion.active_buffs);
            let mut kept_buff: HashMap<String, Box<dyn Buff>> = HashMap::new();
            for (id, mut buff) in current_buff.into_iter() {
                if buff.on_tick(minion) {
                    buff.on_remove(minion);
                } else {
                    kept_buff.insert(id, buff);
                }
            }
            minion.active_buffs = kept_buff;
        }

        // --- Turn ---
        // Player turn
        for (player_id, champ) in &mut self.champions {
            // 0. Check death and replace
            // BUG: Champ dead can still move but is replace each tick
            if champ.is_dead() {
                champ.put_at_max_health();
                champ.place_at_base(&mut self.board);
                continue;
            }
            // 1. Iterate through player action
            if let Some(action) = self.player_action.get(&player_id) {
                if let Err(e) =
                    champ.take_action(action, &mut self.board, &mut self.projectile_manager)
                {
                    println!("Error on player action: {}", e);
                }
            }

            // 2. auto_attack
            if let Some(enemy) = champ.get_potential_target(&self.board) {
                match &enemy.content {
                    Some(content) => {
                        println!("Got content: {:?}", content);
                        match content {
                            CellContent::Tower(id, _) => {
                                if let Some(attack) = champ.can_attack() {
                                    match attack {
                                        AttackAction::Melee { damage, animation } => {
                                            new_animations.push(animation);
                                            pending_effects.push((
                                                Some(*player_id),
                                                Target::Tower(*id),
                                                vec![GameplayEffect::Damage(damage)],
                                            ))
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            CellContent::Monster(id) => {
                                if let Some(attack) = champ.can_attack() {
                                    match attack {
                                        AttackAction::Melee { damage, animation } => {
                                            new_animations.push(animation);
                                            pending_effects.push((
                                                Some(*player_id),
                                                Target::Monster(*id),
                                                vec![GameplayEffect::Damage(damage)],
                                            ))
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            CellContent::Minion(id, _) => {
                                if let Some(attack) = champ.can_attack() {
                                    match attack {
                                        AttackAction::Melee { damage, animation } => {
                                            new_animations.push(animation);
                                            pending_effects.push((
                                                Some(*player_id),
                                                Target::Minion(*id),
                                                vec![GameplayEffect::Damage(damage)],
                                            ))
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            CellContent::Champion(id, _) => {
                                if let Some(attack) = champ.can_attack() {
                                    match attack {
                                        AttackAction::Melee { damage, animation } => {
                                            new_animations.push(animation);
                                            pending_effects.push((
                                                Some(*player_id),
                                                Target::Champion(*id),
                                                vec![GameplayEffect::Damage(damage)],
                                            ))
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            CellContent::Base(team) => {
                                if let Some(attack) = champ.can_attack() {
                                    match attack {
                                        AttackAction::Melee { damage, animation } => {
                                            new_animations.push(animation);
                                            pending_effects.push((
                                                Some(*player_id),
                                                Target::Base(*team),
                                                vec![GameplayEffect::Damage(damage)],
                                            ))
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            _ => break,
                        }
                    }
                    None => break,
                }
            }
        }

        // Minion mouvement turn
        self.minion_manager
            .manage_minions_mouvements(&mut self.board);
        self.minion_manager.make_wave(&mut self.board);
        println!(
            "Minions: {} | Minions per wave {} | Tick: {}",
            self.minion_manager.minions.len(),
            self.minion_manager.minions_this_wave,
            self.tick,
        );

        // Adding minion damages dealt
        self.minion_manager.manage_minions_attack(
            &mut self.board,
            &mut new_animations,
            &mut pending_effects,
        );

        // Monster turn
        let (monster_effects, monster_animations) = self.monster_manager.update(&mut self.board, &self.champions);
        pending_effects.extend(monster_effects.into_iter().map(|(target, effects)| (None, target, effects)));
        new_animations.extend(monster_animations);

        // Tower turn
        // 1. Scan range
        // 2. attack closest enemy
        self.tower_turn();

        let (projectile_effects, projectile_commands) =
            self.projectile_manager.update_and_check_collisions(
                &self.board,
                &self.champions,
                &self.minion_manager.minions,
                &self.towers,
                &self.monster_manager.active_monsters,
            );
        pending_effects.extend(projectile_effects.into_iter().map(|(owner, target, effects)| (Some(owner), target, effects)));
        animation_commands_executable.extend(projectile_commands);

        // 3. Apply dealt damages
        pending_effects
            .into_iter()
            .for_each(|(attacker_id, target, effect)| match target {
                Target::Tower(id) => {
                    if let Some(tower) = self.towers.get_mut(&id) {
                        tower.take_effect(effect);
                        if tower.is_destroyed() {
                            tower.destroy_tower(&mut self.board);
                            self.towers.remove(&id);
                        }
                    }
                }
                Target::Minion(id) => {
                    if let Some(minion) = self.minion_manager.minions.get_mut(&id) {
                        minion.take_effect(effect);
                        self.handle_minion_death(&id);
                    }
                }
                Target::Champion(id) => {
                    if let Some(champ) = self.champions.get_mut(&id) {
                        champ.take_effect(effect);
                    }
                }
                Target::Base(team) => match team {
                    Team::Red => self.red_base.take_effect(effect),
                    Team::Blue => self.blue_base.take_effect(effect),
                },
                Target::Monster(id) => {
                    if let Some(..) = self.monster_manager.active_monsters.get_mut(&id) {
                        if let Some(attacker) = attacker_id {
                            if let Some(reward) = self.monster_manager.apply_effects_to_monster(&id, effect, attacker) {
                                xp_rewards.push(reward);
                            }

                        }
                    }
                }
            });

        // Distribute XP from dead monster
        for (player_id, xp_reward) in xp_rewards.into_iter() {
            if let Some(champion) = self.champions.get_mut(&player_id) {
                champion.add_xp(xp_reward as u32);
            }
        }
        // Distribute XP from dead minions
        for (minion_row, minion_col, minion_team) in self.dead_minion_positions.drain(..) {
            let mut champions_in_range = Vec::new();
            for (_, champion) in self.champions.iter_mut() {
                // Check if champion is in 5x5 range and is on the opposing team
                if champion.team_id != minion_team
                    && (champion.row as i32 - minion_row as i32).abs() <= 2
                    && (champion.col as i32 - minion_col as i32).abs() <= 2
                {
                    champions_in_range.push(champion);
                }
            }

            if !champions_in_range.is_empty() {
                let xp_per_champion = 5 / champions_in_range.len() as u32;
                for champion in champions_in_range {
                    champion.add_xp(xp_per_champion);
                }
            }
        }

        // Render animation
        let mut kept_animations: Vec<Box<dyn AnimationTrait>> = Vec::new();

        // 1. clear past frame animation
        for anim in &self.animations {
            if let Some((row, col)) = anim.get_last_drawn_pos() {
                animation_commands_executable.push(AnimationCommand::Clear { row, col })
            }
        }
        // 2. Process next frame animations
        for mut anim in self.animations.drain(..) {
            let owner_pos = if let Some(champ) = self.champions.get(&anim.get_owner_id()) {
                Some((champ.row, champ.col))
            } else if let Some(tower) = self.towers.get(&anim.get_owner_id()) {
                Some((tower.row, tower.col))
            } else if let Some(minion) = self.minion_manager.minions.get(&anim.get_owner_id()) {
                Some((minion.row, minion.col))
            } else {
                None // Owner might have been removed
            };

            if let Some((owner_row, owner_col)) = owner_pos {
                let command = anim.next_frame(owner_row, owner_col);
                match command {
                    AnimationCommand::Done => {}
                    AnimationCommand::Draw { .. } => {
                        animation_commands_executable.push(command);
                        kept_animations.push(anim);
                    }
                    AnimationCommand::Clear { .. } => {
                        // This command should be handle before
                    }
                }
            } else {
                // Owner is gone, animation should finish and clear in its last frame
            }
        }
        kept_animations.extend(new_animations);
        self.animations = kept_animations;

        // 3. Execute animation command
        for command in animation_commands_executable {
            match command {
                AnimationCommand::Draw {
                    row,
                    col,
                    animation_type,
                } => {
                    // Add bounds check
                    if row < self.board.rows as u16 && col < self.board.cols as u16 {
                        self.board
                            .place_animation(animation_type, row as usize, col as usize);
                    } else {
                        eprintln!("Animation draw position ({}, {}) out of bounds!", row, col);
                    }
                }
                AnimationCommand::Clear { row, col } => {
                    if row < self.board.rows as u16 && col < self.board.cols as u16 {
                        self.board.clean_animation(row as usize, col as usize);
                    } else {
                        eprintln!("Animation clear position ({}, {}) out of bounds!", row, col);
                    }
                }
                AnimationCommand::Done => {
                    // This command should be handled in the loop above, not executed on the board
                }
            }
        }

        // Check for win condition
        if self.red_base.stats.health <= 0 {
            println!("Sending EndGamePacket: Red base destroyed, Blue team wins!");
            let packet = crate::packet::end_game_packet::EndGamePacket::new(Team::Red);
            println!("EndGamePacket: {:?}", packet);
            let serialized_packet = packet.serialize();
            for (player_id, _) in &self.client_channel {
                self.send_to_player(*player_id, BytesMut::from(&serialized_packet[..]));
            }
            std::process::exit(0);
        } else if self.blue_base.stats.health <= 0 {
            println!("Sending EndGamePacket: Blue base destroyed, Red team wins!");
            let packet = crate::packet::end_game_packet::EndGamePacket::new(Team::Blue);
            println!("EndGamePacket: {:?}", packet);
            let serialized_packet = packet.serialize();
            for (player_id, _) in &self.client_channel {
                self.send_to_player(*player_id, BytesMut::from(&serialized_packet[..]));
            }
            std::process::exit(0);
        }

        // --- Send per player there board view ---
        for (player_id, champion) in &self.champions {
            // 1. Get player-specific board view
            let board_rle_vec =
                self.board
                    .run_length_encode(champion.row, champion.col, &self.minion_manager);
            // 2. Create the board packet
            let health = champion.get_health();
            let xp_needed = champion.xp_for_next_level().unwrap_or(0); // Get XP needed, 0 if max level
            let board_packet = BoardPacket::new(
                health.0,
                health.1,
                champion.stats.mana,
                champion.stats.max_mana,
                champion.level,
                champion.xp,
                xp_needed,
                board_rle_vec,
            );
            let serialized_packet = board_packet.serialize();
            // 3. Store the serialized packet to be sent later
            updates.insert(*player_id, serialized_packet);
        }
        println!("--------------------");
        updates
    }

    fn tower_turn(&mut self) {
        let mut projectiles_to_create = Vec::new();

        for (_, tower) in self.towers.iter_mut() {
            if let Some(enemy_cell) = tower.get_potential_target(&self.board) {
                if let Some(enemy_content) = &enemy_cell.content {
                    let target = match enemy_content {
                        CellContent::Champion(id, _) => Some(Target::Champion(*id)),
                        CellContent::Minion(id, _) => Some(Target::Minion(*id)),
                        _ => None, // Towers can't target other entities
                    };

                    if let Some(target) = target {
                        if let Some(attack_action) = tower.can_attack() {
                            if let AttackAction::Projectile {
                                damage,
                                speed,
                                visual,
                            } = attack_action
                            {
                                projectiles_to_create.push((
                                    tower.tower_id,
                                    target,
                                    damage,
                                    speed,
                                    visual,
                                ));
                            }
                        }
                    }
                }
            }
        }
        // We create the projectiles
        for (tower_id, target, damage, speed, visual) in projectiles_to_create {
            if let Some(tower) = self.towers.get(&tower_id) {
                self.projectile_manager.create_homing_projectile(
                    tower.tower_id as u64,
                    tower.team_id,
                    target,
                    (tower.row, tower.col),
                    speed,
                    vec![GameplayEffect::Damage(damage)],
                    visual,
                );
            }
        }
    }

    fn handle_minion_death(&mut self, id: &MinionId) {
        if let Some(minion) = self.minion_manager.minions.get(id) {
            if minion.is_dead() {
                self.dead_minion_positions
                    .push((minion.row, minion.col, minion.team_id));
                self.board
                    .clear_cell(minion.row as usize, minion.col as usize);
                self.minion_manager.minions.remove(&id);
            }
        }
    }
}
