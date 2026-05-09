local module = {
    name        = "FreeCam",
    description = "Detach camera from player; fly freely while position appears frozen server-side",
    category    = "Player",
    enabled     = false,
    settings = {
        speed = 0.3,  -- blocks per second
    },
    _settings_meta = {
        speed = { min = 0.1, max = 20.0 },
    },
    _ox = 0, _oy = 0, _oz = 0,
    _cx = 0, _cy = 0, _cz = 0,
    _active    = false,
    _last_move = 0,
    _last_spoof= 0,
}

function module:on_enable()
    local player = mc.player()
    if not player then self.enabled = false; return end

    local ok, x, y, z = pcall(function() return player:x(), player:y(), player:z() end)
    if not ok then self.enabled = false; return end

    self._ox = x; self._oy = y; self._oz = z
    self._cx = x; self._cy = y; self._cz = z
    self._active     = true
    self._last_move  = mc.clock()
    self._last_spoof = mc.clock()

    pcall(function() player:set_no_physics(true) end)
    pcall(function() player:set_velocity(0, 0, 0) end)
    pcall(function() player:set_on_ground(true) end)

    -- Rust-level freeze: cancels ServerboundMovePlayerPacket without Lua lock,
    -- so camera-position packets can never slip through to the server.
    mc.freeze_pos()
end

function module:on_disable()
    self._active = false
    mc.unfreeze_pos()

    local player = mc.player()
    if not player then return end
    pcall(function() player:set_no_physics(false) end)
    pcall(function() player:set_pos(self._ox, self._oy, self._oz) end)
    pcall(function() player:set_velocity(0, 0, 0) end)
    pcall(function() player:set_on_ground(true) end)
end

function module:on_tick()
    if not self._active then return end

    local now = mc.clock()

    -- Camera movement at up to 60Hz
    local dt = now - self._last_move
    if dt < 0.016 then goto send_spoof end
    self._last_move = now
    dt = math.min(dt, 0.1)

    do
        local player = mc.player()
        if not player then goto send_spoof end

        local ok_rot, yaw, pitch = pcall(function()
            return player:yaw(), player:pitch()
        end)
        if not ok_rot then goto send_spoof end

        local spd = self.settings.speed * dt

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
            player:set_on_ground(true)
        end)
    end

    ::send_spoof::
    -- Send frozen position to server at 20 Hz to keep connection alive.
    -- The Rust interceptor already cancels the game's own movement packets,
    -- so these explicit packets are the server's only position updates.
    if now - self._last_spoof >= 0.05 then
        local ok, spoof = pcall(anemoia.create_position_packet,
            self._ox, self._oy, self._oz, true)
        if ok and spoof then pcall(mc.send_packet, spoof) end
        self._last_spoof = now
    end
end

anemoia.register(module)
