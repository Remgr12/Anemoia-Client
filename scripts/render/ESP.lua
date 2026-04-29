local module = {
    name = "ESP",
    description = "Draws entity names on screen",
    category = "Render",
    enabled = false,
}

anemoia.on_render(function(painter)
    if not module.enabled then return end
    
    local player = mc.player()
    if not player then return end
    
    local entities = mc.entities()
    local y_offset = 50
    
    painter:text(10, y_offset, "Entities:", 255, 255, 255, 255, 18)
    y_offset = y_offset + 20

    for _, entity in ipairs(entities) do
        if not entity:is_local_player() and entity:alive() then
            local dist = math.sqrt(entity:dist_sq(player:x(), player:y(), player:z()))
            local info = string.format("%s [%.1fm]", entity:name(), dist)
            
            painter:text(10, y_offset, info, 200, 255, 200, 255, 15)
            y_offset = y_offset + 18
        end
    end
end)

anemoia.register(module)
