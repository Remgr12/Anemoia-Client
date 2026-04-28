use mlua::prelude::*;
use std::sync::Arc;

use crate::{
    jvm::Jvm,
    mc::{
        minecraft::Minecraft,
        player::LocalPlayer,
        world::{self, LuaEntity},
    },
};

// ── LuaEntity UserData ────────────────────────────────────────────────────────

impl LuaUserData for LuaEntity {
    fn add_methods<M: LuaUserDataMethods<Self>>(m: &mut M) {
        m.add_method("x", |_, this, ()| with_env(|env| this.0.get_x(env)));
        m.add_method("y", |_, this, ()| with_env(|env| this.0.get_y(env)));
        m.add_method("z", |_, this, ()| with_env(|env| this.0.get_z(env)));
        m.add_method("yaw", |_, this, ()| with_env(|env| this.0.get_yaw(env)));
        m.add_method("pitch", |_, this, ()| with_env(|env| this.0.get_pitch(env)));
        m.add_method("alive", |_, this, ()| with_env(|env| this.0.is_alive(env)));
        m.add_method("type_id", |_, this, ()| with_env(|env| this.0.type_id(env)));
        m.add_method("is_local_player", |_, this, ()| {
            with_env(|env| this.0.is_local_player(env))
        });
        m.add_method(
            "dist_sq",
            |_, this, (ox, oy, oz): (f64, f64, f64)| {
                with_env(|env| {
                    let x = this.0.get_x(env)?;
                    let y = this.0.get_y(env)?;
                    let z = this.0.get_z(env)?;
                    Ok((x - ox).powi(2) + (y - oy).powi(2) + (z - oz).powi(2))
                })
            },
        );
    }
}

// ── Registration ──────────────────────────────────────────────────────────────

pub fn register(lua: &Lua, mc_table: &LuaTable) -> anyhow::Result<()> {
    // mc.entities() → array of LuaEntity
    mc_table.set(
        "entities",
        lua.create_function(|lua, ()| {
            let jvm = Jvm::get();
            let mut env = jvm.attach().map_err(lerr)?;

            let mc_obj = Minecraft::get_instance(&mut env)
                .map_err(lerr)?;
            let mc_obj = match mc_obj {
                Some(m) => m,
                None => return Ok(lua.create_table()?),
            };

            let entities = world::get_entities(&mc_obj, &mut env).map_err(lerr)?;

            let t = lua.create_table()?;
            for (i, e) in entities.into_iter().enumerate() {
                t.set(i + 1, LuaEntity(Arc::new(e)))?;
            }
            Ok(t)
        })?,
    )?;

    // mc.attack(entity) — attacks the given LuaEntity
    mc_table.set(
        "attack",
        lua.create_function(|_, entity: LuaAnyUserData| {
            let entity_ref = entity.borrow::<LuaEntity>()?;

            let jvm = Jvm::get();
            let mut env = jvm.attach().map_err(lerr)?;

            let mc_obj = Minecraft::get_instance(&mut env)
                .map_err(lerr)?
                .ok_or_else(|| LuaError::runtime("Minecraft not ready"))?;

            let player = LocalPlayer::from_minecraft(&mc_obj, &mut env)
                .map_err(lerr)?
                .ok_or_else(|| LuaError::runtime("Player is null"))?;

            world::attack(&mc_obj, &player, &entity_ref.0, &mut env).map_err(lerr)
        })?,
    )?;

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn with_env<F, T>(f: F) -> LuaResult<T>
where
    F: FnOnce(&mut jni::JNIEnv) -> anyhow::Result<T>,
{
    let mut env = Jvm::get()
        .attach()
        .map_err(|e| LuaError::runtime(e.to_string()))?;
    f(&mut env).map_err(|e| LuaError::runtime(e.to_string()))
}

fn lerr(e: impl std::fmt::Display) -> LuaError {
    LuaError::runtime(e.to_string())
}
