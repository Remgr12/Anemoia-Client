//! JNI wrappers for world-level queries: entity iteration and combat actions.
//!
//! Design notes:
//! - Entities are represented as `EntitySnapshot` (plain Rust data, no GlobalRef).
//!   All per-entity JNI calls happen in one bulk pass inside `get_entities`.
//! - Block type IDs are returned as plain `String`; no GlobalRef for BlockState/BlockPos.
//! - Every bulk operation uses `push_local_frame`/`pop_local_frame` to bound local-ref
//!   table growth and prevent the table-overflow JVM crashes seen with `GlobalRef`-per-entity.

use anyhow::Result;
use jni::{
    objects::{GlobalRef, JObject, JValue},
    signature::{Primitive, ReturnType},
    JNIEnv,
};

use super::{method_ids, minecraft::Minecraft, paths, player::LocalPlayer};

// ── Entity snapshot ────────────────────────────────────────────────────────────

/// All entity data captured in one JNI pass. No JNI required after creation.
#[derive(Clone)]
pub struct EntitySnapshot {
    pub id:             i32,
    pub x:              f64,
    pub y:              f64,
    pub z:              f64,
    pub yaw:            f32,
    pub pitch:          f32,
    pub alive:          bool,
    pub type_id:        String,
    pub name:           String,
    pub is_local_player: bool,
}

/// mlua `UserData` wrapper around an `EntitySnapshot`.
pub struct LuaEntity(pub EntitySnapshot);

// ── HitResult (still holds a GlobalRef — it is long-lived) ────────────────────

pub struct HitResult {
    pub jni_ref: GlobalRef,
}

impl HitResult {
    pub fn get_type(&self, env: &mut JNIEnv) -> Result<String> {
        let type_obj = env
            .call_method(
                self.jni_ref.as_obj(),
                "getType",
                "()Lnet/minecraft/world/phys/HitResult$Type;",
                &[],
            )?
            .l()?;
        let name_jstr = env
            .call_method(&type_obj, "name", "()Ljava/lang/String;", &[])?
            .l()?;
        let s: String = env.get_string((&name_jstr).into())?.into();
        Ok(s)
    }

    /// Returns a full snapshot of the hit entity (if this is an EntityHitResult).
    pub fn get_entity(&self, env: &mut JNIEnv) -> Result<Option<EntitySnapshot>> {
        let ehr_cls = crate::jvm::Jvm::find_class(env, paths::ENTITY_HIT_RESULT)?;
        if !env.is_instance_of(self.jni_ref.as_obj(), &ehr_cls)? {
            return Ok(None);
        }
        let entity_obj = env
            .call_method(
                self.jni_ref.as_obj(),
                "getEntity",
                "()Lnet/minecraft/world/entity/Entity;",
                &[],
            )?
            .l()?;
        if entity_obj.is_null() {
            return Ok(None);
        }
        let snap = extract_snapshot_slow(env, &entity_obj)?;
        Ok(Some(snap))
    }
}

pub struct LuaHitResult(pub std::sync::Arc<HitResult>);

// ── Entity iteration ───────────────────────────────────────────────────────────

/// Extract one entity snapshot using cached method IDs (fast path).
///
/// # Safety
/// Caller guarantees `m` was resolved for the JVM we are currently attached to,
/// and that every method ID still belongs to a loaded class (always true for MC).
unsafe fn extract_snapshot_fast(
    env: &mut JNIEnv,
    obj: &JObject<'_>,
    m: &method_ids::MethodIds,
) -> Result<EntitySnapshot> {
    env.push_local_frame(8)?;

    let result: Result<EntitySnapshot> = (|| {
        let id    = unsafe { env.call_method_unchecked(obj, m.entity_get_id,    ReturnType::Primitive(Primitive::Int),     &[]) }?.i()?;
        let x     = unsafe { env.call_method_unchecked(obj, m.entity_get_x,     ReturnType::Primitive(Primitive::Double),  &[]) }?.d()?;
        let y     = unsafe { env.call_method_unchecked(obj, m.entity_get_y,     ReturnType::Primitive(Primitive::Double),  &[]) }?.d()?;
        let z     = unsafe { env.call_method_unchecked(obj, m.entity_get_z,     ReturnType::Primitive(Primitive::Double),  &[]) }?.d()?;
        let yaw   = unsafe { env.call_method_unchecked(obj, m.entity_get_y_rot, ReturnType::Primitive(Primitive::Float),   &[]) }?.f()?;
        let pitch = unsafe { env.call_method_unchecked(obj, m.entity_get_x_rot, ReturnType::Primitive(Primitive::Float),   &[]) }?.f()?;
        let alive = unsafe { env.call_method_unchecked(obj, m.entity_is_alive,  ReturnType::Primitive(Primitive::Boolean), &[]) }?.z()?;

        let type_obj   = unsafe { env.call_method_unchecked(obj, m.entity_get_type, ReturnType::Object, &[]) }?.l()?;
        let desc_jstr  = unsafe { env.call_method_unchecked(&type_obj, m.entity_type_get_desc_id, ReturnType::Object, &[]) }?.l()?;
        let type_id: String = env.get_string((&desc_jstr).into())?.into();

        let name_comp  = unsafe { env.call_method_unchecked(obj, m.entity_get_name, ReturnType::Object, &[]) }?.l()?;
        let name_jstr  = unsafe { env.call_method_unchecked(&name_comp, m.component_get_string, ReturnType::Object, &[]) }?.l()?;
        let name: String = env.get_string((&name_jstr).into())?.into();

        let is_local_player = env.is_instance_of(obj, &m.local_player_class)?;

        Ok(EntitySnapshot { id, x, y, z, yaw, pitch, alive, type_id, name, is_local_player })
    })();

    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
    let _ = unsafe { env.pop_local_frame(&JObject::null()) };
    result
}

