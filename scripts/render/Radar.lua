local module = {
    name = "Radar",
    description = "Shows nearby entities on a 2D radar",
    category = "Render",
    enabled = false,
    settings = {
        size = 150,
        range = 100,
        opacity = 150,
    },
    _settings_meta = {
        size    = { min = 50,  max = 500 },
        range   = { min = 10,  max = 250 },
        opacity = { min = 0,   max = 255 },
    },
    _cache = { px = 0, pz = 0, yaw = 0, entities = {} },
}

-- Populate cache on the tick thread (all JNI here; none in on_render).
function module:on_tick()
    self._cache = { px = 0, pz = 0, yaw = 0, entities = {} }
    if not self.enabled then return end

    local player = mc.player()
    if not player then return end

    local ok, px, pz, yaw = pcall(function()
        return player:x(), player:z(), player:yaw()
    end)
    if not ok then return end

    self._cache.px       = px
    self._cache.pz       = pz
    self._cache.yaw      = yaw
    self._cache.entities = mc.entities()
end

anemoia.on_render(function(painter)
    if not module.enabled then return end
    local cache = module._cache
    if #cache.entities == 0 and cache.px == 0 then return end

    local size    = module.settings.size
    local range   = module.settings.range
    local opacity = module.settings.opacity
    local px, pz  = cache.px, cache.pz
    local yaw     = cache.yaw

    local rx, ry = 10, 10
    painter:rect(rx, ry, size, size, 0, 0, 0, opacity, 5)
    painter:rect_outline(rx, ry, size, size, 255, 255, 255, 200, 1, 5)

    local cx, cy = rx + size / 2, ry + size / 2
    painter:rect(cx - 2, cy - 2, 4, 4, 255, 255, 255, 255, 0)

    -- Entity blips (entity:* methods are pure Rust snapshots — no JNI)
    local angle = math.rad(yaw)
    local cos_a = math.cos(angle)
    local sin_a = math.sin(angle)
    local scale = (size / 2) / range
    local half  = size / 2

    for _, entity in ipairs(cache.entities) do
        if entity:alive() and not entity:is_local_player() then
            local dx = entity:x() - px
            local dz = entity:z() - pz

            local nx = (dz * sin_a + dx * cos_a) * scale
            local ny = (dz * cos_a - dx * sin_a) * scale

            if math.abs(nx) < half and math.abs(ny) < half then
                local r, g, b = 0, 255, 0
                if entity:type_id():find("player") then r, g, b = 255, 0, 0 end
                painter:rect(cx + nx - 2, cy + ny - 2, 4, 4, r, g, b, 255, 0)
            end
        end
    end
end)

anemoia.register(module)
