local module = {
    name = "Aimbot",
    description = "Automatically looks at entities",
    category = "Combat",
    enabled = false,
    settings = {
        range         = 4.0,
        speed         = 30.0,
        targets       = "Players",
    },
    _settings_meta = {
        range   = { min = 3.0, max = 10.0 },
        speed   = { min = 5.0, max = 180.0 },
        targets = { type = "enum", options = { "Players", "Mobs", "All" } },
    },
    _cur_yaw   = 0.0,
    _cur_pitch = 0.0,
}

local function smooth_angle(current, target, speed)
    local diff = target - current
    while diff >  180 do diff = diff - 360 end
    while diff < -180 do diff = diff + 360 end
    if math.abs(diff) <= speed then return target end
    return current + (diff > 0 and speed or -speed)
end

local function normalize_yaw(y)
    while y >  180 do y = y - 360 end
    while y < -180 do y = y + 360 end
    return y
end

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local ok_pos, px, py, pz = pcall(function()
        return player:x(), player:y() + 1.62, player:z()
    end)
    if not ok_pos then return end

    local ok_rot, cur_yaw, cur_pitch = pcall(function()
        return player:yaw(), player:pitch()
    end)
    if not ok_rot then return end

    local entities = mc.entities()
    local best = nil
    local best_dsq = self.settings.range * self.settings.range
    local filter = self.settings.targets

    for _, entity in ipairs(entities) do
        if entity:alive() and not entity:is_local_player() and entity:health() > 0 then
            local tid = entity:type_id()
            local valid = false
            if filter == "All" then
                valid = true
            elseif filter == "Players" and tid:find("player") then
                valid = true
            elseif filter == "Mobs" and not tid:find("player") then
                valid = true
            end
            if valid then
                local dsq = entity:dist_sq(px, py - 1.62, pz)
                if dsq < best_dsq then
                    best_dsq = dsq
                    best = entity
                end
            end
        end
    end

    if not best then return end

    local tx = best:x()
    local ty = best:y() + 1.0
    local tz = best:z()
    local dx = tx - px
    local dy = ty - py
    local dz = tz - pz
    local hdist = math.sqrt(dx * dx + dz * dz)

    local tyaw   = normalize_yaw(math.deg(math.atan2(-dx, dz)))
    local tpitch = math.max(-90, math.min(90, -math.deg(math.atan2(dy, hdist))))

    local speed = self.settings.speed
    self._cur_yaw   = smooth_angle(cur_yaw,   tyaw,   speed)
    self._cur_pitch = smooth_angle(cur_pitch, tpitch, speed * 0.75)

    pcall(function()
        player:set_yaw(self._cur_yaw)
        player:set_pitch(self._cur_pitch)
    end)
end

anemoia.register(module)
