use mlua::prelude::*;
use std::sync::Arc;

use crate::{
    jvm::Jvm,
    mc::{minecraft::Minecraft, player::{LocalPlayer, PlayerInventory}},
};

/// Exposes `LocalPlayer` to Lua as a `UserData` object.
pub struct LuaPlayer(pub Arc<LocalPlayer>);

pub struct LuaInventory(pub Arc<PlayerInventory>);

impl LuaUserData for LuaInventory {
    fn add_methods<M: LuaUserDataMethods<Self>>(m: &mut M) {
        m.add_method("selected_slot", |_, this, ()| {
            with_env(|env| this.0.get_selected_slot(env))
        });
        m.add_method("set_selected_slot", |_, this, (v,): (i32,)| {
            with_env(|env| this.0.set_selected_slot(env, v))
        });
        m.add_method("item_at", |lua, this, (slot,): (i32,)| {
            let item = with_env(|env| this.0.get_item_at(env, slot))?;
            Ok(lua.create_userdata(super::item::LuaItemStack(Arc::new(item)))?)
        });
    }
}

impl LuaUserData for LuaPlayer {
    fn add_methods<M: LuaUserDataMethods<Self>>(m: &mut M) {
        m.add_method("x", |_, this, ()| jni_f64(&this.0, LocalPlayer::get_x));
        m.add_method("y", |_, this, ()| jni_f64(&this.0, LocalPlayer::get_y));
        m.add_method("z", |_, this, ()| jni_f64(&this.0, LocalPlayer::get_z));
        m.add_method("yaw", |_, this, ()| jni_f32(&this.0, LocalPlayer::get_yaw));
        m.add_method("pitch", |_, this, ()| {
            jni_f32(&this.0, LocalPlayer::get_pitch)
        });

        m.add_method("set_yaw", |_, this, (v,): (f32,)| {
            with_env(|env| this.0.set_yaw(env, v))
        });

        m.add_method("set_pitch", |_, this, (v,): (f32,)| {
            with_env(|env| this.0.set_pitch(env, v))
        });

        m.add_method("velocity", |lua, this, ()| {
            let (x, y, z) = with_env(|env| this.0.get_delta_movement(env))?;
            let t = lua.create_table()?;
            t.set(1, x)?;
            t.set(2, y)?;
            t.set(3, z)?;
            Ok(t)
        });

        m.add_method("set_velocity", |_, this, (x, y, z): (f64, f64, f64)| {
            with_env(|env| this.0.set_delta_movement(env, x, y, z))
        });

        m.add_method("on_ground", |_, this, ()| {
            with_env(|env| this.0.on_ground(env))
        });

        m.add_method("set_on_ground", |_, this, (v,): (bool,)| {
            with_env(|env| this.0.set_on_ground(env, v))
        });

        m.add_method("is_sprinting", |_, this, ()| {
            with_env(|env| this.0.is_sprinting(env))
        });

        m.add_method("set_sprinting", |_, this, (v,): (bool,)| {
            with_env(|env| this.0.set_sprinting(env, v))
        });

        m.add_method("is_using_item", |_, this, ()| {
            with_env(|env| this.0.is_using_item(env))
        });

        m.add_method("stop_using_item", |_, this, ()| {
            with_env(|env| this.0.stop_using_item(env))
        });

        m.add_method("food_level", |_, this, ()| {
            with_env(|env| this.0.get_food_level(env))
        });

        m.add_method("main_hand_item", |lua, this, ()| {
            let item = with_env(|env| this.0.get_main_hand_item(env))?;
            Ok(lua.create_userdata(super::item::LuaItemStack(Arc::new(item)))?)
        });

        m.add_method("off_hand_item", |lua, this, ()| {
            let item = with_env(|env| this.0.get_off_hand_item(env))?;
            Ok(lua.create_userdata(super::item::LuaItemStack(Arc::new(item)))?)
        });

        m.add_method("health", |_, this, ()| {
            with_env(|env| this.0.get_health(env))
        });

        m.add_method("max_health", |_, this, ()| {
            with_env(|env| this.0.get_max_health(env))
        });

        m.add_method("absorption", |_, this, ()| {
            with_env(|env| this.0.get_absorption_amount(env))
        });

        m.add_method("swing_arm", |_, this, (hand,): (Option<String>,)| {
            let hand = hand.unwrap_or_else(|| "MAIN_HAND".to_owned());
            with_env(|env| this.0.swing_arm(env, &hand))
        });

        m.add_method("has_effect", |_, this, (effect_name,): (String,)| {
            with_env(|env| {
                let field = match mob_effect_field(&effect_name) {
                    Some(f) => f,
                    None => return Ok(false),
                };
                let cls = match Jvm::find_class(env, "net/minecraft/world/effect/MobEffects") {
                    Ok(c) => c,
                    Err(_) => { let _ = env.exception_clear(); return Ok(false); }
                };
                // Try Holder (MC 1.20.2+/26.x) then legacy MobEffect
                let effect = match env.get_static_field(&cls, field, "Lnet/minecraft/core/Holder;").and_then(|v| v.l()) {
                    Ok(obj) if !obj.is_null() => obj,
                    _ => {
                        let _ = env.exception_clear();
                        match env.get_static_field(&cls, field, "Lnet/minecraft/world/effect/MobEffect;").and_then(|v| v.l()) {
                            Ok(obj) if !obj.is_null() => obj,
                            _ => { let _ = env.exception_clear(); return Ok(false); }
                        }
                    }
                };
                this.0.has_effect(env, effect)
            })
        });

        m.add_method("jump", |_, this, ()| {
            with_env(|env| this.0.jump(env))
        });

        m.add_method("get_step_height", |_, this, ()| {
            with_env(|env| this.0.get_step_height(env))
        });

        m.add_method("set_step_height", |_, this, (v,): (f32,)| {
            with_env(|env| this.0.set_step_height(env, v))
        });

        m.add_method("is_collided_horizontally", |_, this, ()| {
            with_env(|env| this.0.is_collided_horizontally(env))
        });

        m.add_method("is_in_water", |_, this, ()| {
            with_env(|env| this.0.is_in_water(env))
        });

        m.add_method("is_in_lava", |_, this, ()| {
            with_env(|env| this.0.is_in_lava(env))
        });

        m.add_method("is_dead", |_, this, ()| {
            with_env(|env| this.0.is_dead(env))
        });

        m.add_method("is_in_web", |_, this, ()| {
            with_env(|env| this.0.is_in_web(env))
        });

        m.add_method("respawn", |_, this, ()| {
            with_env(|env| this.0.respawn(env))
        });

        m.add_method("remove_effect", |_, this, (effect_name,): (String,)| {
            with_env(|env| {
                let field = match mob_effect_field(&effect_name) {
                    Some(f) => f,
                    None => return Ok(()),
                };
                let cls = match Jvm::find_class(env, "net/minecraft/world/effect/MobEffects") {
                    Ok(c) => c,
                    Err(_) => { let _ = env.exception_clear(); return Ok(()); }
                };
                let effect = match env.get_static_field(&cls, field, "Lnet/minecraft/core/Holder;").and_then(|v| v.l()) {
                    Ok(obj) if !obj.is_null() => obj,
                    _ => {
                        let _ = env.exception_clear();
                        match env.get_static_field(&cls, field, "Lnet/minecraft/world/effect/MobEffect;").and_then(|v| v.l()) {
                            Ok(obj) if !obj.is_null() => obj,
                            _ => { let _ = env.exception_clear(); return Ok(()); }
                        }
                    }
                };
                this.0.remove_effect(env, effect)
            })
        });

        m.add_method("container_id", |_, this, ()| {
            with_env(|env| this.0.get_container_id(env))
        });

        m.add_method("fall_distance", |_, this, ()| {
            with_env(|env| this.0.get_fall_distance(env))
        });

        m.add_method("hurt_time", |_, this, ()| {
            with_env(|env| this.0.get_hurt_time(env))
        });

        m.add_method("input", |lua, this, ()| {
            let t = lua.create_table()?;
            let input = match with_env(|env| this.0.get_input(env)) {
                Ok(i) => i,
                Err(_) => {
                    t.set("left", false)?; t.set("right", false)?;
                    t.set("up", false)?;   t.set("down", false)?;
                    t.set("jumping", false)?; t.set("sneaking", false)?;
                    return Ok(t);
                }
            };
            t.set("left", input.left)?;
            t.set("right", input.right)?;
            t.set("up", input.up)?;
            t.set("down", input.down)?;
            t.set("jumping", input.jumping)?;
            t.set("sneaking", input.shift_key_down)?;
            Ok(t)
        });

        m.add_method("inventory", |lua, this, ()| {
            let inv = with_env(|env| this.0.get_inventory(env))?;
            Ok(lua.create_userdata(LuaInventory(Arc::new(inv)))?)
        });

        m.add_method("display_message", |_, this, (message,): (String,)| {
            with_env(|env| this.0.display_message(env, &message))
        });

        m.add_method("set_pos", |_, this, (x, y, z): (f64, f64, f64)| {
            with_env(|env| this.0.set_pos(env, x, y, z))
        });

        m.add_method("set_no_physics", |_, this, (v,): (bool,)| {
            with_env(|env| this.0.set_no_physics(env, v))
        });
    }
}

