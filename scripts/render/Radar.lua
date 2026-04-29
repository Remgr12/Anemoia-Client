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
        size = { min = 50, max = 500 },
        range = { min = 10, max = 250 },
        opacity = { min = 0, max = 255 }
    }
}

anemoia.on_render(function(painter)
    if not module.enabled then return end

    local player = mc.player()
    if not player then return end

    local px, py, pz = player:x(), player:y(), player:z()
    local yaw = player:yaw()
    
    local size = module.settings.size
    local range = module.settings.range
    local opacity = module.settings.opacity
    
    -- Radar background (top right)
    local rx, ry = 10, 10
    painter:rect(rx, ry, size, size, 0, 0, 0, opacity, 5)
    painter:rect_outline(rx, ry, size, size, 255, 255, 255, 200, 1, 5)
    
    -- Center (Player)
    local cx, cy = rx + size/2, ry + size/2
    painter:rect(cx - 2, cy - 2, 4, 4, 255, 255, 255, 255, 0)

    local entities = mc.entities()
    for _, entity in ipairs(entities) do
        if entity:alive() and not entity:is_local_player() then
            local ex, ey, ez = entity:x(), entity:y(), entity:z()
            
            local dx = ex - px
            local dz = ez - pz
            
            -- Rotate according to player yaw
            local angle = math.rad(yaw)
            local cos = math.cos(angle)
            local sin = math.sin(angle)
            
            local nx = dz * sin + dx * cos
            local ny = dz * cos - dx * sin
            
            -- Scale to radar size
            local scale = (size / 2) / range
            nx = nx * scale
            ny = ny * scale
            
            -- Clamp to radar bounds
            if math.abs(nx) < size/2 and math.abs(ny) < size/2 then
                local tx, ty = cx + nx, cy + ny
                
                local r, g, b = 0, 255, 0
                if entity:type_id():find("player") then
                    r, g, b = 255, 0, 0
                end
                
                painter:rect(tx - 2, ty - 2, 4, 4, r, g, b, 255, 0)
            end
        end
    end
end)

anemoia.register(module)
