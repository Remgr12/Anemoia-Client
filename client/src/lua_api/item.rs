use mlua::prelude::*;
use std::sync::Arc;
use crate::mc::item::ItemStack;
use crate::jvm::Jvm;

pub struct LuaItemStack(pub Arc<ItemStack>);

impl LuaUserData for LuaItemStack {
    fn add_methods<M: LuaUserDataMethods<Self>>(m: &mut M) {
        m.add_method("type_id", |_, this, ()| {
            with_env(|env| this.0.type_id(env))
        });
        m.add_method("is_empty", |_, this, ()| {
            with_env(|env| this.0.is_empty(env))
        });
    }
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
