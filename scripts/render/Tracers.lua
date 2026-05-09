local module = {
    name = "Tracers",
    description = "Draws lines from screen center to entities",
    category = "Render",
    enabled = false,
    settings = {
        distance = 64,
        r = 255, g = 85, b = 85, a = 200,
        width    = 1.0,
    },
    _settings_meta = {
        distance = { min = 8,  max = 256 },
        r = { min = 0, max = 255 },
        g = { min = 0, max = 255 },
        b = { min = 0, max = 255 },
        a = { min = 0, max = 255 },
        width = { min = 0.5, max = 4.0 },
    },
    _cam = nil,
}

function module:on_tick()
    local player = mc.player()
    if not player then self._cam = nil; return end
    local ok, px, py, pz, yaw, pitch = pcall(function()
        return player:x(), player:y() + 1.62, player:z(), player:yaw(), player:pitch()
    end)
    if not ok then self._cam = nil; return end
    local ok2, fov = pcall(mc.fov)
    self._cam = { px=px, py=py, pz=pz, yaw=yaw, pitch=pitch, fov=ok2 and fov or 70 }
end

local function world_to_screen(tx, ty, tz, cam, sw, sh)
    local dx = tx - cam.px
    local dy = ty - cam.py
    local dz = tz - cam.pz

    local yr = math.rad(cam.yaw)
    local pr = math.rad(cam.pitch)
    local cyr, syr = math.cos(yr), math.sin(yr)
    local cpr, spr = math.cos(pr), math.sin(pr)

    local cam_x =  dx*cyr                + dz*syr
    local cam_y = -dx*syr*spr + dy*cpr   + dz*cyr*spr
    local cam_z = -dx*syr*cpr - dy*spr   + dz*cyr*cpr

    if cam_z <= 0.1 then return nil end

    local scale  = 1.0 / math.tan(math.rad(cam.fov * 0.5))
    local aspect = sw / sh

    local sx = ( cam_x / cam_z * scale / aspect + 1) * sw * 0.5
    local sy = (-cam_y / cam_z * scale           + 1) * sh * 0.5
    return sx, sy
end

anemoia.on_render(function(painter)
    if not module.enabled then return end
    local cam = module._cam
    if not cam then return end

    local sw, sh   = painter:screen_size()
    local cx, cy   = sw * 0.5, sh * 0.75
    local dist_sq  = module.settings.distance ^ 2
    local r, g, b, a = module.settings.r, module.settings.g, module.settings.b, module.settings.a
    local w        = module.settings.width

    for _, e in ipairs(mc.entities()) do
        if e:alive() and not e:is_local_player() then
            local ex, ey, ez = e:x(), e:y() + 1.0, e:z()
            local d2 = (ex-cam.px)^2 + (ey-cam.py)^2 + (ez-cam.pz)^2
            if d2 <= dist_sq then
                local tx, ty = world_to_screen(ex, ey, ez, cam, sw, sh)
                if tx then
                    painter:line(cx, cy, tx, ty, r, g, b, a, w)
                end
            end
        end
    end
end)

anemoia.register(module)
