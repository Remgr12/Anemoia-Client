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

    pub fn get_name(&self, env: &mut JNIEnv) -> Result<String> {
        let name_component = env
            .call_method(
                self.jni_ref.as_obj(),
                "getName",
                "()Lnet/minecraft/network/chat/Component;",
                &[],
            )?
            .l()?;

        let name_jstr = env
            .call_method(&name_component, "getString", "()Ljava/lang/String;", &[])?
            .l()?;

        let s: String = env.get_string((&name_jstr).into())?.into();
        Ok(s)
    }

    /// `true` if this entity is a `LocalPlayer` (the controlled player).
    pub fn is_local_player(&self, env: &mut JNIEnv) -> Result<bool> {
        let cls = crate::jvm::Jvm::find_class(env, paths::LOCAL_PLAYER)?;
        Ok(env.is_instance_of(self.jni_ref.as_obj(), cls)?)
    }
}

pub struct BlockPos {
    pub jni_ref: GlobalRef,
}

impl BlockPos {
    pub fn new(env: &mut JNIEnv, x: i32, y: i32, z: i32) -> Result<Self> {
        let cls = crate::jvm::Jvm::find_class(env, paths::BLOCK_POS)?;
        let obj = env.new_object(cls, "(III)V", &[
            JValue::Int(x),
            JValue::Int(y),
            JValue::Int(z),
        ])?;
        Ok(BlockPos { jni_ref: env.new_global_ref(obj)? })
    }
}

pub struct BlockState {
    pub jni_ref: GlobalRef,
}

impl BlockState {
    pub fn type_id(&self, env: &mut JNIEnv) -> Result<String> {
        let block = env.call_method(self.jni_ref.as_obj(), "getBlock", "()Lnet/minecraft/world/level/block/Block;", &[])?.l()?;
        let description_id = env.call_method(&block, "getDescriptionId", "()Ljava/lang/String;", &[])?.l()?;
        let s: String = env.get_string((&description_id).into())?.into();
        Ok(s)
    }
}

pub struct HitResult {
    pub jni_ref: GlobalRef,
}

impl HitResult {
    pub fn get_type(&self, env: &mut JNIEnv) -> Result<String> {
        let type_obj = env.call_method(self.jni_ref.as_obj(), "getType", "()Lnet/minecraft/world/phys/HitResult$Type;", &[])?.l()?;
        let name_jstr = env.call_method(&type_obj, "name", "()Ljava/lang/String;", &[])?.l()?;
        let s: String = env.get_string((&name_jstr).into())?.into();
        Ok(s)
    }

    pub fn get_entity(&self, env: &mut JNIEnv) -> Result<Option<Entity>> {
        let cls = crate::jvm::Jvm::find_class(env, paths::ENTITY_HIT_RESULT)?;
        if env.is_instance_of(self.jni_ref.as_obj(), cls)? {
            let entity_obj = env.call_method(self.jni_ref.as_obj(), "getEntity", "()Lnet/minecraft/world/entity/Entity;", &[])?.l()?;
            if entity_obj.is_null() {
                return Ok(None);
            }
            return Ok(Some(Entity { jni_ref: env.new_global_ref(entity_obj)? }));
        }
        Ok(None)
    }
}

/// mlua `UserData` wrapper — holds an `Arc` so multiple Lua references share
/// the same `GlobalRef` without copying.
pub struct LuaEntity(pub Arc<Entity>);

pub struct LuaHitResult(pub Arc<HitResult>);

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

pub fn get_block_state(mc: &Minecraft, env: &mut JNIEnv, x: i32, y: i32, z: i32) -> Result<BlockState> {
    let level = env
        .get_field(
            mc.jni_ref.as_obj(),
            "level",
            "Lnet/minecraft/client/multiplayer/ClientLevel;",
        )?
        .l()?;

    anyhow::ensure!(!level.is_null(), "level is null");

    let pos = BlockPos::new(env, x, y, z)?;
    let state_obj = env.call_method(&level, "getBlockState", "(Lnet/minecraft/core/BlockPos;)Lnet/minecraft/world/level/block/state/BlockState;", &[
        JValue::Object(pos.jni_ref.as_obj())
    ])?.l()?;

    Ok(BlockState { jni_ref: env.new_global_ref(state_obj)? })
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
