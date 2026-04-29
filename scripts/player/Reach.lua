local module = {
    name = "Reach",
    description = "Increases your reach distance",
    category = "Player",
    enabled = false,
    settings = {
        distance = 4.5,
    },
    _settings_meta = {
        distance = { min = 3.0, max = 6.0 }
    }
}

-- Note: Real reach usually requires hooking the 'getPickRange' or similar.
-- With Lua only, we can try to force an attack if we are clicking.

function module:on_tick()
    local player = mc.player()
    if not player then return end

    if mc.is_key_down(0) then -- Left click
        local px, py, pz = player:x(), player:y() + 1.62, player:z()
        local entities = mc.entities()
        local range_sq = self.settings.distance * self.settings.distance
        
        for _, entity in ipairs(entities) do
            if entity:alive() and not entity:is_local_player() then
                if entity:dist_sq(px, py - 1.62, pz) < range_sq then
                    -- This might cause double attacks if not careful,
                    -- but demonstrates the intent.
                    mc.attack(entity)
                    break
                end
            end
        end
    end
end

anemoia.register(module)
