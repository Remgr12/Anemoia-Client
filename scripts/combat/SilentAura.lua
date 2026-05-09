local module = {
    name = "SilentAura",
    description = "Attacks entities without changing visible client-side rotation",
    category = "Combat",
    enabled = false,
    settings = {
        range   = 4.0,
        cps     = 10,
        fov     = 360.0,
        targets = "Players",
    },
    _settings_meta = {
        range   = { min = 3.0, max = 6.0 },
        cps     = { min = 1, max = 20 },
        fov     = { min = 30.0, max = 360.0 },
        targets = { type = "enum", options = { "Players", "Mobs", "All" } },
    },
    _last_attack = 0,
}

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
    if now - self._last_attack < (1000 / self.settings.cps) then return end

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
        local valid = (filter == "All")
            or (filter == "Players" and ok_tid and tid:find("player"))
            or (filter == "Mobs"    and ok_tid and not tid:find("player"))
        if not valid then goto continue end

        if antibot and antibot.enabled and antibot:is_bot(entity) then goto continue end

        local ok_d, dsq = pcall(function() return entity:dist_sq(px, py - 1.62, pz) end)
        if not ok_d or dsq >= best_dsq then goto continue end

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

    -- Criticals gate (HitSelect mode)
    local crits = _G["criticals_module"]
    if crits and crits.enabled and not crits:can_attack(player) then return end

    -- Calculate exact rotation to target
    local ok_bt, tx, ty, tz = pcall(function()
        return best:x(), best:y() + 1.0, best:z()
    end)
    if not ok_bt then return end

    local dx    = tx - px
    local dy    = ty - py
    local dz    = tz - pz
    local hdist = math.sqrt(dx*dx + dz*dz)
    local tyaw  = math.deg(math.atan2(dz, dx)) - 90
    while tyaw < -180 do tyaw = tyaw + 360 end
    while tyaw >  180 do tyaw = tyaw - 360 end
    local tpitch = -math.deg(math.atan2(dy, hdist))

    -- Silent: send posrot packet (server sees this yaw/pitch), don't touch client camera
    local ok_pkt, spoof = pcall(anemoia.create_posrot_packet,
        player:x(), player:y(), player:z(), tyaw, tpitch,
        player:on_ground())
    if ok_pkt and spoof then pcall(mc.send_packet, spoof) end

    -- Criticals preparation (Packet/Jump modes)
    if crits and crits.enabled and crits.settings.mode ~= "HitSelect" then
        crits:prepare(player)
    end

    pcall(function() mc.attack(best) end)

    -- Restore original rotation on server via another posrot packet
    local ok_rst, restore = pcall(anemoia.create_posrot_packet,
        player:x(), player:y(), player:z(), cur_yaw, cur_pitch,
        player:on_ground())
    if ok_rst and restore then pcall(mc.send_packet, restore) end

    self._last_attack = now
end

anemoia.register(module)
