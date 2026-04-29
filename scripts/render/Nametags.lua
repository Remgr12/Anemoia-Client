local module = {
    name = "Nametags",
    description = "Shows information about entities above their heads",
    category = "Render",
    enabled = false,
}

anemoia.on_render(function(painter)
    if not module.enabled then return end
    
    -- This would also use world_to_screen.
    -- For now, we will add it to the radar or skip true 3D nametags.
end)

anemoia.register(module)
