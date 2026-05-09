local module = {
    name = "FastPlace",
    description = "Removes right-click delay",
    category = "World",
    enabled = false,
}

function module:on_tick()
    pcall(function() mc.set_right_click_delay(0) end)
end

function module:on_disable()
    pcall(function() mc.set_right_click_delay(4) end)
end

anemoia.register(module)
