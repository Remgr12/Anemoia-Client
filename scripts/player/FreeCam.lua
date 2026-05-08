local module = {
    name        = "FreeCam",
    description = "Detach camera from player; fly freely while position appears frozen server-side",
    category    = "Player",
    enabled     = false,
    settings = {
        speed = 0.15,
    },
    _settings_meta = {
        speed = { min = 0.02, max = 1.0 },
    },
    _ox = 0, _oy = 0, _oz = 0,
    _cx = 0, _cy = 0, _cz = 0,
    _active = false,
    _last_t = 0,
}

local _mod = module
local _cb_registered = false

local function _packet_cb(packet)
    if not _mod._active then return false end
    local name = packet:type_name()
    if name:find("ServerboundMovePlayerPacket") then
        local ok, spoof = pcall(anemoia.create_position_packet,
            _mod._ox, _mod._oy, _mod._oz, true)
        if ok and spoof then pcall(mc.send_packet, spoof) end
        return true
    end
    return false
end

function module:on_enable()
    local player = mc.player()
    if not player then self.enabled = false; return end

    local ok, x, y, z = pcall(function() return player:x(), player:y(), player:z() end)
    if not ok then self.enabled = false; return end

    self._ox = x; self._oy = y; self._oz = z
    self._cx = x; self._cy = y; self._cz = z
    self._active = true
    self._last_t = os.clock()

    pcall(function() player:set_no_physics(true) end)
    pcall(function() player:set_velocity(0, 0, 0) end)

    if not _cb_registered then
        anemoia.on_packet_send(_packet_cb)
        _cb_registered = true
    end
end

function module:on_disable()
    self._active = false
    local player = mc.player()
    if not player then return end
    pcall(function() player:set_no_physics(false) end)
    pcall(function() player:set_pos(self._ox, self._oy, self._oz) end)
    pcall(function() player:set_velocity(0, 0, 0) end)
end

function module:on_tick()
    if not self._active then return end

    -- Rate limit to ~30fps: enough for smooth movement, avoids JNI overload
    local t = os.clock()
    if t - self._last_t < 0.033 then return end
    local dt = math.min(t - self._last_t, 0.1)  -- cap dt to avoid large jumps
    self._last_t = t

    local player = mc.player()
    if not player then return end

    local ok_rot, yaw, pitch = pcall(function()
        return player:yaw(), player:pitch()
    end)
    if not ok_rot then return end

    -- Speed is in blocks/second
    local spd = self.settings.speed * 60 * dt

    local yr = math.rad(yaw)
    local pr = math.rad(pitch)
    local fx = -math.sin(yr) * math.cos(pr)
    local fy = -math.sin(pr)
    local fz =  math.cos(yr) * math.cos(pr)
    local rx =  math.cos(yr)
    local rz =  math.sin(yr)

    local dx, dy, dz = 0, 0, 0
    if mc.is_key_down(87)  then dx=dx+fx*spd; dy=dy+fy*spd; dz=dz+fz*spd end
    if mc.is_key_down(83)  then dx=dx-fx*spd; dy=dy-fy*spd; dz=dz-fz*spd end
    if mc.is_key_down(68)  then dx=dx+rx*spd; dz=dz+rz*spd end
    if mc.is_key_down(65)  then dx=dx-rx*spd; dz=dz-rz*spd end
    if mc.is_key_down(32)  then dy=dy+spd end
    if mc.is_key_down(340) then dy=dy-spd end

    self._cx = self._cx + dx
    self._cy = self._cy + dy
    self._cz = self._cz + dz

    pcall(function()
        player:set_pos(self._cx, self._cy, self._cz)
        player:set_velocity(0, 0, 0)
    end)
end

anemoia.register(module)
