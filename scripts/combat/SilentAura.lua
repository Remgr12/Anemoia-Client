local module = {
    name = "SilentAura",
    description = "Attacks entities without changing your client-side rotation",
    category = "Combat",
    enabled = false,
    settings = {
        range = 4.0,
        cps = 10,
        targets = "Players",
    },
    _settings_meta = {
        range = { min = 3.0, max = 6.0 },
        cps = { min = 1, max = 20 },
        targets = { type = "enum", options = { "Players", "Mobs", "All" } }
    },
    last_attack = 0
}

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local now = os.clock() * 1000
    if now - self.last_attack < (1000 / self.settings.cps) then
        return
    end

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
            elseif filter == "Mobs" and not tid:find("player") then
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
        local old_yaw = player:yaw()
        local old_pitch = player:pitch()

        pcall(function()
            self:rotate(player, px, py, pz, best_target)
            mc.attack(best_target)
        end)

        -- Always restore — even if rotate or attack errored
        player:set_yaw(old_yaw)
        player:set_pitch(old_pitch)

        self.last_attack = now
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
    while yaw < -180 do yaw = yaw + 360 end
    while yaw > 180 do yaw = yaw - 360 end
    player:set_yaw(yaw)
    player:set_pitch(pitch)
end

anemoia.register(module)
