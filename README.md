# Anemoia Client

Linux Minecraft ghost client for MC 26.2. Rust `.so` injection + Lua 5.4 scripting.

## Installation

Download the latest release from [Releases](../../releases), extract, and run:

```bash
tar -xzf anemoia-vX.Y.Z-linux-x86_64.tar.gz
cd anemoia-vX.Y.Z-linux-x86_64
./install.sh          # installs to ~/.anemoia
```

Then launch:

```bash
~/.anemoia/anemoia-launcher
# or inject into a running Minecraft process:
~/.anemoia/anemoia-inject --pid <pid>
```

## Building from source

```bash
git clone https://github.com/Remgr12/Anemoia-Client.git
cd Anemoia-Client
chmod +x build.sh
./build.sh
```

Requires Rust stable + `libfontconfig-dev`, `libxcb-*-dev`, `libxkbcommon-dev`, `libssl-dev` (Ubuntu/Debian):

```bash
sudo apt-get install -y libfontconfig1-dev libxcb-render0-dev libxcb-shape0-dev \
    libxcb-xfixes0-dev libxkbcommon-dev libssl-dev
```

## GUI

Press **Right Shift** in-game to open the ClickGUI. Modules are grouped by category. Click a module to toggle it, click **Bind** to set a keybind, click **Settings** to configure it.

## Modules

| Category | Module | Description |
|----------|--------|-------------|
| Combat | Aimbot | Smooth server-side rotation toward nearest target |
| Combat | AutoClicker | Randomised click rate |
| Combat | Criticals | Times attacks to hit on falling ticks |
| Combat | KeepSprint | Prevents losing sprint on attack |
| Combat | KillAura | Auto-attacks nearby entities |
| Combat | SilentAura | Attacks without changing client-side rotation |
| Combat | Velocity | Reduces or removes knockback |
| Movement | AirJump | Jump while airborne |
| Movement | HighJump | Boosted jump height |
| Movement | LiquidWalk | Walk on water/lava |
| Movement | NoSlow | Removes slowdown from blocks/items |
| Movement | NoWeb | Removes cobweb slowdown |
| Movement | Spider | Climb any wall |
| Movement | Sprint | Auto-sprint |
| Movement | Step | Step up blocks without jumping |
| Player | AntiAFK | Sends periodic inputs to avoid AFK kick |
| Player | AutoRespawn | Auto-clicks respawn on death |
| Player | ChestStealer | Auto-loots nearby containers |
| Player | FastUse | Reduces item use delay |
| Player | NoFall | Spoofs on-ground to negate fall damage |
| Player | NoPitchLimit | Removes 90° pitch clamp |
| Player | Reach | Extends attack range |
| Render | AntiBlind | Removes blindness/darkness effects |
| Render | ESP | Entity box outlines through walls |
| Render | FullBright | Maximum gamma |
| Render | Nametags | Entity name labels |
| Render | Radar | 2D mini-map of nearby entities |
| Render | Tracers | Lines from player to nearby entities |
| Misc | AntiBot | Filters bot entities from combat targeting |
| Misc | FakeLag | Holds and releases packets in bursts |
| Misc | PacketLogger | Logs sent packets to console |
| Misc | ZulipBridge | Forwards chat to a Zulip channel |
| World | AutoTool | Switches to best tool before breaking blocks |
| World | FastPlace | Reduces block placement delay |
| World | Scaffold | Places blocks under you automatically |

## Writing Modules

Modules are Lua 5.4 files in `~/.anemoia/scripts/<category>/`. Drop a new `.lua` file there and restart.

```lua
local module = {
    name        = "MyModule",
    description = "Does something",
    category    = "Misc",
    enabled     = false,
    settings    = {
        speed   = 1.0,
        mode    = "Fast",
        hotkey  = 0,    -- keybind (GLFW key code)
    },
    _settings_meta = {
        speed   = { min = 0.5, max = 5.0 },
        mode    = { type = "enum", options = { "Fast", "Slow" } },
        hotkey  = { type = "keybind" },
    },
}

function module:on_tick()
    local player = mc.player()
    if not player then return end
    -- your logic here
end

-- Optional render callback (no JNI calls here — use on_tick to cache data)
anemoia.on_render(function(painter)
    if not module.enabled then return end
    painter:rect(10, 10, 100, 20, 0, 0, 0, 180, 4)
    painter:text("Hello", 15, 14, 255, 255, 255, 255, 12)
end)

anemoia.register(module)
```

### on_tick / on_render

| Hook | Thread | JNI allowed |
|------|--------|-------------|
| `module:on_tick()` | game tick | yes |
| `anemoia.on_render(fn)` | render thread | no — use cached data from on_tick |

### on_packet_send

```lua
anemoia.on_packet_send(function(packet)
    local name = packet:type_name()
    if name:find("Position") then
        return true  -- cancel packet
    end
end)
```

Return `true` to cancel the packet. Return `false` or nothing to let it through.

## Lua API Reference

