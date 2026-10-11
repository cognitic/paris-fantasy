use macroquad::{
    prelude::*,
    texture::{FilterMode, Image, RenderTarget},
};

const VIEW_W: f32 = 320.0;
const VIEW_H: f32 = 180.0;
const MAP_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/default_world.png"));

const INK: Color = Color::from_rgba(27, 29, 38, 255);
const PAPER: Color = Color::from_rgba(229, 214, 170, 255);
const GOLD: Color = Color::from_rgba(238, 194, 92, 255);
const RED: Color = Color::from_rgba(193, 76, 68, 255);
const GREEN: Color = Color::from_rgba(92, 176, 104, 255);

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    World,
    Battle,
    Town,
}

struct WorldMap {
    texture: Texture2D,
    pixels: Vec<[u8; 4]>,
    width: usize,
    height: usize,
    town: (f32, f32),
    shards: [(f32, f32); 3],
}

impl WorldMap {
    fn from_image(image: Image) -> Self {
        let pixels = image.get_image_data().to_vec();
        let width = image.width as usize;
        let height = image.height as usize;
        let texture = Texture2D::from_image(&image);
        texture.set_filter(FilterMode::Nearest);
        let mut map = Self {
            texture,
            pixels,
            width,
            height,
            town: (0.0, 0.0),
            shards: [(0.0, 0.0); 3],
        };
        map.town = map.nearest_walkable(width as f32 * 0.58, height as f32 * 0.46);
        map.shards = [
            map.nearest_walkable(width as f32 * 0.79, height as f32 * 0.30),
            map.nearest_walkable(width as f32 * 0.14, height as f32 * 0.48),
            map.nearest_walkable(width as f32 * 0.49, height as f32 * 0.23),
        ];
        map
    }

    fn pixel(&self, x: i32, y: i32) -> Color {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return Color::from_rgba(29, 55, 91, 255);
        }
        let pixel = self.pixels[y as usize * self.width + x as usize];
        Color::from_rgba(pixel[0], pixel[1], pixel[2], pixel[3])
    }

    fn blocked(&self, x: i32, y: i32) -> bool {
        let color = self.pixel(x, y);
        let (r, g, b) = (color.r * 255.0, color.g * 255.0, color.b * 255.0);
        let water = b > r + 16.0 && b > g + 8.0;
        let mountain = r > 85.0 && (r - g).abs() < 23.0 && (g - b).abs() < 23.0;
        water || mountain
    }

    fn terrain(&self, x: i32, y: i32) -> &'static str {
        let color = self.pixel(x, y);
        if self.blocked(x, y) {
            "SEA OR CLIFF"
        } else if color.g > color.r * 1.22 {
            "ELDERWOOD"
        } else if color.r > 0.62 && color.g > 0.48 {
            "THE OLD ROAD"
        } else {
            "MOONSHADE FIELD"
        }
    }

    fn nearest_walkable(&self, x: f32, y: f32) -> (f32, f32) {
        let center_x = x.round() as i32;
        let center_y = y.round() as i32;
        for radius in 0..self.width.max(self.height) as i32 {
            for offset in -radius..=radius {
                for (check_x, check_y) in [
                    (center_x + offset, center_y - radius),
                    (center_x + offset, center_y + radius),
                    (center_x - radius, center_y + offset),
                    (center_x + radius, center_y + offset),
                ] {
                    if can_stand(self, check_x as f32, check_y as f32) {
                        return (check_x as f32, check_y as f32);
                    }
                }
            }
        }
        (x, y)
    }
}

struct Hero {
    x: f32,
    y: f32,
    hp: i32,
    max_hp: i32,
    mp: i32,
    max_mp: i32,
    potions: i32,
    gold: i32,
    xp: i32,
    shards: u8,
    found_shards: [bool; 3],
    steps: u32,
    facing: (i32, i32),
}

