local module = {
    name = "AntiBot",
    description = "Filters fake entities and bot players from combat modules",
    category = "Misc",
    enabled = false,
    settings = {
        check_name       = true,
        check_alive      = true,
        players_only     = true,
        check_health     = true,
        min_health       = 0.5,
    },
    _settings_meta = {
        min_health = { min = 0, max = 20 },
    },
}

-- Exposed globally so KillAura, SilentAura, etc. can call antibot_module:is_bot(entity)
_G["antibot_module"] = module

function module:is_bot(entity)
    local ok_alive, alive = pcall(function() return entity:alive() end)
    if self.settings.check_alive and (not ok_alive or not alive) then
        return true
    end

    local ok_name, name = pcall(function() return entity:name() end)
    if self.settings.check_name and (not ok_name or name == "" or name == nil) then
        return true
    end

    local ok_tid, tid = pcall(function() return entity:type_id() end)
    if self.settings.players_only then
        if not ok_tid or not tid:find("player") then
            return true
        end
    end

    if self.settings.check_health then
        local ok_hp, hp = pcall(function() return entity:health() end)
        if ok_hp and hp < self.settings.min_health then
            return true
        end
    end

    return false
end

-- Passive filter — no per-tick logic needed
function module:on_tick() end

anemoia.register(module)