### mc

| Function | Returns | Description |
|----------|---------|-------------|
| `mc.player()` | `LocalPlayer` or `nil` | Local player, nil if not in world |
| `mc.entities()` | `Entity[]` | Snapshot of all loaded entities |
| `mc.attack(entity)` | — | Send attack swing at entity |
| `mc.click()` | — | Left mouse click |
| `mc.is_key_down(key)` | `bool` | GLFW key/button held |
| `mc.send_packet(packet)` | — | Send a packet (triggers on_packet_send) |
| `mc.block(x, y, z)` | `Block` | Block at world position |

### LocalPlayer

| Method | Returns | Description |
|--------|---------|-------------|
| `x()`, `y()`, `z()` | `number` | Position |
| `yaw()`, `pitch()` | `number` | Rotation (degrees) |
| `set_yaw(v)`, `set_pitch(v)` | — | Set rotation |
| `velocity()` | `{x,y,z}` | Delta movement |
| `set_velocity(x,y,z)` | — | Set delta movement |
| `on_ground()` | `bool` | On ground flag |
| `set_on_ground(bool)` | — | Spoof ground flag |
| `is_sprinting()` | `bool` | |
| `set_sprinting(bool)` | — | |
| `fall_distance()` | `number` | |
| `hurt_time()` | `number` | Ticks remaining of hurt cooldown |
| `input()` | `{up,down,left,right,jumping,sneaking}` | Key input state (all `bool`) |
| `health()` | `number` | |
| `max_health()` | `number` | |
| `absorption()` | `number` | Absorption hearts |
| `food_level()` | `number` | |
| `is_dead()` | `bool` | |
| `respawn()` | — | |
| `is_sprinting()` | `bool` | |
| `is_using_item()` | `bool` | |
| `stop_using_item()` | — | |
| `is_in_water()` | `bool` | |
| `is_in_lava()` | `bool` | |
| `is_in_web()` | `bool` | Cobweb or sweet berry bush |
| `is_collided_horizontally()` | `bool` | |
| `jump()` | — | `jumpFromGround()` |
| `get_step_height()` | `number` | |
| `set_step_height(v)` | — | |
| `swing_arm(hand?)` | — | `"MAIN_HAND"` or `"OFF_HAND"` |
| `has_effect(name)` | `bool` | e.g. `"speed"`, `"blindness"` |
| `remove_effect(name)` | — | |
| `main_hand_item()` | `ItemStack` | |
| `off_hand_item()` | `ItemStack` | |
| `inventory()` | `Inventory` | |
| `container_id()` | `number` | Open container ID |
| `display_message(msg)` | — | Shows action-bar-style message |
| `send_chat(msg)` | — | Sends chat message |

### Entity (snapshot — no JNI, safe in on_render)

| Method | Returns | Description |
|--------|---------|-------------|
| `x()`, `y()`, `z()` | `number` | |
| `type_id()` | `string` | e.g. `"minecraft:player"` |
| `alive()` | `bool` | |
| `is_local_player()` | `bool` | |
| `dist_sq(x,y,z)` | `number` | Squared distance to point |

### Painter (on_render only)

| Method | Description |
|--------|-------------|
| `rect(x,y,w,h, r,g,b,a, radius)` | Filled rounded rectangle |
| `rect_outline(x,y,w,h, r,g,b,a, thickness, radius)` | Outline |
| `text(str, x,y, r,g,b,a, size)` | Text label |
| `line(x1,y1,x2,y2, r,g,b,a, thickness)` | Line |
| `circle(cx,cy,radius, r,g,b,a)` | Filled circle |

### ItemStack

| Method | Returns |
|--------|---------|
| `item_id()` | `string` — e.g. `"minecraft:diamond_sword"` |
| `count()` | `number` |
| `is_empty()` | `bool` |

### Inventory

| Method | Returns/Description |
|--------|---------------------|
| `selected_slot()` | `number` — hotbar slot (0–8) |
| `set_selected_slot(n)` | — |

### Packet

| Method | Returns |
|--------|---------|
| `type_name()` | `string` — Java class name |

### Block

| Method | Returns |
|--------|---------|
| `id()` | `string` — e.g. `"minecraft:stone"` |
| `is_air()` | `bool` |

## Architecture

```
injector/        — CLI: attaches to running JVM via JVM Attach API, loads agent_loader
agent_loader/    — JNI agent: loads libanemoia_client.so into the JVM
client/          — core: hooks glXSwapBuffers, runs Lua engine, egui GUI
launcher/        — CLI: launches Minecraft then injects automatically
scripts/         — Lua modules (loaded at runtime from ~/.anemoia/scripts/)
```

Injection flow: `anemoia-inject` → ptrace attach → write `agent_loader.so` path → `dlopen` → `libagent_loader.so` → `JVM_OnLoad` → dlopen `libanemoia_client.so` → hook `glXSwapBuffers` → tick/render loop.

## License

See [LICENSE](LICENSE).