/// Extract one entity snapshot using named JNI lookups (slow / first-tick fallback).
fn extract_snapshot_slow(env: &mut JNIEnv, obj: &JObject<'_>) -> Result<EntitySnapshot> {
    env.push_local_frame(8)?;

    let result: Result<EntitySnapshot> = (|| {
        let id    = env.call_method(obj, "getId",    "()I", &[])?.i()?;
        let x     = env.call_method(obj, "getX",     "()D", &[])?.d()?;
        let y     = env.call_method(obj, "getY",     "()D", &[])?.d()?;
        let z     = env.call_method(obj, "getZ",     "()D", &[])?.d()?;
        let yaw   = env.call_method(obj, "getYRot",  "()F", &[])?.f()?;
        let pitch = env.call_method(obj, "getXRot",  "()F", &[])?.f()?;
        let alive = env.call_method(obj, "isAlive",  "()Z", &[])?.z()?;

        let type_obj  = env.call_method(obj, "getType",
            "()Lnet/minecraft/world/entity/EntityType;", &[])?.l()?;
        let desc_jstr = env.call_method(&type_obj, "getDescriptionId",
            "()Ljava/lang/String;", &[])?.l()?;
        let type_id: String = env.get_string((&desc_jstr).into())?.into();

        let name_comp = env.call_method(obj, "getName",
            "()Lnet/minecraft/network/chat/Component;", &[])?.l()?;
        let name_jstr = env.call_method(&name_comp, "getString",
            "()Ljava/lang/String;", &[])?.l()?;
        let name: String = env.get_string((&name_jstr).into())?.into();

        let lp_cls = crate::jvm::Jvm::find_class(env, paths::LOCAL_PLAYER)?;
        let is_local_player = env.is_instance_of(obj, &lp_cls)?;

        Ok(EntitySnapshot { id, x, y, z, yaw, pitch, alive, type_id, name, is_local_player })
    })();

    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
    let _ = unsafe { env.pop_local_frame(&JObject::null()) };
    result
}

/// Returns all entities in the current level as data-only `EntitySnapshot`s.
///
/// Local frame management ensures the local-ref table stays bounded regardless
/// of entity count. No `GlobalRef` is created per entity.
pub fn get_entities(mc: &Minecraft, env: &mut JNIEnv) -> Result<Vec<EntitySnapshot>> {
    // Lazy one-time init of method ID cache.
    if method_ids::get().is_none() {
        let _ = method_ids::init(env);
    }
    let ids = method_ids::get();

    // Outer frame holds: level, getter, iterable, iterator (≤ 4 local refs).
    env.push_local_frame(16)?;

    let result: Result<Vec<EntitySnapshot>> = (|| {
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

        let iterable = match ids {
            Some(m) => unsafe {
                env.call_method_unchecked(&getter, m.level_getter_get_all, ReturnType::Object, &[])?
                    .l()?
            },
            None => env
                .call_method(&getter, "getAll", "()Ljava/lang/Iterable;", &[])?
                .l()?,
        };
        if iterable.is_null() {
            return Ok(vec![]);
        }

        let iterator = match ids {
            Some(m) => unsafe {
                env.call_method_unchecked(&iterable, m.iterable_iterator, ReturnType::Object, &[])?
                    .l()?
            },
            None => env
                .call_method(&iterable, "iterator", "()Ljava/util/Iterator;", &[])?
                .l()?,
        };

        let mut snapshots = Vec::new();

        loop {
            let has_next = match ids {
                Some(m) => unsafe {
                    env.call_method_unchecked(
                        &iterator,
                        m.iter_has_next,
                        ReturnType::Primitive(Primitive::Boolean),
                        &[],
                    )?
                    .z()?
                },
                None => env.call_method(&iterator, "hasNext", "()Z", &[])?.z()?,
            };
            if !has_next {
                break;
            }

            let obj = match ids {
                Some(m) => unsafe {
                    env.call_method_unchecked(&iterator, m.iter_next, ReturnType::Object, &[])?
                        .l()?
                },
                None => env
                    .call_method(&iterator, "next", "()Ljava/lang/Object;", &[])?
                    .l()?,
            };

            if obj.is_null() {
                let _ = env.delete_local_ref(obj);
                continue;
            }

            // extract_snapshot_* push/pop their own inner frame for temporaries.
            let snap = match ids {
                Some(m) => unsafe { extract_snapshot_fast(env, &obj, m) },
                None => extract_snapshot_slow(env, &obj),
            };

            // Release entity local ref — prevents outer-frame overflow for large worlds.
            let _ = env.delete_local_ref(obj);

            match snap {
                Ok(s) => snapshots.push(s),
                Err(_) => {
                    // Entity may have been removed mid-iteration; skip it.
                    let _ = env.exception_clear();
                }
            }
        }

        Ok(snapshots)
    })();

    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
    let _ = unsafe { env.pop_local_frame(&JObject::null()) };
    result
}

