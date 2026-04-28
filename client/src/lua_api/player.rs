use mlua::prelude::*;
use std::sync::Arc;

use crate::{
    jvm::Jvm,
    mc::{minecraft::Minecraft, player::LocalPlayer},
};

/// Exposes `LocalPlayer` to Lua as a `UserData` object.
///
/// Each method re-attaches the calling thread, which is safe since these calls
/// originate from the render-thread hook.
pub struct LuaPlayer(pub Arc<LocalPlayer>);

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

fn with_env<F, T>(f: F) -> LuaResult<T>
where
    F: FnOnce(&mut jni::JNIEnv) -> anyhow::Result<T>,
{
    let mut env = Jvm::get()
        .attach()
        .map_err(|e| LuaError::runtime(e.to_string()))?;
    f(&mut env).map_err(|e| LuaError::runtime(e.to_string()))
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
