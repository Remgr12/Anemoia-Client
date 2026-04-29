local module = {
    name = "Tracers",
    description = "Draws lines to entities",
    category = "Render",
    enabled = false,
}

-- Without world_to_screen, we can't do true tracers.
-- But we can add it to the API!

anemoia.on_render(function(painter)
    if not module.enabled then return end
    -- Placeholder until world_to_screen is added
end)

anemoia.register(module)
