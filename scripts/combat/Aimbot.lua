local module = {
    name = "Aimbot",
    description = "Smoothly tracks entities; Adaptive mode scales speed with distance",
    category = "Combat",
    enabled = false,
    settings = {
        range   = 4.0,
        speed   = 30.0,
        mode    = "Simple",      -- Simple (fixed speed) | Adaptive (scales with distance)
        fov     = 180.0,         -- max degrees from crosshair to begin tracking
        target  = "Body",        -- aim point: Body | Head | Feet | Random
        targets = "Players",
    },
    _settings_meta = {
        range   = { min = 3.0, max = 10.0 },
        speed   = { min = 5.0, max = 180.0 },
        fov     = { min = 10.0, max = 180.0 },
        mode    = { type = "enum", options = { "Simple", "Adaptive" } },
        target  = { type = "enum", options = { "Body", "Head", "Feet", "Random" } },
        targets = { type = "enum", options = { "Players", "Mobs", "All" } },
    },
    _cur_yaw   = 0.0,
    _cur_pitch = 0.0,
    _rand_off  = 0.0,  -- random vertical offset refreshed per target switch
    _last_id   = -1,
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

local function fov_angle(lx, ly, lz, dx, dy, dz, dist)
    if dist < 0.0001 then return 0 end
    local dot = (lx*dx + ly*dy + lz*dz) / dist
    return math.deg(math.acos(math.max(-1, math.min(1, dot))))
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

    -- Build look vector for FOV filtering
    local ry = math.rad(cur_yaw)
    local rp = math.rad(cur_pitch)
    local lx = -math.sin(ry) * math.cos(rp)
    local ly = -math.sin(rp)
    local lz =  math.cos(ry) * math.cos(rp)

    local best, best_dsq = nil, self.settings.range * self.settings.range
    local filter = self.settings.targets

    for _, entity in ipairs(mc.entities()) do
        if not entity:alive() or entity:is_local_player() or entity:health() <= 0 then goto c end
        local ok_tid, tid = pcall(function() return entity:type_id() end)
        local valid = (filter == "All")
            or (filter == "Players" and ok_tid and tid:find("player"))
            or (filter == "Mobs"    and ok_tid and not tid:find("player"))
        if not valid then goto c end

        local ok_d, dsq = pcall(function() return entity:dist_sq(px, py - 1.62, pz) end)
        if not ok_d or dsq >= best_dsq then goto c end

        -- FOV check
        local ok_ep, tx, ty, tz = pcall(function() return entity:x(), entity:y() + 1.0, entity:z() end)
        if ok_ep then
            local dx, dy2, dz2 = tx - px, ty - py, tz - pz
            local dist = math.sqrt(dx*dx + dy2*dy2 + dz2*dz2)
            if fov_angle(lx, ly, lz, dx, dy2, dz2, dist) > self.settings.fov then goto c end
        end

        best_dsq = dsq
        best = entity
        ::c::
    end

    if not best then return end

    -- Refresh random offset on target switch
    local bid = best:id()
    if bid ~= self._last_id then
        self._last_id   = bid
        self._rand_off  = math.random() * 1.7
    end

    -- Target aim point
    local aim_off = 1.0  -- Body
    local tmode = self.settings.target
    if     tmode == "Head"   then aim_off = 1.8
    elseif tmode == "Feet"   then aim_off = 0.1
    elseif tmode == "Random" then aim_off = 0.1 + self._rand_off
    end

    local tx = best:x()
    local ty = best:y() + aim_off
    local tz = best:z()
    local dx    = tx - px
    local dy    = ty - py
    local dz    = tz - pz
    local hdist = math.sqrt(dx*dx + dz*dz)

    local tyaw   = normalize_yaw(math.deg(math.atan2(-dx, dz)))
    local tpitch = math.max(-90, math.min(90, -math.deg(math.atan2(dy, hdist))))

    -- Adaptive: speed ∝ 1/distance (closer = faster snap, far = slower)
    local speed = self.settings.speed
    if self.settings.mode == "Adaptive" then
        local dist = math.sqrt(best_dsq)
        speed = speed * (4.0 / math.max(dist, 0.5))
        speed = math.min(speed, 180.0)
    end

    self._cur_yaw   = smooth_angle(cur_yaw,   tyaw,   speed)
    self._cur_pitch = smooth_angle(cur_pitch, tpitch, speed * 0.75)

    pcall(function()
        player:set_yaw(self._cur_yaw)
        player:set_pitch(self._cur_pitch)
    end)
end

_G["aimbot_module"] = module
anemoia.register(module)
