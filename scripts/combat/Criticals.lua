local module = {
    name = "Criticals",
    description = "Forces critical hits by timing attacks or position packets",
    category = "Combat",
    enabled = false,
    settings = {
        mode = "Packet",
    },
    _settings_meta = {
        mode = { type = "enum", options = { "Packet", "Jump", "HitSelect" } }
    },
    _last_crit = 0,
}

-- Exposed so KillAura/SilentAura can call prepare() before each attack
_G["criticals_module"] = module

-- Called by combat modules just before mc.attack().
-- HitSelect mode: gate is handled by can_attack() instead.
function module:prepare(player)
    if not self.enabled then return end
    local mode = self.settings.mode

    if mode == "Packet" then
        local ok_g, on_ground = pcall(function() return player:on_ground() end)
        if not ok_g or not on_ground then return end
        local ok_pos, x, y, z = pcall(function() return player:x(), player:y(), player:z() end)
        if not ok_pos then return end
        local p1 = anemoia.create_position_packet(x, y + 0.0625, z, false)
        local p2 = anemoia.create_position_packet(x, y, z, false)
        pcall(mc.send_packet, p1)
        pcall(mc.send_packet, p2)

    elseif mode == "Jump" then
        local ok_g, on_ground = pcall(function() return player:on_ground() end)
        if ok_g and on_ground then
            pcall(function() player:jump() end)
        end
    end
end

-- HitSelect mode: returns false when vy >= 0 (not falling), blocking the attack.
-- Combat modules call this to gate attacks.
function module:can_attack(player)
    if not self.enabled then return true end
    if self.settings.mode ~= "HitSelect" then return true end

    local ok_vel, vel = pcall(function() return player:velocity() end)
    if not ok_vel then return true end
    local _, vy, _ = table.unpack(vel)
    return vy < 0
end

-- Standalone: Packet/Jump modes activate on LMB hold (player manually clicking)
function module:on_tick()
    if self.settings.mode == "HitSelect" then return end

    local player = mc.player()
    if not player then return end
    if not mc.is_key_down(0) then return end

    local now = os.clock() * 1000
    if now - self._last_crit < 100 then return end

    self:prepare(player)
    self._last_crit = now
end

anemoia.register(module)
