local module = {
    name = "Nametags",
    description = "Shows entity names and health above their heads",
    category = "Render",
    enabled = false,
    settings = {
        distance = 64,
        health   = true,
        size     = 12,
    },
    _settings_meta = {
        distance = { min = 8,  max = 256 },
        size     = { min = 8,  max = 24 },
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

-- Camera basis (MC: yaw 0=South, positive pitch=down):
--   right   = ( cos(yaw),             0,         sin(yaw)            )
--   up      = (-sin(yaw)*sin(pitch),  cos(pitch), cos(yaw)*sin(pitch) )
--   forward = (-sin(yaw)*cos(pitch), -sin(pitch), cos(yaw)*cos(pitch) )
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

    local sw, sh  = painter:screen_size()
    local dist_sq = module.settings.distance ^ 2
    local sz      = module.settings.size
    local show_hp = module.settings.health

    for _, e in ipairs(mc.entities()) do
        if e:alive() and not e:is_local_player() then
            local ex, ey, ez = e:x(), e:y(), e:z()
            local d2 = (ex-cam.px)^2 + (ey-cam.py)^2 + (ez-cam.pz)^2
            if d2 <= dist_sq then
                local sx, sy = world_to_screen(ex, ey + 2.3, ez, cam, sw, sh)
                if sx then
                    local label = e:name()
                    if show_hp then
                        local hp = e:health()
                        if hp > 0 then
                            label = label .. " " .. math.floor(hp + 0.5)
                        end
                    end
                    local ox = sz * #label * 0.25
                    painter:text(sx - ox + 1, sy - sz + 1, label, 0,   0,   0,   120, sz)
                    painter:text(sx - ox,     sy - sz,     label, 255, 255, 255, 230, sz)
                end
            end
        end
    end
end)

anemoia.register(module)