// ── Block query ────────────────────────────────────────────────────────────────

/// Returns the block's description ID string (e.g. `"block.minecraft.air"`).
///
/// Uses a local frame — no `GlobalRef` is created for BlockPos or BlockState.
pub fn get_block_type_id(mc: &Minecraft, env: &mut JNIEnv, x: i32, y: i32, z: i32) -> Result<String> {
    env.push_local_frame(8)?;

    let result: Result<String> = (|| {
        let level = env
            .get_field(
                mc.jni_ref.as_obj(),
                "level",
                "Lnet/minecraft/client/multiplayer/ClientLevel;",
            )?
            .l()?;
        anyhow::ensure!(!level.is_null(), "level is null");

        // BlockPos as a plain local object — no GlobalRef needed.
        let bp_cls = crate::jvm::Jvm::find_class(env, paths::BLOCK_POS)?;
        let pos = env.new_object(&bp_cls, "(III)V", &[
            JValue::Int(x),
            JValue::Int(y),
            JValue::Int(z),
        ])?;

        let state = env
            .call_method(
                &level,
                "getBlockState",
                "(Lnet/minecraft/core/BlockPos;)Lnet/minecraft/world/level/block/state/BlockState;",
                &[JValue::Object(&pos)],
            )?
            .l()?;
        anyhow::ensure!(!state.is_null(), "getBlockState returned null");

        let block = env
            .call_method(&state, "getBlock", "()Lnet/minecraft/world/level/block/Block;", &[])?
            .l()?;

        let desc_jstr = env
            .call_method(&block, "getDescriptionId", "()Ljava/lang/String;", &[])?
            .l()?;

        let s: String = env.get_string((&desc_jstr).into())?.into();
        Ok(s)
    })();

    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
    let _ = unsafe { env.pop_local_frame(&JObject::null()) };
    result
}

// ── Combat ────────────────────────────────────────────────────────────────────

/// Attacks an entity identified by its entity ID.
///
/// Looks the entity up fresh from the level — no stored GlobalRef required.
pub fn attack_by_id(
    mc: &Minecraft,
    player: &LocalPlayer,
    entity_id: i32,
    env: &mut JNIEnv,
) -> Result<()> {
    env.push_local_frame(8)?;

    let result: Result<()> = (|| {
        let level = env
            .get_field(
                mc.jni_ref.as_obj(),
                "level",
                "Lnet/minecraft/client/multiplayer/ClientLevel;",
            )?
            .l()?;
        anyhow::ensure!(!level.is_null(), "level is null");

        let entity = env
            .call_method(
                &level,
                "getEntity",
                "(I)Lnet/minecraft/world/entity/Entity;",
                &[JValue::Int(entity_id)],
            )?
            .l()?;
        anyhow::ensure!(!entity.is_null(), "entity {} not found in level", entity_id);

        let gamemode = env
            .get_field(
                mc.jni_ref.as_obj(),
                "gameMode",
                "Lnet/minecraft/client/multiplayer/MultiPlayerGameMode;",
            )?
            .l()?;
        anyhow::ensure!(!gamemode.is_null(), "gameMode is null");

        env.call_method(
            &gamemode,
            "attack",
            "(Lnet/minecraft/world/entity/player/Player;Lnet/minecraft/world/entity/Entity;)V",
            &[
                JValue::Object(player.jni_ref.as_obj()),
                JValue::Object(&entity),
            ],
        )?;

        Ok(())
    })();

    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
    let _ = unsafe { env.pop_local_frame(&JObject::null()) };
    result
}
