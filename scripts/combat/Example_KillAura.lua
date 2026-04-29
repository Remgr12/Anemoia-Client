-- KillAura: attacks the nearest living entity each tick.
-- Excludes the local player.  Toggle with Right Ctrl by default.

local module = {
    name    = "KillAura",
    description = "Attacks the nearest entity within range",
    category = "Combat",
    key     = 345,   -- Right Ctrl (GLFW_KEY_RIGHT_CTRL)
    enabled = false,
    range   = 4.5,
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local px, py, pz = player:x(), player:y(), player:z()

    local entities = mc.entities()
    local best_dist = self.range * self.range  -- compare squared to avoid sqrt
    local best = nil

    for _, entity in ipairs(entities) do
        if entity:alive() and not entity:is_local_player() then
            local dsq = entity:dist_sq(px, py, pz)
            if dsq < best_dist then
                best_dist = dsq
                best = entity
            end
        end
    end

    if best then
        mc.attack(best)
    end
end

anemoia.register(module)
