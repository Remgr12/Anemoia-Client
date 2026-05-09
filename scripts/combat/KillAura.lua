local module = {
    name = "KillAura",
    description = "Automatically attacks entities within range",
    category = "Combat",
    enabled = false,
    settings = {
        range          = 4.0,
        cps            = 10,
        cps_randomize  = true,
        rotations      = true,
        rotation_speed = 45.0,
        fov            = 360.0,  -- degrees; 360 = disabled
        targets        = "Players",
        miss_chance    = 0,      -- % chance to deliberately skip an attack
        weapon_only    = false,  -- require sword or axe in main hand
    },
    _settings_meta = {
        range          = { min = 3.0, max = 6.0 },
        cps            = { min = 1, max = 20 },
        rotation_speed = { min = 5.0, max = 180.0 },
        fov            = { min = 30.0, max = 360.0 },
        miss_chance    = { min = 0, max = 50 },
        targets        = { type = "enum", options = { "Players", "Mobs", "All" } },
    },
    _last_attack   = 0,
    _next_interval = 100,
    _cur_yaw       = 0.0,
    _cur_pitch     = 0.0,
}

local function is_valid_target(filter, type_id)
    if filter == "All"     then return true end
    if filter == "Players" then return type_id:find("player") ~= nil end
    if filter == "Mobs"    then return type_id:find("player") == nil end
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

-- Returns the angle (degrees) between where the player looks and the direction to target
local function fov_angle(px, py, pz, yaw, pitch, tx, ty, tz)
    local ry = math.rad(yaw)
    local rp = math.rad(pitch)
    local lx = -math.sin(ry) * math.cos(rp)
    local ly = -math.sin(rp)
    local lz =  math.cos(ry) * math.cos(rp)
    local dx, dy, dz = tx - px, ty - py, tz - pz
    local dist = math.sqrt(dx*dx + dy*dy + dz*dz)
    if dist < 0.0001 then return 0 end
    local dot = (lx*dx + ly*dy + lz*dz) / dist
    return math.deg(math.acos(math.max(-1, math.min(1, dot))))
end

function module:on_tick()
    local player = mc.player()
    if not player then return end

    local now = os.clock() * 1000
    if now < self._last_attack + self._next_interval then return end

    -- Weapon gate
    if self.settings.weapon_only then
        local ok, item = pcall(function() return player:main_hand_item() end)
        if ok and item then
            local ok2, tid = pcall(function() return item:type_id() end)
            if ok2 and not (tid:find("sword") or tid:find("axe")) then return end
        end
    end

    local ok_pos, px, py, pz = pcall(function()
        return player:x(), player:y() + 1.62, player:z()
    end)
    if not ok_pos then return end

    local ok_rot, cur_yaw, cur_pitch = pcall(function()
        return player:yaw(), player:pitch()
    end)
    if not ok_rot then return end

    local best, best_dsq = nil, self.settings.range * self.settings.range
    local half_fov = self.settings.fov * 0.5
    local filter   = self.settings.targets
    local antibot  = _G["antibot_module"]

    for _, entity in ipairs(mc.entities()) do
        local ok_b, alive, is_local = pcall(function()
            return entity:alive(), entity:is_local_player()
        end)
        if not ok_b or not alive or is_local then goto continue end

        local ok_tid, tid = pcall(function() return entity:type_id() end)
        if not ok_tid or not is_valid_target(filter, tid) then goto continue end

        if antibot and antibot.enabled and antibot:is_bot(entity) then goto continue end

        local ok_d, dsq = pcall(function() return entity:dist_sq(px, py - 1.62, pz) end)
        if not ok_d or dsq >= best_dsq then goto continue end

        -- FOV filter
        if half_fov < 180 then
            local ok_ep, tx, ty, tz = pcall(function()
                return entity:x(), entity:y() + 1.0, entity:z()
            end)
            if ok_ep then
                local angle = fov_angle(px, py, pz, cur_yaw, cur_pitch, tx, ty, tz)
                if angle > half_fov then goto continue end
            end
        end

        best_dsq = dsq
        best     = entity
        ::continue::
    end

    if not best then return end

    -- HitSelect gate
    local hs = _G["hitselect_module"]
    if hs and hs.enabled and not hs:should_attack(player) then return end

    -- Miss chance — burn the interval without attacking
    if self.settings.miss_chance > 0 and math.random(100) <= self.settings.miss_chance then
        self._last_attack   = now
        self._next_interval = (1000.0 / self.settings.cps) * (self.settings.cps_randomize and (0.8 + math.random() * 0.4) or 1.0)
        return
    end

    -- Rotations — skip if Aimbot is enabled (it owns the camera)
    local aimbot = _G["aimbot_module"]
    if self.settings.rotations and not (aimbot and aimbot.enabled) then
        local ok_bt, tx, ty, tz = pcall(function()
            return best:x(), best:y() + 1.0, best:z()
        end)
        if ok_bt then
            local dx    = tx - px
            local dy    = ty - py
            local dz    = tz - pz
            local hdist = math.sqrt(dx*dx + dz*dz)
            local tyaw   = normalize_yaw(math.deg(math.atan2(-dx, dz)))
            local tpitch = math.max(-90, math.min(90, -math.deg(math.atan2(dy, hdist))))
            local speed  = self.settings.rotation_speed
            self._cur_yaw   = smooth_angle(cur_yaw,   tyaw,   speed)
            self._cur_pitch = smooth_angle(cur_pitch, tpitch, speed * 0.75)
            pcall(function()
                player:set_yaw(self._cur_yaw)
                player:set_pitch(self._cur_pitch)
            end)
        end
    end

    -- Criticals preparation (Packet/Jump modes)
    local crits = _G["criticals_module"]
    if crits and crits.enabled and crits.settings.mode ~= "HitSelect" then
        crits:prepare(player)
    end

    local ok_atk = pcall(function() mc.attack(best) end)
    if ok_atk then
        self._last_attack   = now
        self._next_interval = (1000.0 / self.settings.cps) * (self.settings.cps_randomize and (0.8 + math.random() * 0.4) or 1.0)
    end
end

anemoia.register(module)
