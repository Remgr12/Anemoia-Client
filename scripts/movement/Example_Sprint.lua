-- Example: keep the player sprinting by zeroing downward Y velocity each tick
-- and boosting horizontal speed slightly.
--
-- Drop this file into ~/.config/anemoia/scripts/ (or $ANEMOIA_SCRIPTS).
-- Toggle `enabled` to activate.

local module = {
    name = "SpeedBoost",
    description = "Slight horizontal speed boost while on ground",
    category = "Movement",
    enabled = false,
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local vx, vy, vz = table.unpack(player:velocity())

    -- Only modify velocity while the player has horizontal movement.
    if math.abs(vx) > 0.01 or math.abs(vz) > 0.01 then
        local scale = 1.08
        player:set_velocity(vx * scale, vy, vz * scale)
    end
end

anemoia.register(module)
