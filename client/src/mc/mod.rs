pub mod minecraft;
pub mod netty;
pub mod packet;
pub mod player;
pub mod reflect;
pub mod window;
pub mod world;
pub mod item;

// Raw JNI class/method paths for Minecraft Java Edition 26.2 (unobfuscated).
// No mapping layer needed — class names match the shipped bytecode directly.
pub mod paths {
    pub const MINECRAFT: &str = "net/minecraft/client/Minecraft";
    pub const LOCAL_PLAYER: &str = "net/minecraft/client/player/LocalPlayer";
    pub const ENTITY: &str = "net/minecraft/world/entity/Entity";
    pub const ENTITY_TYPE: &str = "net/minecraft/world/entity/EntityType";
    pub const COMPONENT: &str = "net/minecraft/network/chat/Component";
    pub const LEVEL: &str = "net/minecraft/world/level/Level";
    pub const CLIENT_LEVEL: &str = "net/minecraft/client/multiplayer/ClientLevel";
    pub const LEVEL_ENTITY_GETTER: &str = "net/minecraft/world/level/entity/LevelEntityGetter";
    pub const INPUT: &str = "net/minecraft/client/player/Input";
    pub const CONNECTION: &str = "net/minecraft/network/Connection";
    pub const PACKET: &str = "net/minecraft/network/protocol/Packet";
    pub const BLOCK_POS: &str = "net/minecraft/core/BlockPos";
    pub const BLOCK_STATE: &str = "net/minecraft/world/level/block/state/BlockState";
    pub const LIVING_ENTITY: &str = "net/minecraft/world/entity/LivingEntity";
    pub const HIT_RESULT: &str = "net/minecraft/world/phys/HitResult";
    pub const ENTITY_HIT_RESULT: &str = "net/minecraft/world/phys/EntityHitResult";
    pub const BLOCK_HIT_RESULT: &str = "net/minecraft/world/phys/BlockHitResult";
    pub const ITEM_STACK: &str = "net/minecraft/world/item/ItemStack";
    pub const MOB_EFFECT: &str = "net/minecraft/world/effect/MobEffect";
    pub const MOB_EFFECT_INSTANCE: &str = "net/minecraft/world/effect/MobEffectInstance";
    pub const OPTIONS: &str = "net/minecraft/client/Options";
    pub const GAME_RENDERER: &str = "net/minecraft/client/renderer/GameRenderer";
    pub const MATRIX4F: &str = "org/joml/Matrix4f";
}

/// Globally cached JNI method IDs for the hot entity-iteration path.
///
/// `JMethodID` values are permanent for the lifetime of the JVM — they are
/// resolved once and reused for every call without paying the name-lookup cost.
pub mod method_ids {
    use std::sync::OnceLock;

    use anyhow::Result;
    use jni::{
        objects::{GlobalRef, JMethodID, JObject},
        JNIEnv,
    };

    pub struct MethodIds {
        // Entity methods
        pub entity_get_id:    JMethodID,
        pub entity_get_x:     JMethodID,
        pub entity_get_y:     JMethodID,
        pub entity_get_z:     JMethodID,
        pub entity_get_y_rot: JMethodID,
        pub entity_get_x_rot: JMethodID,
        pub entity_is_alive:  JMethodID,
        pub entity_get_type:  JMethodID,
        pub entity_get_name:  JMethodID,
        // EntityType / Component
        pub entity_type_get_desc_id: JMethodID,
        pub component_get_string:    JMethodID,
        // Iterator / Iterable / LevelEntityGetter
        pub iterable_iterator:    JMethodID,
        pub iter_has_next:        JMethodID,
        pub iter_next:            JMethodID,
        pub level_getter_get_all: JMethodID,
        /// Cached GlobalRef to LocalPlayer class — avoids repeated find_class in is_instance_of.
        pub local_player_class: GlobalRef,
    }

    // Safety: JMethodID is a raw pointer into the JVM method table. Method IDs are
    // permanent (valid until the declaring class is unloaded; MC classes never are).
    // GlobalRef is inherently thread-safe.
    unsafe impl Send for MethodIds {}
    unsafe impl Sync for MethodIds {}

    static IDS: OnceLock<MethodIds> = OnceLock::new();

    pub fn get() -> Option<&'static MethodIds> {
        IDS.get()
    }

    /// Resolve and cache all method IDs. No-op if already initialised.
    /// On failure, `get()` stays `None` and callers fall back to named lookups.
    pub fn init(env: &mut JNIEnv) -> Result<()> {
        if IDS.get().is_some() {
            return Ok(());
        }

        // Use a local frame so all class references are released together.
        env.push_local_frame(24)?;
        let result = resolve(env);
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
        let _ = unsafe { env.pop_local_frame(&JObject::null()) };

        match result {
            Ok(ids) => { let _ = IDS.set(ids); Ok(()) }
            Err(e)  => Err(e),
        }
    }

    fn resolve(env: &mut JNIEnv) -> Result<MethodIds> {
        use super::paths;
        let jvm = crate::jvm::Jvm::find_class;

        let entity_cls      = jvm(env, paths::ENTITY)?;
        let entity_type_cls = jvm(env, paths::ENTITY_TYPE)?;
        let component_cls   = jvm(env, paths::COMPONENT)?;
        let iterable_cls    = jvm(env, "java/lang/Iterable")?;
        let iter_cls        = jvm(env, "java/util/Iterator")?;
        let getter_cls      = jvm(env, paths::LEVEL_ENTITY_GETTER)?;
        let lp_cls          = jvm(env, paths::LOCAL_PLAYER)?;

        let ids = MethodIds {
            entity_get_id:    env.get_method_id(&entity_cls, "getId",    "()I")?,
            entity_get_x:     env.get_method_id(&entity_cls, "getX",     "()D")?,
            entity_get_y:     env.get_method_id(&entity_cls, "getY",     "()D")?,
            entity_get_z:     env.get_method_id(&entity_cls, "getZ",     "()D")?,
            entity_get_y_rot: env.get_method_id(&entity_cls, "getYRot",  "()F")?,
            entity_get_x_rot: env.get_method_id(&entity_cls, "getXRot",  "()F")?,
            entity_is_alive:  env.get_method_id(&entity_cls, "isAlive",  "()Z")?,
            entity_get_type:  env.get_method_id(&entity_cls, "getType",
                "()Lnet/minecraft/world/entity/EntityType;")?,
            entity_get_name:  env.get_method_id(&entity_cls, "getName",
                "()Lnet/minecraft/network/chat/Component;")?,

            entity_type_get_desc_id: env.get_method_id(
                &entity_type_cls, "getDescriptionId", "()Ljava/lang/String;")?,
            component_get_string: env.get_method_id(
                &component_cls, "getString", "()Ljava/lang/String;")?,

            iterable_iterator: env.get_method_id(
                &iterable_cls, "iterator", "()Ljava/util/Iterator;")?,
            iter_has_next: env.get_method_id(&iter_cls, "hasNext", "()Z")?,
            iter_next:     env.get_method_id(&iter_cls, "next",    "()Ljava/lang/Object;")?,

            level_getter_get_all: env.get_method_id(
                &getter_cls, "getAll", "()Ljava/lang/Iterable;")?,

            local_player_class: env.new_global_ref(&lp_cls)?,
        };

        Ok(ids)
    }
}
