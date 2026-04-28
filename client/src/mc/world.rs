//! JNI wrappers for world-level queries: entity iteration and combat actions.

use anyhow::Result;
use jni::{
    objects::{GlobalRef, JValue},
    JNIEnv,
};
use std::sync::Arc;

use super::{minecraft::Minecraft, paths, player::LocalPlayer};

// ── Entity ────────────────────────────────────────────────────────────────────

pub struct Entity {
    pub jni_ref: GlobalRef,
}

impl Entity {
    pub fn get_x(&self, env: &mut JNIEnv) -> Result<f64> {
        Ok(env.call_method(self.jni_ref.as_obj(), "getX", "()D", &[])?.d()?)
    }

    pub fn get_y(&self, env: &mut JNIEnv) -> Result<f64> {
        Ok(env.call_method(self.jni_ref.as_obj(), "getY", "()D", &[])?.d()?)
    }

    pub fn get_z(&self, env: &mut JNIEnv) -> Result<f64> {
        Ok(env.call_method(self.jni_ref.as_obj(), "getZ", "()D", &[])?.d()?)
    }

    pub fn get_yaw(&self, env: &mut JNIEnv) -> Result<f32> {
        Ok(env.call_method(self.jni_ref.as_obj(), "getYRot", "()F", &[])?.f()?)
    }

    pub fn get_pitch(&self, env: &mut JNIEnv) -> Result<f32> {
        Ok(env.call_method(self.jni_ref.as_obj(), "getXRot", "()F", &[])?.f()?)
    }

    pub fn is_alive(&self, env: &mut JNIEnv) -> Result<bool> {
        Ok(env.call_method(self.jni_ref.as_obj(), "isAlive", "()Z", &[])?.z()?)
    }

    /// Returns `entity.getType().getDescriptionId()` — e.g. `"entity.minecraft.zombie"`.
    pub fn type_id(&self, env: &mut JNIEnv) -> Result<String> {
        let entity_type = env
            .call_method(
                self.jni_ref.as_obj(),
                "getType",
                "()Lnet/minecraft/world/entity/EntityType;",
                &[],
            )?
            .l()?;

        let id_jstr = env
            .call_method(&entity_type, "getDescriptionId", "()Ljava/lang/String;", &[])?
            .l()?;

        // Bind to a local first so the JavaStr borrow ends before id_jstr drops.
        let s: String = env.get_string((&id_jstr).into())?.into();
        Ok(s)
    }

    /// `true` if this entity is a `LocalPlayer` (the controlled player).
    pub fn is_local_player(&self, env: &mut JNIEnv) -> Result<bool> {
        let cls = env.find_class(paths::LOCAL_PLAYER)?;
        Ok(env.is_instance_of(self.jni_ref.as_obj(), cls)?)
    }
}

/// mlua `UserData` wrapper — holds an `Arc` so multiple Lua references share
/// the same `GlobalRef` without copying.
pub struct LuaEntity(pub Arc<Entity>);

// ── World queries ─────────────────────────────────────────────────────────────

/// Returns all entities in the current level by iterating `level.getEntities().getAll()`.
///
/// Returns an empty list when not in a world (level == null).
pub fn get_entities(mc: &Minecraft, env: &mut JNIEnv) -> Result<Vec<Entity>> {
    let level = env
        .get_field(
            mc.jni_ref.as_obj(),
            "level",
            "Lnet/minecraft/client/multiplayer/ClientLevel;",
        )?
        .l()?;

    if level.is_null() {
        return Ok(vec![]);
    }

    // level.getEntities() → LevelEntityGetter<Entity>
    let getter = env
        .call_method(
            &level,
            "getEntities",
            "()Lnet/minecraft/world/level/entity/LevelEntityGetter;",
            &[],
        )?
        .l()?;

    if getter.is_null() {
        return Ok(vec![]);
    }

    // getter.getAll() → Iterable<Entity> (generic erased to raw Iterable)
    let iterable = env
        .call_method(&getter, "getAll", "()Ljava/lang/Iterable;", &[])?
        .l()?;

    if iterable.is_null() {
        return Ok(vec![]);
    }

    // Iterate with java.util.Iterator
    let iterator = env
        .call_method(&iterable, "iterator", "()Ljava/util/Iterator;", &[])?
        .l()?;

    let mut entities = vec![];

    loop {
        let has_next = env
            .call_method(&iterator, "hasNext", "()Z", &[])?
            .z()?;

        if !has_next {
            break;
        }

        let obj = env
            .call_method(&iterator, "next", "()Ljava/lang/Object;", &[])?
            .l()?;

        if obj.is_null() {
            continue;
        }

        entities.push(Entity {
            jni_ref: env.new_global_ref(obj)?,
        });
    }

    Ok(entities)
}

// ── Combat ────────────────────────────────────────────────────────────────────

/// Calls `MultiPlayerGameMode.attack(player, target)`.
///
/// MC descriptor:
///   `(Lnet/minecraft/world/entity/player/Player;Lnet/minecraft/world/entity/Entity;)V`
pub fn attack(mc: &Minecraft, player: &LocalPlayer, target: &Entity, env: &mut JNIEnv) -> Result<()> {
    let gamemode = env
        .get_field(
            mc.jni_ref.as_obj(),
            "gameMode",
            "Lnet/minecraft/client/multiplayer/MultiPlayerGameMode;",
        )?
        .l()?;

    anyhow::ensure!(!gamemode.is_null(), "gameMode is null (not in a world?)");

    env.call_method(
        &gamemode,
        "attack",
        "(Lnet/minecraft/world/entity/player/Player;Lnet/minecraft/world/entity/Entity;)V",
        &[
            JValue::Object(player.jni_ref.as_obj()),
            JValue::Object(target.jni_ref.as_obj()),
        ],
    )?;

    Ok(())
}