struct Battle {
    hp: i32,
    max_hp: i32,
    name: &'static str,
    message: String,
    menu: usize,
    phase: u8,
    defeated: bool,
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Paris Fantasy".to_owned(),
        window_width: 1920,
        window_height: 1080,
        high_dpi: false,
        window_resizable: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let image = match load_file("assets/world.png").await {
        Ok(bytes) => Image::from_file_with_format(&bytes, None)
            .unwrap_or_else(|_| Image::from_file_with_format(MAP_BYTES, None).unwrap()),
        Err(_) => Image::from_file_with_format(MAP_BYTES, None).unwrap(),
    };
    let world = WorldMap::from_image(image);
    let mut target_size = (VIEW_W as u32, VIEW_H as u32);
    let mut target = render_target(target_size.0, target_size.1);
    target.texture.set_filter(FilterMode::Nearest);
    let mut font = get_default_font();
    font.set_filter(FilterMode::Nearest);
    let spawn = world.nearest_walkable(world.width as f32 * 0.34, world.height as f32 * 0.68);
    let mut hero = Hero {
        x: spawn.0,
        y: spawn.1,
        hp: 32,
        max_hp: 32,
        mp: 9,
        max_mp: 9,
        potions: 3,
        gold: 18,
        xp: 0,
        shards: 0,
        found_shards: [false; 3],
        steps: 0,
        facing: (0, 1),
    };
    let mut screen = Screen::World;
    let mut battle: Option<Battle> = None;
    let mut encounter_meter = 0.0;
    let mut rng = 0x51A7_2026_u32;
    let mut toast = String::from("THE CROWN HAS FALLEN. FIND THE THREE STAR SHARDS.");
    let mut toast_timer = 5.0;

