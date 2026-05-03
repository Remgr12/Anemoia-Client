-- Cache populated on client thread (on_tick); render thread reads cache only.
local module = {
    name        = "ESP",
    description = "Draws entity names on screen",
    category    = "Render",
    enabled     = false,
    _cache      = {},
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

    local entities = mc.entities()
    for _, entity in ipairs(entities) do
        local ok, is_local, alive = pcall(function()
            return entity:is_local_player(), entity:alive()
        end)
        if not ok or is_local or not alive then goto continue end

        local ok_info, name, dsq, type_id = pcall(function()
            return entity:name(), entity:dist_sq(px, py, pz), entity:type_id()
        end)
        if not ok_info then goto continue end

        local dist  = math.sqrt(dsq)
        local r, g, b = 200, 255, 200
        if type_id:find("player") then
            r, g, b = 255, 100, 100
        end

        self._cache[#self._cache + 1] = {
            label = string.format("%s [%.1fm]", name, dist),
            r = r, g = g, b = b,
        }

        ::continue::
    end
end

anemoia.on_render(function(painter)
    if not module.enabled then return end
    local cache = module._cache
    if #cache == 0 then return end

    local y = 50
    painter:text(10, y, "Entities:", 255, 255, 255, 255, 18)
    y = y + 20

    for _, entry in ipairs(cache) do
        painter:text(10, y, entry.label, entry.r, entry.g, entry.b, 255, 15)
        y = y + 18
    end
end)

anemoia.register(module)
