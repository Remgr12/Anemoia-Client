local module = {
    name = "KillAura",
    description = "Automatically attacks entities around you",
    category = "Combat",
    enabled = false,
    settings = {
        range          = 4.0,
        cps            = 10,
        rotations      = true,
        rotation_speed = 45.0,
        targets        = "Players",
    },
    _settings_meta = {
        range          = { min = 3.0, max = 6.0 },
        cps            = { min = 1, max = 20 },
        rotation_speed = { min = 5.0, max = 180.0 },
        targets        = { type = "enum", options = { "Players", "Mobs", "All" } }
    },
    _last_attack = 0,
    _cur_yaw     = 0.0,
    _cur_pitch   = 0.0,
}

local MOB_IDS = {
    "zombie", "skeleton", "creeper", "spider",
    "enderman", "witch", "pillager", "blaze", "ghast",
}

local function is_valid_target(filter, type_id)
    if filter == "All" then return true end
    if filter == "Players" then return type_id:find("player") ~= nil end
    if filter == "Mobs" then
        for _, mob in ipairs(MOB_IDS) do
            if type_id:find(mob) then return true end
        end
    end
    return false
end

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

local function calc_rotations(px, py, pz, target)
    local ok, tx, ty, tz = pcall(function()
        return target:x(), target:y() + 1.0, target:z()
    end)
    if not ok then return nil, nil end

    local dx    = tx - px
    local dy    = ty - py
    local dz    = tz - pz
    local hdist = math.sqrt(dx * dx + dz * dz)

    local yaw   = normalize_yaw(math.deg(math.atan2(-dx, dz)))
    local pitch = math.max(-90, math.min(90, -math.deg(math.atan2(dy, hdist))))
    return yaw, pitch
end

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local now      = os.clock() * 1000
    local interval = 1000.0 / self.settings.cps
    if now - self._last_attack < interval then return end

    local ok_pos, px, py, pz = pcall(function()
        return player:x(), player:y() + 1.62, player:z()
    end)
    if not ok_pos then return end

    local entities   = mc.entities()
    local best       = nil
    local best_dsq   = self.settings.range * self.settings.range
    local filter     = self.settings.targets

    for _, entity in ipairs(entities) do
        local ok_basic, alive, is_local = pcall(function()
            return entity:alive(), entity:is_local_player()
        end)
        if not ok_basic or not alive or is_local then goto continue end

        local ok_tid, tid = pcall(function() return entity:type_id() end)
        if not ok_tid or not is_valid_target(filter, tid) then goto continue end

        local ok_d, dsq = pcall(function()
            return entity:dist_sq(px, py - 1.62, pz)
        end)
        if ok_d and dsq < best_dsq then
            best_dsq = dsq
            best     = entity
        end

        ::continue::
    end

    if not best then return end

    if self.settings.rotations then
        local ok_cur, cur_yaw, cur_pitch = pcall(function()
            return player:yaw(), player:pitch()
        end)
        if ok_cur then
            local tyaw, tpitch = calc_rotations(px, py, pz, best)
            if tyaw and tpitch then
                local speed = self.settings.rotation_speed
                self._cur_yaw   = smooth_angle(cur_yaw,   tyaw,   speed)
                self._cur_pitch = smooth_angle(cur_pitch, tpitch, speed * 0.75)
                pcall(function()
                    player:set_yaw(self._cur_yaw)
                    player:set_pitch(self._cur_pitch)
                end)
            end
        end
    end

    local ok_atk = pcall(function() mc.attack(best) end)
    if ok_atk then
        self._last_attack = now
    end
end

anemoia.register(module)