    loop {
        let fit_scale = (screen_width() / VIEW_W).min(screen_height() / VIEW_H);
        let scale = fit_scale.max(0.01);
        let output_size = (
            (VIEW_W * scale).round().max(1.0) as u32,
            (VIEW_H * scale).round().max(1.0) as u32,
        );
        if output_size != target_size {
            target = render_target(output_size.0, output_size.1);
            target.texture.set_filter(FilterMode::Nearest);
            target_size = output_size;
        }
        let origin = vec2(
            ((screen_width() - target_size.0 as f32) * 0.5).round(),
            ((screen_height() - target_size.1 as f32) * 0.5).round(),
        );
        let dt = get_frame_time().min(0.05);
        if toast_timer > 0.0 {
            toast_timer -= dt;
        }

        match screen {
            Screen::World => {
                let mut dx: f32 = 0.0;
                let mut dy: f32 = 0.0;
                if is_key_down(KeyCode::Left) || is_key_down(KeyCode::A) {
                    dx -= 1.0;
                }
                if is_key_down(KeyCode::Right) || is_key_down(KeyCode::D) {
                    dx += 1.0;
                }
                if is_key_down(KeyCode::Up) || is_key_down(KeyCode::W) {
                    dy -= 1.0;
                }
                if is_key_down(KeyCode::Down) || is_key_down(KeyCode::S) {
                    dy += 1.0;
                }
                if dx != 0.0 || dy != 0.0 {
                    if dx != 0.0 && dy != 0.0 {
                        dx *= 0.7071;
                        dy *= 0.7071;
                    }
                    hero.facing = (dx.signum() as i32, dy.signum() as i32);
                    let speed = if world.terrain(hero.x as i32, hero.y as i32) == "ELDERWOOD" {
                        75.0
                    } else {
                        117.0
                    };
                    let next_x = hero.x + dx * speed * dt;
                    let next_y = hero.y + dy * speed * dt;
                    // TODO: Re-enable pixel obstacle checks when map collision is ready.
                    // if can_stand(&world, next_x, hero.y) {
                    hero.x = next_x;
                    // }
                    // if can_stand(&world, hero.x, next_y) {
                    hero.y = next_y;
                    // }
                    hero.steps = hero.steps.wrapping_add(1);
                    encounter_meter += dt * (dx.abs() + dy.abs());
                    if encounter_meter > 3.7 {
                        encounter_meter = 0.0;
                        rng = next_random(rng);
                        if rng % 100 < 18 {
                            start_battle(&mut screen, &mut battle, rng);
                        }
                    }
                }
                for (index, (shard_x, shard_y)) in world.shards.iter().enumerate() {
                    let dx = hero.x - shard_x;
                    let dy = hero.y - shard_y;
                    if !hero.found_shards[index] && dx * dx + dy * dy < 9.0 * 9.0 {
                        hero.found_shards[index] = true;
                        hero.shards += 1;
                        toast = if hero.shards == 3 {
                            String::from("THE THREE STAR SHARDS ARE YOURS. THE CROWN AWAKENS.")
                        } else {
                            format!("STAR SHARD FOUND.  {} / 3", hero.shards)
                        };
                        toast_timer = 4.0;
                    }
                }
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                    if near_town(&hero, &world) {
                        screen = Screen::Town;
                    } else {
                        toast = format!(
                            "{}  |  THE ROAD REMEMBERS YOUR FOOTSTEPS.",
                            world.terrain(hero.x as i32, hero.y as i32)
                        );
                        toast_timer = 2.5;
                    }
                }
            }
            Screen::Battle => {
                if let Some(current) = battle.as_mut() {
                    if current.phase == 0 {
                        if is_key_pressed(KeyCode::Up) {
                            current.menu = (current.menu + 3) % 4;
                        }
                        if is_key_pressed(KeyCode::Down) {
                            current.menu = (current.menu + 1) % 4;
                        }
                        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                            battle_action(current, &mut hero);
                        }
                    } else if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                        if current.hp <= 0 {
                            if current.defeated {
                                let spawn = world.nearest_walkable(
                                    world.width as f32 * 0.34,
                                    world.height as f32 * 0.68,
                                );
                                hero.x = spawn.0;
                                hero.y = spawn.1;
                                toast = String::from(
                                    "THE SHRINE SENDS YOU BACK. HALF YOUR GOLD IS LOST.",
                                );
                            } else {
                                hero.xp += 12;
                                hero.gold += 9;
                                hero.hp = (hero.hp + 7).min(hero.max_hp);
                                toast = format!("VICTORY!  12 XP  /  9 G  /  {} XP TOTAL", hero.xp);
                            }
                            toast_timer = 4.0;
                            battle = None;
                            screen = Screen::World;
                        } else if current.phase == 2 {
                            toast = String::from("YOU ESCAPED INTO THE NIGHT.");
                            toast_timer = 2.5;
                            battle = None;
                            screen = Screen::World;
                        } else {
                            current.phase = 0;
                            current.message.clear();
                        }
                    }
                }
            }
            Screen::Town => {
                if is_key_pressed(KeyCode::Escape) {
                    toast = String::from("YOU STEP OUT INTO THE NIGHT.");
                    toast_timer = 2.0;
                    screen = Screen::World;
                } else if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                    hero.hp = hero.max_hp;
                    hero.mp = hero.max_mp;
                    hero.potions = (hero.potions + 1).min(9);
                    hero.gold = hero.gold.saturating_sub(4);
                    toast = String::from("RESTED AND READY. ONE POTION ADDED. 4 G SPENT.");
                    toast_timer = 3.0;
                    screen = Screen::World;
                }
            }
        }

        draw_world(
            &world,
            &hero,
            screen,
            battle.as_ref(),
            &toast,
            toast_timer,
            &target,
            origin,
            target_size,
        );
        next_frame().await;
    }
}

fn can_stand(world: &WorldMap, x: f32, y: f32) -> bool {
    let x = x.round() as i32;
    let y = y.round() as i32;
    [(-2, -3), (2, -3), (-2, 3), (2, 3), (0, 0)]
        .into_iter()
        .all(|(ox, oy)| !world.blocked(x + ox, y + oy))
}

fn camera_center(focus: f32, world_size: f32, view_size: f32) -> f32 {
    if world_size <= view_size {
        (world_size * 0.5).round()
    } else {
        focus
            .round()
            .clamp(view_size * 0.5, world_size - view_size * 0.5)
    }
}

