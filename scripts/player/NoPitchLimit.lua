local module = {
    name = "NoPitchLimit",
    description = "Allows you to look beyond 90 degrees up/down",
    category = "Player",
    enabled = false,
}

function module:on_tick()
    -- This module might actually need to hook the camera rotation logic
    -- but setting pitch directly can also work if the game doesn't clamp it every frame.
end

anemoia.register(module)
