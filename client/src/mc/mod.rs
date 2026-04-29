pub mod minecraft;
pub mod packet;
pub mod player;
pub mod window;
pub mod world;
pub mod item;

// Raw JNI class/method paths for Minecraft Java Edition 26.2 (unobfuscated).
// No mapping layer needed — class names match the shipped bytecode directly.
pub mod paths {
    pub const MINECRAFT: &str = "net/minecraft/client/Minecraft";
    pub const LOCAL_PLAYER: &str = "net/minecraft/client/player/LocalPlayer";
    pub const ENTITY: &str = "net/minecraft/world/entity/Entity";
    pub const LEVEL: &str = "net/minecraft/world/level/Level";
    pub const CLIENT_LEVEL: &str = "net/minecraft/client/multiplayer/ClientLevel";
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