fn near_town(hero: &Hero, world: &WorldMap) -> bool {
    let dx = hero.x - world.town.0;
    let dy = hero.y - world.town.1;
    dx * dx + dy * dy < 13.0 * 13.0
}

fn next_random(value: u32) -> u32 {
    let mut x = value;
    x ^= x << 13;
    x ^= x >> 17;
    x ^ (x << 5)
}

fn start_battle(screen: &mut Screen, battle: &mut Option<Battle>, seed: u32) {
    let (name, hp) = if seed % 3 == 0 {
        ("MARSH WRAITH", 18)
    } else {
        ("CROWN BANDIT", 15)
    };
    *battle = Some(Battle {
        hp,
        max_hp: hp,
        name,
        message: String::from("A FOE BLOCKS THE ROAD. PRESS ENTER."),
        menu: 0,
        phase: 1,
        defeated: false,
    });
    *screen = Screen::Battle;
}

fn battle_action(battle: &mut Battle, hero: &mut Hero) {
    battle.message = match battle.menu {
        0 => {
            battle.hp -= 7;
            format!("YOU STRIKE TRUE! {} HP LEFT.", battle.hp.max(0))
        }
        1 if hero.mp >= 3 => {
            hero.mp -= 3;
            battle.hp -= 12;
            format!("STARFIRE! {} HP LEFT.", battle.hp.max(0))
        }
        1 => String::from("NOT ENOUGH MP."),
        2 if hero.potions > 0 => {
            hero.potions -= 1;
            hero.hp = (hero.hp + 16).min(hero.max_hp);
            String::from("A POTION RESTORES 16 HP.")
        }
        2 => String::from("YOUR BAG IS EMPTY."),
        _ => {
            battle.phase = 2;
            String::from("YOU SLIP AWAY... PRESS ENTER.")
        }
    };
    if battle.hp > 0 && battle.phase != 2 {
        let damage = if battle.name == "MARSH WRAITH" { 6 } else { 4 };
        hero.hp = (hero.hp - damage).max(0);
        battle
            .message
            .push_str(&format!("  {} DEALS {} DAMAGE.", battle.name, damage));
        battle.phase = 1;
    } else if battle.phase != 2 {
        battle.phase = 1;
    }
    if hero.hp == 0 {
        hero.hp = hero.max_hp / 2;
        hero.gold /= 2;
        battle.hp = 0;
        battle.defeated = true;
        battle.message = String::from("THE SHRINE SENDS YOU BACK. HALF YOUR GOLD IS LOST.");
    }
    if battle.hp <= 0 && hero.hp > 0 {
        battle.phase = 3;
        battle.message.push_str("  PRESS ENTER.");
    }
}

