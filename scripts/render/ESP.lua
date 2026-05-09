local module = {
    name        = "ESP",
    description = "Draws entity info and health on screen",
    category    = "Render",
    enabled     = false,
    settings = {
        show_health  = true,
        show_dist    = true,
        players_only = false,
        max_dist     = 64.0,
    },
    _settings_meta = {
        max_dist = { min = 10.0, max = 128.0 },
    },
    _cache = {},
}

function module:on_tick()
    self._cache = {}
    if not self.enabled then return end

    local player = mc.player()
    if not player then return end

    local ok_pos, px, py, pz = pcall(function()
        return player:x(), player:y(), player:z()
    end)
    if not ok_pos then return end

    local max_dsq = self.settings.max_dist * self.settings.max_dist

    for _, entity in ipairs(mc.entities()) do
        local ok, is_local, alive = pcall(function()
            return entity:is_local_player(), entity:alive()
        end)
        if not ok or is_local or not alive then goto continue end

        local ok_info, name, dsq, type_id, hp = pcall(function()
            return entity:name(), entity:dist_sq(px, py, pz), entity:type_id(), entity:health()
        end)
        if not ok_info or dsq > max_dsq then goto continue end

        local is_player = type_id:find("player") ~= nil
        if self.settings.players_only and not is_player then goto continue end

        local dist = math.sqrt(dsq)

        -- Health-based colour for players: green (full) → yellow → red (low)
        local r, g, b = 150, 255, 150
        if is_player then
            local frac = math.max(0, math.min(1, hp / 20.0))
            r = math.floor(255 * (1 - frac))
            g = math.floor(255 * frac)
            b = 50
        end

        local label = name or type_id
        if self.settings.show_health then
            label = string.format("%s %.1f hp", label, hp)
        end
        if self.settings.show_dist then
            label = string.format("%s [%.1fm]", label, dist)
        end

        self._cache[#self._cache + 1] = { label = label, health = hp, r = r, g = g, b = b }
        ::continue::
    end

    -- Most damaged targets first (highest priority)
    table.sort(self._cache, function(a, b) return a.health < b.health end)
end

anemoia.on_render(function(painter)
    if not module.enabled then return end
    local cache = module._cache
    if #cache == 0 then return end

    local y = 50
    painter:text(10, y, "Entities:", 255, 255, 255, 255, 18)
    y = y + 22

    for _, entry in ipairs(cache) do
        painter:text(10, y, entry.label, entry.r, entry.g, entry.b, 255, 15)
        y = y + 18
    end
end)

anemoia.register(module)
