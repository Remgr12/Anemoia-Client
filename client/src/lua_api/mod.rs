mod player;
mod world;

use anyhow::Result;
use mlua::prelude::*;

/// Registers the full `mc` and `anemoia` globals into the Lua state.
pub fn register(lua: &Lua) -> Result<()> {
    let mc = lua.create_table()?;
    player::register(lua, &mc)?;
    world::register(lua, &mc)?;
    lua.globals().set("mc", mc)?;

    let anemoia = lua.create_table()?;
    let modules = lua.create_table()?;
    anemoia.set("_modules", modules)?;

    anemoia.set(
        "register",
        lua.create_function(|lua, module: LuaTable| {
            let anemoia: LuaTable = lua.globals().get("anemoia")?;
            let list: LuaTable = anemoia.get("_modules")?;
            list.push(module)?;
            Ok(())
        })?,
    )?;

    lua.globals().set("anemoia", anemoia)?;
    Ok(())
}
