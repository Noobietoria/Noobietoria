# Noobietoria Studio — Instance Reference

Danh sách các Instance có thể dùng trong Noobietoria Studio, cùng service liên quan để điều khiển chúng.

---

## :material-cube-outline: Part

Part là 1 object cơ bản trong Noobietoria Places.

**Các thuộc tính:** RGBA, Images Texture (NImageAssets), Size (XYZ), position và orientation.

**Các loại:** `Part(Block)`, `Sphere`, `Wedge`, `Cylinder`, `MeshPart`, `Triangle`, `Mesh`.

**Services liên quan:** [TweenService](api.md#tweenservice), [InstanceService](api.md#instanceservice), [CollisionService](api.md#collisionservice), [PhysicsService](api.md#physicsservice)

---

## :material-ladder: TrussPart

Con của [`Part`](#part). TrussPart là 1 object giúp bạn có thể leo trèo.

**Các thuộc tính:** Gồm các thuộc tính của `Part` và `ClimbSpeed`.

**Services liên quan:** Giống `Part`.

---

## :material-pin: Attachment

Attachment là 1 điểm neo trong môi trường 3D.

**Các thuộc tính:** `Position`, `Orientation`.

**Service liên quan:** [InstanceService](api.md#instanceservice)

---

## :material-crop-square: Frame

Frame là 1 object chứa mọi thứ liên quan GUI.

**Các thuộc tính:** RGBA, Size (XY), position và orientation.

**Services liên quan:** [InstanceService](api.md#instanceservice)

---

## :material-format-text: TextLabel

TextLabel là 1 object chứa chữ.

**Các thuộc tính:** RGBA, Size (XY), position và orientation, `Text` (nội dung chữ), `TextSize`, `Font`, `TextColor3`.

**Services liên quan:** [InstanceService](api.md#instanceservice), [TextService](api.md#textservice)

---

## :material-gesture-tap-button: TextButton

TextButton là 1 object là 1 cái nút.

**Các thuộc tính:** RGBA, Size (XY), position và orientation, `Text` (nội dung chữ), `TextSize`, `Font`, `TextColor3`.

**Services liên quan:** [InstanceService](api.md#instanceservice), [TextService](api.md#textservice)

---

## :material-image-outline: ImageLabel

ImageLabel là 1 object hiển thị hình ảnh.

**Các thuộc tính:** RGBA, Size (XY), position và orientation, `Image` (Asset ID từ NImageAssets), `ImageColor3`, `ImageTransparency`, `ScaleType`.

**Services liên quan:** [InstanceService](api.md#instanceservice), [UIService](api.md#uiservice), [TextService](api.md#textservice)

---

## :material-image-frame: ImageButton

ImageButton là 1 object là 1 cái nút hiển thị hình ảnh thay cho chữ.

**Các thuộc tính:** Gồm các thuộc tính của `ImageLabel` và `Text` (nội dung chữ đè lên nút, có thể để trống).

**Services liên quan:** [InstanceService](api.md#instanceservice), [UIService](api.md#uiservice), [TextService](api.md#textservice)

---

## :material-form-textbox: TextBox

TextBox là 1 object cho phép người chơi nhập chữ.

**Các thuộc tính:** RGBA, Size (XY), position và orientation, `Text`, `PlaceholderText`, `TextSize`, `Font`, `TextColor3`, `ClearTextOnFocus`, `MultiLine`.

**Services liên quan:** [InstanceService](api.md#instanceservice), [UIService](api.md#uiservice), [TextService](api.md#textservice)

---

## :material-arrow-expand-vertical: ScrollingFrame

ScrollingFrame là 1 Frame có thể cuộn khi nội dung bên trong vượt quá kích thước khung.

**Các thuộc tính:** Gồm các thuộc tính của `Frame`, `CanvasSize` (XY), `ScrollBarThickness`, `ScrollingEnabled`, `ElasticBehavior`.

**Services liên quan:** [InstanceService](api.md#instanceservice), [UIService](api.md#uiservice)

---

## :material-lan-connect: NetworkEvent

NetworkEvent là 1 object dùng để nối mạng (giao tiếp 2 chiều) giữa [`ServerScript`](#serverscript) và [`ClientScript`](#clientscript).

**Các thuộc tính:** `Name` (tên định danh, dùng để 2 bên tìm đúng event với nhau).

**Nơi đặt:** Nên đặt trong `ReplicatedStorage` (hoặc tương đương) để cả Client lẫn Server cùng nhìn thấy được Instance này.

**Cách hoạt động:**

- Server có thể gọi `FireClient` / `FireAllClients` để gửi dữ liệu xuống Client, và lắng nghe `OnServerEvent` để nhận dữ liệu Client gửi lên.
- Client có thể gọi `FireServer` để gửi dữ liệu lên Server, và lắng nghe `OnClientEvent` để nhận dữ liệu Server gửi xuống.

!!! warning "Lưu ý"
    Dữ liệu truyền qua NetworkEvent phải serialize được (string, number, boolean, table dữ liệu thuần). Không truyền được Instance sống trực tiếp, hãy dùng GUID/Name để tham chiếu.

**Services liên quan:** [NetworkService](api.md#networkservice), [InstanceService](api.md#instanceservice)

---

## :material-server: ServerScript

ServerScript là 1 Script chỉ chạy ở máy chủ (Server), **không** replicate xuống Client và Client không thể đọc được mã nguồn bên trong.

**Các thuộc tính:** `Source` (mã Luau), `Enabled` (bật/tắt chạy script), `Parent`.

**Cách hoạt động:** Chạy ngay khi được tạo/khi máy chủ khởi động (tùy vị trí đặt), có toàn quyền truy cập các Service phía Server ([DataStoreService](api.md#datastoreservice), [BanService](api.md#banservice), [PurchaseService](api.md#purchaseservice), ...).

!!! tip "Lưu ý"
    Dùng ServerScript cho logic nhạy cảm (kinh tế, chống gian lận, dữ liệu) vì Client không thể can thiệp hay xem được.

**Services liên quan:** [NetworkService](api.md#networkservice) (để giao tiếp với Client qua NetworkEvent), toàn bộ các Service phía Server.

---

## :material-cellphone-link: ClientScript

ClientScript (hay `LocalScript`) là 1 Script chỉ chạy ở máy Client của từng người chơi.

**Các thuộc tính:** `Source` (mã Luau), `Enabled`, `Parent`.

**Cách hoạt động:** Chỉ chạy trên máy của người chơi sở hữu nó (thường đặt trong `StarterPlayerScripts` hoặc tương đương), **không** có quyền truy cập trực tiếp các Service nhạy cảm phía Server ([DataStoreService](api.md#datastoreservice), [BanService](api.md#banservice), ...).

!!! tip "Lưu ý"
    Dùng ClientScript cho UI, hiệu ứng, input, camera — các thứ chỉ ảnh hưởng đến trải nghiệm của riêng người chơi đó. Muốn thay đổi dữ liệu chung (tiền tệ, inventory, ...) phải gửi yêu cầu qua NetworkEvent lên ServerScript xử lý.

**Services liên quan:** [NetworkService](api.md#networkservice), [InputService](api.md#inputservice), [CameraService](api.md#cameraservice), [UIService](api.md#uiservice), [TextService](api.md#textservice)

---

## :material-puzzle-outline: ModuleScript

ModuleScript là 1 Script có thể tái sử dụng (`require` được từ Script khác), dùng để chia sẻ hàm/dữ liệu chung, **không** tự chạy khi được tạo.

**Các thuộc tính:** `Source` (mã Luau, phải `return` 1 giá trị — thường là table), `Parent`.

**Cách hoạt động:** Được nạp bằng hàm `require(Instance)`, chạy đúng 1 lần và trả về (cache) cùng 1 giá trị cho mọi lần require tiếp theo trong cùng môi trường (Server hoặc Client).

!!! info "Lưu ý"
    ModuleScript đặt trong vùng dùng chung (ví dụ `ReplicatedStorage`) để cả Server lẫn Client cùng require được; ModuleScript đặt riêng phía Server (ví dụ `ServerStorage`) thì chỉ ServerScript require được.

**Services liên quan:** [InstanceService](api.md#instanceservice)

---

## Ví dụ tổng hợp: Client ↔ Server qua NetworkEvent + ModuleScript

```luau
-- (ModuleScript "EconomyUtils" trong ReplicatedStorage)
local EconomyUtils = {}

function EconomyUtils.FormatCoin(amount)
    return "🪙 " .. tostring(amount)
end

return EconomyUtils
```

```luau
-- (ServerScript, dùng InstanceService để require ModuleScript
--  và NetworkService để lắng nghe Client)
local EconomyUtils = InstanceService.require("EconomyUtils")

InstanceService.Create("BuyItem", "NetworkEvent", "ReplicatedStorage")
NetworkService.OnServerEvent("BuyItem", function(Username, ItemId)
    CurrencyService.Deduct(Username, "Coin", 100)
    NetworkService.FireClient("BuyItem", Username, "ok", EconomyUtils.FormatCoin(CurrencyService.GetBalance(Username, "Coin")))
end)
```

```luau
-- (ClientScript)
NetworkService.OnClientEvent("BuyItem", function(status, message)
    if status == "ok" then
        UIService.SetText("CoinLabel", message)
    end
end)

UIService.OnClick("BuyButton", function()
    NetworkService.FireServer("BuyItem", "Sword_01")
end)
```
