use mlua::prelude::*;
use crate::jvm::Jvm;
use crate::mc::packet::Packet;

pub fn register(lua: &Lua, anemoia: &LuaTable) -> anyhow::Result<()> {
    anemoia.set("on_packet_send", lua.create_function(|lua, func: LuaFunction| {
        let registry_key = lua.create_registry_value(func)?;
        let callbacks = crate::lua_engine::get_packet_send_callbacks();
        callbacks.lock().push(registry_key);
        Ok(())
    })?)?;

    anemoia.set("create_position_packet", lua.create_function(|lua, (x, y, z, on_ground): (f64, f64, f64, bool)| {
        with_env(|env| {
            let cls = Jvm::find_class(env, "net/minecraft/network/protocol/game/ServerboundMovePlayerPacket$Pos")?;
            let obj = env.new_object(cls, "(DDDZ)V", &[
                jni::objects::JValue::Double(x),
                jni::objects::JValue::Double(y),
                jni::objects::JValue::Double(z),
                jni::objects::JValue::Bool(on_ground as jni::sys::jboolean),
            ])?;
            Ok(lua.create_userdata(Packet { jni_ref: env.new_global_ref(obj)? })?)
        })
    })?)?;

    Ok(())
}

fn with_env<F, T>(f: F) -> LuaResult<T>
where
    F: FnOnce(&mut jni::JNIEnv) -> anyhow::Result<T>,
{
    let mut env = Jvm::get()
        .attach()
        .map_err(|e| LuaError::runtime(e.to_string()))?;
    f(&mut env).map_err(|e| LuaError::runtime(e.to_string()))
}