fn draw_world(
    world: &WorldMap,
    hero: &Hero,
    screen: Screen,
    battle: Option<&Battle>,
    toast: &str,
    toast_timer: f32,
    target: &RenderTarget,
    origin: Vec2,
    target_size: (u32, u32),
) {
    let render_scale = target_size.0 as f32 / VIEW_W;
    let world_view_width = target_size.0 as f32;
    let hud_top = (15.0 * render_scale).round() as i32;
    let hud_bottom = (40.0 * render_scale).round() as i32;
    let world_view_height = (target_size.1 as i32 - hud_top - hud_bottom).max(1);
    let world_view_height = world_view_height as f32;
    clear_background(Color::from_rgba(12, 17, 30, 255));
    let camera_x = camera_center(hero.x, world.width as f32, world_view_width);
    let camera_y = camera_center(hero.y, world.height as f32, world_view_height);
    let world_camera = Camera2D {
        target: vec2(camera_x, camera_y),
        zoom: vec2(2.0 / world_view_width, -2.0 / world_view_height),
        render_target: Some(target.clone()),
        viewport: Some((
            0,
            hud_bottom,
            target_size.0 as i32,
            world_view_height as i32,
        )),
        ..Default::default()
    };
    set_camera(&world_camera);
    clear_background(Color::from_rgba(29, 55, 91, 255));
    draw_texture(&world.texture, 0.0, 0.0, WHITE);

    draw_town(world.town.0, world.town.1);
    for (index, (shard_x, shard_y)) in world.shards.iter().enumerate() {
        if !hero.found_shards[index] {
            draw_crystal(*shard_x, *shard_y);
        }
    }
    draw_hero(hero.x.round(), hero.y.round(), hero.facing);

    let ui_camera = Camera2D {
        target: vec2(VIEW_W * 0.5, VIEW_H * 0.5),
        zoom: vec2(2.0 / VIEW_W, -2.0 / VIEW_H),
        render_target: Some(target.clone()),
        ..Default::default()
    };
    set_camera(&ui_camera);
    draw_rectangle(0.0, 0.0, VIEW_W, 15.0, Color::from_rgba(22, 27, 38, 230));
    text("Paris Fantasy", 6.0, 10.0, 8.0, GOLD, render_scale);
    text(
        &format!("SHARDS {} / 3", hero.shards),
        153.0,
        10.0,
        7.0,
        GREEN,
        render_scale,
    );
    text(
        &format!("G {:03}  XP {:03}", hero.gold, hero.xp),
        235.0,
        10.0,
        6.0,
        PAPER,
        render_scale,
    );

    draw_rectangle(
        0.0,
        VIEW_H - 40.0,
        VIEW_W,
        40.0,
        Color::from_rgba(22, 27, 38, 245),
    );
    draw_rectangle(0.0, VIEW_H - 40.0, VIEW_W, 1.0, GOLD);
    text(
        &format!("HP {:02}/{:02}", hero.hp, hero.max_hp),
        7.0,
        VIEW_H - 28.0,
        7.0,
        PAPER,
        render_scale,
    );
    bar(57.0, VIEW_H - 34.0, 49.0, 5.0, hero.hp, hero.max_hp, RED);
    text(
        &format!("MP {:02}/{:02}", hero.mp, hero.max_mp),
        7.0,
        VIEW_H - 14.0,
        7.0,
        PAPER,
        render_scale,
    );
    bar(
        57.0,
        VIEW_H - 20.0,
        49.0,
        5.0,
        hero.mp,
        hero.max_mp,
        Color::from_rgba(80, 151, 222, 255),
    );
    let prompt = if screen == Screen::World && near_town(hero, world) {
        "ENTER: ALDERFALL INN"
    } else {
        "ENTER: LOOK AROUND"
    };
    text(
        &format!("POTIONS {}", hero.potions),
        119.0,
        VIEW_H - 27.0,
        7.0,
        PAPER,
        render_scale,
    );
    text(
        "ARROWS MOVE",
        119.0,
        VIEW_H - 13.0,
        6.0,
        Color::from_rgba(159, 173, 182, 255),
        render_scale,
    );
    text(prompt, 213.0, VIEW_H - 13.0, 6.0, GOLD, render_scale);
    if toast_timer > 0.0 && screen == Screen::World {
        draw_rectangle(
            20.0,
            20.0,
            VIEW_W - 40.0,
            17.0,
            Color::from_rgba(22, 27, 38, 225),
        );
        text(toast, 25.0, 31.0, 6.0, PAPER, render_scale);
    }
    if screen == Screen::Battle {
        draw_battle(battle, hero, render_scale);
    }
    if screen == Screen::Town {
        draw_inn(hero, render_scale);
    }

    set_default_camera();
    draw_texture_ex(
        &target.texture,
        origin.x,
        origin.y,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(target_size.0 as f32, target_size.1 as f32)),
            flip_y: true,
            ..Default::default()
        },
    );
}

