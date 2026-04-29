local module = {
    name = "Aimbot",
    description = "Automatically looks at entities",
    category = "Combat",
    enabled = false,
    settings = {
        range = 4.0,
        targets = "Players", -- Players, Mobs, All
    },
    _settings_meta = {
        range = { min = 3.0, max = 10.0 },
        targets = { type = "enum", options = { "Players", "Mobs", "All" } }
    }
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local px, py, pz = player:x(), player:y() + 1.62, player:z()
    local entities = mc.entities()
    
    local best_target = nil
    local best_dist = self.settings.range * self.settings.range

    for _, entity in ipairs(entities) do
        if entity:alive() and not entity:is_local_player() then
            local tid = entity:type_id()
            local valid = false
            
            local filter = self.settings.targets
            if filter == "All" then
                valid = true
            elseif filter == "Players" and tid:find("player") then
                valid = true
            elseif filter == "Mobs" and (tid:find("zombie") or tid:find("skeleton") or tid:find("creeper") or tid:find("spider")) then
                valid = true
            end

            if valid then
                local dsq = entity:dist_sq(px, py - 1.62, pz)
                if dsq < best_dist then
                    best_dist = dsq
                    best_target = entity
                end
            end
        end
    end

    if best_target then
        self:rotate(player, px, py, pz, best_target)
    end
end

function module:rotate(player, px, py, pz, target)
    local tx, ty, tz = target:x(), target:y() + 1.0, target:z()
    
    local dx = tx - px
    local dy = ty - py
    local dz = tz - pz
    local dist = math.sqrt(dx*dx + dz*dz)
    
    local yaw = math.deg(math.atan2(dz, dx)) - 90
    local pitch = -math.deg(math.atan2(dy, dist))
    
    -- Normalize yaw
    while yaw < -180 do yaw = yaw + 360 end
    while yaw > 180 do yaw = yaw - 360 end
    
    player:set_yaw(yaw)
    player:set_pitch(pitch)
end

anemoia.register(module)
