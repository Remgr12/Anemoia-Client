local module = {
    name = "FastPlace",
    description = "Removes right-click delay",
    category = "World",
    enabled = false,
}

function module:on_tick()
    mc.set_right_click_delay(0)
end

function module:on_disable()
    mc.set_right_click_delay(4)
end

anemoia.register(module)
