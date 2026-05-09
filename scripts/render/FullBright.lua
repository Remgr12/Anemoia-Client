local module = {
    name = "FullBright",
    description = "Makes everything bright",
    category = "Render",
    enabled = false,
}

function module:on_tick()
    pcall(function() mc.set_gamma(1000.0) end)
end

function module:on_disable()
    pcall(function() mc.set_gamma(1.0) end)
end

anemoia.register(module)
