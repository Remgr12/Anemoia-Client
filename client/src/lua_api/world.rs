use mlua::prelude::*;
use std::sync::Arc;

use crate::{
    jvm::Jvm,
    mc::{
        minecraft::Minecraft,
        player::LocalPlayer,
        world::{self, LuaHitResult},
    },
};

// ── LuaBlock ──────────────────────────────────────────────────────────────────
// Holds a pre-fetched type_id string. No JNI required after construction.

pub struct LuaBlock(pub String);

impl LuaUserData for LuaBlock {
    fn add_methods<M: LuaUserDataMethods<Self>>(m: &mut M) {
        m.add_method("type_id", |_, this, ()| Ok(this.0.clone()));
    }
}

// ── LuaEntity UserData ────────────────────────────────────────────────────────
// EntitySnapshot holds all entity data — no JNI calls needed for any method.

impl LuaUserData for world::LuaEntity {
    fn add_methods<M: LuaUserDataMethods<Self>>(m: &mut M) {
        m.add_method("x",              |_, this, ()| Ok(this.0.x));
        m.add_method("y",              |_, this, ()| Ok(this.0.y));
        m.add_method("z",              |_, this, ()| Ok(this.0.z));
        m.add_method("yaw",            |_, this, ()| Ok(this.0.yaw));
        m.add_method("pitch",          |_, this, ()| Ok(this.0.pitch));
        m.add_method("alive",          |_, this, ()| Ok(this.0.alive));
        m.add_method("type_id",        |_, this, ()| Ok(this.0.type_id.clone()));
        m.add_method("name",           |_, this, ()| Ok(this.0.name.clone()));
        m.add_method("is_local_player",|_, this, ()| Ok(this.0.is_local_player));
        m.add_method("health",         |_, this, ()| Ok(this.0.health));
        m.add_method("id",             |_, this, ()| Ok(this.0.id));
        m.add_method("dist_sq", |_, this, (ox, oy, oz): (f64, f64, f64)| {
            let dx = this.0.x - ox;
            let dy = this.0.y - oy;
            let dz = this.0.z - oz;
            Ok(dx * dx + dy * dy + dz * dz)
        });
    }
}

// ── LuaHitResult UserData ─────────────────────────────────────────────────────

impl LuaUserData for LuaHitResult {
    fn add_methods<M: LuaUserDataMethods<Self>>(m: &mut M) {
        m.add_method("type", |_, this, ()| with_env(|env| this.0.get_type(env)));
        m.add_method("entity", |lua, this, ()| {
            let entity = with_env(|env| this.0.get_entity(env))?;
            match entity {
                Some(snap) => Ok(LuaValue::UserData(
                    lua.create_userdata(world::LuaEntity(snap))?,
                )),
                None => Ok(LuaValue::Nil),
            }
        });
    }
}

// ── Registration ──────────────────────────────────────────────────────────────

pub fn register(lua: &Lua, mc_table: &LuaTable) -> anyhow::Result<()> {
    mc_table.set(
        "hit_result",
        lua.create_function(|lua, ()| {
            let mut env = Jvm::get().attach().map_err(lerr)?;
            let mc_obj = Minecraft::get_instance(&mut env)
                .map_err(lerr)?
                .ok_or_else(|| LuaError::runtime("Minecraft not ready"))?;
            let hr = mc_obj.get_hit_result(&mut env).map_err(lerr)?;
            match hr {
                Some(h) => Ok(LuaValue::UserData(
                    lua.create_userdata(LuaHitResult(Arc::new(h)))?,
                )),
                None => Ok(LuaValue::Nil),
            }
        })?,
    )?;

    mc_table.set(
        "entities",
        lua.create_function(|lua, ()| {
            let mut env = Jvm::get().attach().map_err(lerr)?;
            let mc_obj = match Minecraft::get_instance(&mut env).map_err(lerr)? {
                Some(m) => m,
                None => return Ok(lua.create_table()?),
            };
            let snaps = world::get_entities(&mc_obj, &mut env).map_err(lerr)?;
            let t = lua.create_table()?;
            for (i, snap) in snaps.into_iter().enumerate() {
                t.set(i + 1, world::LuaEntity(snap))?;
            }
            Ok(t)
        })?,
    )?;

    mc_table.set(
        "block",
        lua.create_function(|lua, (x, y, z): (i32, i32, i32)| {
            let mut env = Jvm::get().attach().map_err(lerr)?;
            let mc_obj = Minecraft::get_instance(&mut env)
                .map_err(lerr)?
                .ok_or_else(|| LuaError::runtime("Minecraft not ready"))?;
            let type_id = world::get_block_type_id(&mc_obj, &mut env, x, y, z).map_err(lerr)?;
            Ok(lua.create_userdata(LuaBlock(type_id))?)
        })?,
    )?;

    mc_table.set(
        "attack",
        lua.create_function(|_, entity: LuaAnyUserData| {
            let snap = entity.borrow::<world::LuaEntity>()?;
            let entity_id = snap.0.id;

            let mut env = Jvm::get().attach().map_err(lerr)?;
            let mc_obj = Minecraft::get_instance(&mut env)
                .map_err(lerr)?
                .ok_or_else(|| LuaError::runtime("Minecraft not ready"))?;
            let player = LocalPlayer::from_minecraft(&mc_obj, &mut env)
                .map_err(lerr)?
                .ok_or_else(|| LuaError::runtime("Player is null"))?;

            world::attack_by_id(&mc_obj, &player, entity_id, &mut env).map_err(lerr)
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
    f(&mut env).map_err(|e| {
        let _ = env.exception_clear();
        LuaError::runtime(e.to_string())
    })
}

fn lerr(e: impl std::fmt::Display) -> LuaError {
    LuaError::runtime(e.to_string())
}
