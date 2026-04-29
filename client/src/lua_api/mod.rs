mod player;
mod world;
pub mod render;
mod packet;
pub mod item;
pub mod http;

use anyhow::Result;
use mlua::prelude::*;

/// Registers the full `mc` and `anemoia` globals into the Lua state.
pub fn register(lua: &Lua) -> Result<()> {
    let mc = lua.create_table()?;
    player::register(lua, &mc)?;
    world::register(lua, &mc)?;
    lua.globals().set("mc", &mc)?;

    let anemoia = lua.create_table()?;
    let modules = lua.create_table()?;
    anemoia.set("_modules", modules)?;
    render::register(lua, &anemoia)?;
    packet::register(lua, &anemoia)?;
    http::register(lua, &anemoia)?;

    anemoia.set(
        "register",
        lua.create_function(|lua, module: LuaTable| {
            let anemoia: LuaTable = lua.globals().get("anemoia")?;
            let list: LuaTable = anemoia.get("_modules")?;
            list.push(module)?;
            Ok(())
        })?,
    )?;

    mc.set(
        "click",
        lua.create_function(|_, ()| {
            let jvm = crate::jvm::Jvm::get();
            let mut env = jvm
                .attach()
                .map_err(|e| LuaError::runtime(e.to_string()))?;

            let mc_obj = crate::mc::minecraft::Minecraft::get_instance(&mut env)
                .map_err(|e| LuaError::runtime(e.to_string()))?
                .ok_or_else(|| LuaError::runtime("Minecraft not ready"))?;

            mc_obj.left_click_mouse(&mut env).map_err(|e| LuaError::runtime(e.to_string()))
        })?,
    )?;

    mc.set(
        "use_item",
        lua.create_function(|_, hand: Option<String>| {
            with_env(|env| {
                let mc_obj = crate::mc::minecraft::Minecraft::get_instance(env)?
                    .ok_or_else(|| anyhow::anyhow!("Minecraft not ready"))?;
                mc_obj.use_item(env, &hand.unwrap_or("MAIN_HAND".to_owned()))
            })
        })?,
    )?;

    mc.set(
        "right_click",
        lua.create_function(|_, ()| {
            let jvm = crate::jvm::Jvm::get();
            let mut env = jvm
                .attach()
                .map_err(|e| LuaError::runtime(e.to_string()))?;

            let mc_obj = crate::mc::minecraft::Minecraft::get_instance(&mut env)
                .map_err(|e| LuaError::runtime(e.to_string()))?
                .ok_or_else(|| LuaError::runtime("Minecraft not ready"))?;

            mc_obj.right_click_mouse(&mut env).map_err(|e| LuaError::runtime(e.to_string()))
        })?,
    )?;

    mc.set(
        "send_packet",
        lua.create_function(|_, (packet_ud, raw): (LuaAnyUserData, Option<bool>)| {
            let packet_ref = packet_ud.borrow::<crate::mc::packet::Packet>()?;
            
            let jvm = crate::jvm::Jvm::get();
            let mut env = jvm.attach().map_err(|e| LuaError::runtime(e.to_string()))?;

            let mc_obj = crate::mc::minecraft::Minecraft::get_instance(&mut env)
                .map_err(|e| LuaError::runtime(e.to_string()))?
                .ok_or_else(|| LuaError::runtime("Minecraft not ready"))?;

            let player = crate::mc::player::LocalPlayer::from_minecraft(&mc_obj, &mut env)
                .map_err(|e| LuaError::runtime(e.to_string()))?
                .ok_or_else(|| LuaError::runtime("Player is null"))?;

            let connection = player.get_connection(&mut env)
                .map_err(|e| LuaError::runtime(e.to_string()))?;

            connection.send(&mut env, &packet_ref, !raw.unwrap_or(false)).map_err(|e| LuaError::runtime(e.to_string()))
        })?,
    )?;

    mc.set(
        "is_key_down",
        lua.create_function(|_, key: i32| {
            let glfw_handle = match unsafe { crate::glfw::Glfw::load() } {
                Ok(h) => h,
                Err(e) => return Err(LuaError::runtime(e.to_string())),
            };

            let jvm = crate::jvm::Jvm::get();
            let mut env = jvm
                .attach()
                .map_err(|e| LuaError::runtime(e.to_string()))?;

            let mc_obj = crate::mc::minecraft::Minecraft::get_instance(&mut env)
                .map_err(|e| LuaError::runtime(e.to_string()))?
                .ok_or_else(|| LuaError::runtime("Minecraft not ready"))?;

            let window = crate::mc::window::get_glfw_window(&mc_obj, &mut env)
                .map_err(|e| LuaError::runtime(e.to_string()))?;

            Ok(glfw_handle.key_pressed(window, key))
        })?,
    )?;

    mc.set(
        "right_click_delay",
        lua.create_function(|_, ()| {
            with_env(|env| {
                let mc_obj = crate::mc::minecraft::Minecraft::get_instance(env)?
                    .ok_or_else(|| anyhow::anyhow!("Minecraft not ready"))?;
                mc_obj.get_right_click_delay_timer(env)
            })
        })?,
    )?;

    mc.set(
        "set_right_click_delay",
        lua.create_function(|_, delay: i32| {
            with_env(|env| {
                let mc_obj = crate::mc::minecraft::Minecraft::get_instance(env)?
                    .ok_or_else(|| anyhow::anyhow!("Minecraft not ready"))?;
                mc_obj.set_right_click_delay_timer(env, delay)
            })
        })?,
    )?;

    mc.set(
        "set_gamma",
        lua.create_function(|_, value: f64| {
            with_env(|env| {
                let mc_obj = crate::mc::minecraft::Minecraft::get_instance(env)?
                    .ok_or_else(|| anyhow::anyhow!("Minecraft not ready"))?;
                mc_obj.set_gamma(env, value)
            })
        })?,
    )?;

    mc.set(
        "inventory_click",
        lua.create_function(|_, (container_id, slot, button, click_type): (i32, i32, i32, String)| {
            with_env(|env| {
                let mc_obj = crate::mc::minecraft::Minecraft::get_instance(env)?
                    .ok_or_else(|| anyhow::anyhow!("Minecraft not ready"))?;
                mc_obj.inventory_click(env, container_id, slot, button, &click_type)
            })
        })?,
    )?;

    lua.globals().set("anemoia", anemoia)?;
    Ok(())
}

fn with_env<F, T>(f: F) -> LuaResult<T>
where
    F: FnOnce(&mut jni::JNIEnv) -> anyhow::Result<T>,
{
    let mut env = crate::jvm::Jvm::get()
        .attach()
        .map_err(|e| LuaError::runtime(e.to_string()))?;
    f(&mut env).map_err(|e| LuaError::runtime(e.to_string()))
}