/// Registers `mc.player() → LuaPlayer | nil`.
pub fn register(lua: &Lua, mc: &LuaTable) -> anyhow::Result<()> {
    mc.set(
        "player",
        lua.create_function(|lua, ()| {
            let jvm = Jvm::get();
            let mut env = jvm
                .attach()
                .map_err(|e| LuaError::runtime(e.to_string()))?;

            let mc_obj = Minecraft::get_instance(&mut env)
                .map_err(|e| LuaError::runtime(e.to_string()))?;
            let mc_obj = match mc_obj {
                Some(m) => m,
                None => return Ok(LuaValue::Nil),
            };

            let player = LocalPlayer::from_minecraft(&mc_obj, &mut env)
                .map_err(|e| LuaError::runtime(e.to_string()))?;
            match player {
                Some(p) => Ok(LuaValue::UserData(
                    lua.create_userdata(LuaPlayer(Arc::new(p)))?,
                )),
                None => Ok(LuaValue::Nil),
            }
        })?,
    )?;

    Ok(())
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn mob_effect_field(name: &str) -> Option<&'static str> {
    match name.to_lowercase().replace(['-', ' '], "_").as_str() {
        "speed" | "movement_speed"        => Some("SPEED"),
        "slowness" | "movement_slowdown"  => Some("SLOWNESS"),
        "haste" | "dig_speed"             => Some("HASTE"),
        "mining_fatigue" | "dig_slowdown" => Some("MINING_FATIGUE"),
        "strength" | "damage_boost"       => Some("STRENGTH"),
        "instant_health" | "heal"         => Some("INSTANT_HEALTH"),
        "instant_damage" | "harm"         => Some("INSTANT_DAMAGE"),
        "jump_boost" | "jump"             => Some("JUMP_BOOST"),
        "nausea" | "confusion"            => Some("NAUSEA"),
        "regeneration"                    => Some("REGENERATION"),
        "resistance" | "damage_resistance"=> Some("RESISTANCE"),
        "fire_resistance"                 => Some("FIRE_RESISTANCE"),
        "water_breathing"                 => Some("WATER_BREATHING"),
        "invisibility"                    => Some("INVISIBILITY"),
        "blindness"                       => Some("BLINDNESS"),
        "night_vision"                    => Some("NIGHT_VISION"),
        "hunger"                          => Some("HUNGER"),
        "weakness"                        => Some("WEAKNESS"),
        "poison"                          => Some("POISON"),
        "wither"                          => Some("WITHER"),
        "health_boost"                    => Some("HEALTH_BOOST"),
        "absorption"                      => Some("ABSORPTION"),
        "saturation"                      => Some("SATURATION"),
        "levitation"                      => Some("LEVITATION"),
        "luck"                            => Some("LUCK"),
        "bad_luck" | "unluck"             => Some("UNLUCK"),
        "slow_falling"                    => Some("SLOW_FALLING"),
        "conduit_power"                   => Some("CONDUIT_POWER"),
        "dolphins_grace"                  => Some("DOLPHINS_GRACE"),
        "bad_omen"                        => Some("BAD_OMEN"),
        "hero_of_the_village"             => Some("HERO_OF_THE_VILLAGE"),
        "darkness"                        => Some("DARKNESS"),
        _                                 => None,
    }
}

fn with_env<F, T>(f: F) -> LuaResult<T>
where
    F: FnOnce(&mut jni::JNIEnv) -> anyhow::Result<T>,
{
    let mut env = Jvm::get()
        .attach()
        .map_err(|e| LuaError::runtime(e.to_string()))?;
    f(&mut env).map_err(|e| {
        let _ = env.exception_clear();
        LuaError::runtime(e.to_string())
    })
}

fn jni_f64(
    p: &Arc<LocalPlayer>,
    f: fn(&LocalPlayer, &mut jni::JNIEnv) -> anyhow::Result<f64>,
) -> LuaResult<f64> {
    with_env(|env| f(p, env))
}

fn jni_f32(
    p: &Arc<LocalPlayer>,
    f: fn(&LocalPlayer, &mut jni::JNIEnv) -> anyhow::Result<f32>,
) -> LuaResult<f32> {
    with_env(|env| f(p, env))
}