fn draw_town(x: f32, y: f32) {
    draw_rectangle(
        x - 6.0,
        y - 5.0,
        13.0,
        10.0,
        Color::from_rgba(58, 48, 47, 255),
    );
    draw_rectangle(
        x - 7.0,
        y - 8.0,
        15.0,
        5.0,
        Color::from_rgba(151, 64, 57, 255),
    );
    draw_rectangle(
        x - 2.0,
        y + 1.0,
        4.0,
        4.0,
        Color::from_rgba(232, 191, 111, 255),
    );
    draw_rectangle(x + 7.0, y - 2.0, 2.0, 2.0, GOLD);
}

fn draw_crystal(x: f32, y: f32) {
    draw_rectangle(x, y - 5.0, 2.0, 10.0, Color::from_rgba(115, 223, 222, 255));
    draw_rectangle(
        x - 2.0,
        y - 2.0,
        6.0,
        4.0,
        Color::from_rgba(169, 244, 224, 255),
    );
    draw_rectangle(x - 3.0, y + 5.0, 8.0, 1.0, PAPER);
}

fn draw_hero(x: f32, y: f32, facing: (i32, i32)) {
    const SPRITE_SCALE: f32 = 3.0;
    let bob = if (get_time() * 8.0) as i32 % 2 == 0 {
        0.0
    } else {
        1.0
    };
    let armor = Color::from_rgba(151, 37, 44, 255);
    let armor_light = Color::from_rgba(207, 62, 57, 255);
    let armor_shadow = Color::from_rgba(93, 30, 39, 255);
    let skin = Color::from_rgba(232, 194, 143, 255);
    let sword_side = if facing.0 < 0 { -1.0 } else { 1.0 };

    let rect = |offset_x: f32, offset_y: f32, width: f32, height: f32, color: Color| {
        draw_rectangle(
            x + offset_x * SPRITE_SCALE,
            y + offset_y * SPRITE_SCALE,
            width * SPRITE_SCALE,
            height * SPRITE_SCALE,
            color,
        );
    };

    rect(-7.0, 9.0, 14.0, 2.0, Color::from_rgba(27, 29, 38, 180));
    rect(-9.0, -3.0, 4.0, 10.0, GOLD);
    rect(-8.0, -2.0, 2.0, 8.0, armor_shadow);
    rect(-5.0, -4.0 + bob, 10.0, 11.0, armor_shadow);
    rect(-4.0, -4.0 + bob, 8.0, 9.0, armor);
    rect(-3.0, -3.0 + bob, 6.0, 3.0, armor_light);
    rect(-2.0, 1.0 + bob, 4.0, 3.0, GOLD);
    rect(-1.0, 1.0 + bob, 2.0, 2.0, armor_light);
    rect(-5.0, 5.0 + bob, 4.0, 5.0, armor_shadow);
    rect(1.0, 5.0 + bob, 4.0, 5.0, armor_shadow);
    rect(-5.0, 9.0 + bob, 5.0, 2.0, INK);
    rect(1.0, 9.0 + bob, 5.0, 2.0, INK);

    rect(-3.0, -9.0 + bob, 6.0, 5.0, skin);
    rect(-4.0, -11.0 + bob, 8.0, 3.0, armor_shadow);
    rect(-3.0, -12.0 + bob, 6.0, 3.0, armor_light);
    rect(-1.0, -15.0 + bob, 2.0, 4.0, GOLD);
    rect(-2.0, -7.0 + bob, 4.0, 1.0, INK);
    rect(sword_side * 7.0, -7.0 + bob, 2.0, 10.0, PAPER);
    rect(sword_side * 6.0, 1.0 + bob, 4.0, 2.0, GOLD);
}

