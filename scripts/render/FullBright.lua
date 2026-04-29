local module = {
    name = "FullBright",
    description = "Makes everything bright",
    category = "Render",
    enabled = false,
}

function module:on_tick()
    mc.set_gamma(1000.0)
end

function module:on_disable()
    mc.set_gamma(1.0)
end

anemoia.register(module)