fn draw_battle(battle: Option<&Battle>, hero: &Hero, render_scale: f32) {
    let Some(battle) = battle else {
        return;
    };
    draw_rectangle(22.0, 23.0, 276.0, 108.0, Color::from_rgba(22, 27, 38, 247));
    draw_rectangle(23.0, 24.0, 274.0, 106.0, PAPER);
    draw_rectangle(25.0, 26.0, 270.0, 102.0, INK);
    text(battle.name, 37.0, 39.0, 10.0, GOLD, render_scale);
    bar(37.0, 45.0, 72.0, 5.0, battle.hp, battle.max_hp, RED);
    draw_enemy(215.0, 75.0, battle.name == "MARSH WRAITH");
    draw_rectangle(30.0, 88.0, 260.0, 35.0, Color::from_rgba(11, 17, 30, 255));
    text(&battle.message, 37.0, 101.0, 7.0, PAPER, render_scale);
    if battle.phase == 0 {
        let labels = ["ATTACK", "STARFIRE  3 MP", "POTION", "RUN"];
        for (index, label) in labels.iter().enumerate() {
            let color = if index == battle.menu { GOLD } else { PAPER };
            text(
                &format!("{} {}", if index == battle.menu { ">" } else { " " }, label),
                132.0,
                43.0 + index as f32 * 11.0,
                7.0,
                color,
                render_scale,
            );
        }
    }
    text(
        &format!("HERO  HP {:02}   MP {:02}", hero.hp, hero.mp),
        37.0,
        78.0,
        7.0,
        GREEN,
        render_scale,
    );
}

fn draw_enemy(x: f32, y: f32, wraith: bool) {
    let body = if wraith {
        Color::from_rgba(101, 195, 189, 255)
    } else {
        Color::from_rgba(116, 163, 73, 255)
    };
    draw_rectangle(x - 12.0, y - 9.0, 24.0, 21.0, body);
    draw_rectangle(x - 8.0, y - 14.0, 16.0, 9.0, body);
    draw_rectangle(x - 7.0, y - 4.0, 3.0, 3.0, PAPER);
    draw_rectangle(x + 4.0, y - 4.0, 3.0, 3.0, PAPER);
    draw_rectangle(x - 5.0, y - 3.0, 1.0, 2.0, INK);
    draw_rectangle(x + 5.0, y - 3.0, 1.0, 2.0, INK);
    draw_rectangle(x - 8.0, y + 12.0, 5.0, 4.0, INK);
    draw_rectangle(x + 3.0, y + 12.0, 5.0, 4.0, INK);
}

fn draw_inn(hero: &Hero, render_scale: f32) {
    draw_rectangle(20.0, 21.0, 280.0, 117.0, Color::from_rgba(22, 27, 38, 247));
    draw_rectangle(22.0, 23.0, 276.0, 113.0, PAPER);
    draw_rectangle(24.0, 25.0, 272.0, 109.0, INK);
    text(
        "ALDERFALL  /  THE LAST INN",
        37.0,
        43.0,
        10.0,
        GOLD,
        render_scale,
    );
    text(
        "The keeper sets a bowl of stew before you.",
        37.0,
        63.0,
        7.0,
        PAPER,
        render_scale,
    );
    text(
        "Rest to recover HP and MP. Take a potion.",
        37.0,
        76.0,
        7.0,
        PAPER,
        render_scale,
    );
    text(
        "4 G   |   YOUR PURSE: ",
        37.0,
        96.0,
        7.0,
        PAPER,
        render_scale,
    );
    text(
        &format!("{} G", hero.gold),
        137.0,
        96.0,
        7.0,
        GOLD,
        render_scale,
    );
    text(
        "ENTER: REST   /   ESC: LEAVE",
        37.0,
        119.0,
        7.0,
        GREEN,
        render_scale,
    );
}

fn bar(x: f32, y: f32, width: f32, height: f32, value: i32, max: i32, color: Color) {
    draw_rectangle(x, y, width, height, Color::from_rgba(58, 57, 58, 255));
    draw_rectangle(
        x,
        y,
        width * (value.max(0) as f32 / max.max(1) as f32),
        height,
        color,
    );
}

fn text(label: impl AsRef<str>, x: f32, y: f32, size: f32, color: Color, render_scale: f32) {
    let render_scale = render_scale.max(0.01);
    draw_text_ex(
        label,
        x,
        y,
        TextParams {
            font_size: (size * render_scale).round().clamp(1.0, u16::MAX as f32) as u16,
            font_scale: 1.0 / render_scale,
            color,
            ..Default::default()
        },
    );
}
