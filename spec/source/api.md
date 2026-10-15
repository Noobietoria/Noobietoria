# Noobietoria Studio — API Reference

---

## <img src="/Docs/studio/accessory.png" alt="Accessory icon" width="28" style="vertical-align:-6px" /> Accessory

Các vật phẩm trên Noobietoria Marketplace.

> **Note:** Accessory chỉ bao gồm Items và Clothes. Các vật phẩm khác ví dụ Gamepasses không thể triển khai dù bạn có cố tình nhập vào.

**Cách triển khai:**

- **Dành cho player:** `[Tên Player] -> IngameAccessoriesID` rồi sau đó nhập chuỗi ID
- **Dành cho NPC:** `[Tên NPC] -> IngameAccessoriesID` rồi sau đó nhập chuỗi ID

Chuỗi ID tách nhau bằng dấu `,`, không yêu cầu mua để triển khai lên player.

Không thể triển khai trong mục `DefaultAccessories` của player do mục này là các item mà player đã triển khai. Chỉ có thể bật hoặc tắt biến `DefaultAccessories` tại root `Player`.

---

## <img src="/Docs/studio/accessoryservice.png" alt="AccessoryService icon" width="28" style="vertical-align:-6px" /> AccessoryService

Cơ chế gần giống Accessory, nhưng đây là biến tĩnh và không thể dùng Luau để ghi đè nó.

**Lưu ý:** Tất cả 3 tham số đều là chuỗi, không được có giá trị nil.

```luau
-- Đeo phụ kiện cho người chơi có tên "Player1"
AccessoryService("Player1", "Player", "1082345, 1082350, 1082360")
-- Đeo phụ kiện cho NPC có tên "ShopKeeper" ở Workspace
AccessoryService("ShopKeeper", "NPC", "12345678, 87654321")
```

---

## <img src="/Docs/studio/audioassets.png" alt="AudioAssets icon" width="28" style="vertical-align:-6px" /> AudioAssets

Sử dụng âm thanh trong trò chơi bất kì.

> **Note:** Audio chỉ bao gồm các đoạn âm thanh (dài ngắn gì team chúng tôi không quan tâm). Các loại Assets khác không được chấp nhận và sẽ không hoạt động.

**Cách triển khai:**

- Tạo: `New -> Audio` sau đó vào `Audio -> ID` và nhập ID.
- Bật Loop: `Audio -> Loop = true`

---

## <img src="/Docs/studio/audiocservice.png" alt="AudioCService icon" width="28" style="vertical-align:-6px" /> AudioCService

Điều khiển 1 thực thể `Audio` đã tồn tại.

```luau
AudioService.New(Name, ID)
AudioService.Stop(Name)
AudioService.Resume(Name)
AudioService.Pause(Name)
AudioService.SetVolume(Name, Volume)
AudioService.Change(Name, ID)
AudioService.Loop(Name, Boolean)
```

---

## <img src="/Docs/studio/datastoreservice.png" alt="DataStoreService icon" width="28" style="vertical-align:-6px" /> DataStoreService

DataStoreService là dịch vụ cho phép bạn lưu trữ và lấy dữ liệu từ cơ sở dữ liệu của chúng tôi.

**Giới hạn:**

- Tối đa 15 datastores/game
- Dung lượng tối đa: Key tối đa 50 kí tự ASCII và value là 1MB. Dung lượng tối đa là 25MB + (5MB×N) trong đó N là số visits.
- Giới hạn request: 15 + (10×N) request/máy chủ/phút trong đó N là CCU và 16 kết nối đồng thời tối đa đến Datastore tổng cộng toàn máy chủ.

**Lưu ý:**

- `Init` tức là thiết lập kết nối đến datastore. Cần `Close` sau khi thao tác xong.
- `SessionID` là bạn tự random kiểu gì cũng được miễn là base32.
- `Value` phải là JSON.
- Nếu bạn không có bất cứ tin nhắn nào sau 15 giây thì sẽ tự động ngắt kết nối. Trường hợp có data đang chạy ở trong và chưa xác nhận xong thì sẽ chờ đến khi xong.

```luau
DataStoreService.Init(SessionID)
DataStoreService.Close(SessionID)
DataStoreService.Insert(SessionID, Key, Value)
DataStoreService.Get(SessionID, Key)
DataStoreService.GetKeys(SessionID)
DataStoreService.Delete(SessionID, Key)

--- dành cho mấy dev lười: chúng tôi thêm luôn các API bên Roblox vào. Tự động quản lý kết nối.
DataStoreService:GetDataStore(name, scope)
DataStoreService:GetOrderedDataStore(name, scope)
DataStoreService:GetRequestBudgetForRequestType(requestType)
DataStore:GetAsync(key)
DataStore:SetAsync(key, value, userIds, options)
DataStore:UpdateAsync(key, transformFunction)
DataStore:RemoveAsync(key)
```

---

## <img src="/Docs/studio/tweenservice.png" alt="TweenService icon" width="28" style="vertical-align:-6px" /> TweenService

TweenService là 1 dịch vụ cho phép bạn di chuyển vật thể từ chỗ này sang chỗ khác mà không bị giật 1 đống.

**Lưu ý:**

- `Instance` là Tên của Part.
- Nếu có nhiều cái trùng tên thì hãy dùng GUID thay vì chỉ tên.

```luau
TweenService.Move(Instance, EndPosition, Time, EasingDirection, EasingStyle, Loop, RGBA)
TweenService.Stop(Instance)
TweenService.Resume(Instance)
TweenService.Pause(Instance)
TweenService.Back(Instance)
```

---

## <img src="/Docs/studio/playerservice.png" alt="PlayerService icon" width="28" style="vertical-align:-6px" /> PlayerService

PlayerService là 1 dịch vụ cho phép bạn theo dõi: Người vào, người ra, đi đâu, về đâu, bay lên trời, lên xe rồng à lộn, tên người chơi, quốc gia của họ.

```luau
PlayerService.GetPlayers()
PlayerService.GetPlayer("Username")
PlayerService.GetPlayerName(UserId)
PlayerService.GetPlayerCountry(UserId)
PlayerService.GetPlayerAvatar(UserId)
PlayerService.GetPlayerAvatarUrl(UserId)
PlayerService.GetPlayerThumbnail(UserId)
PlayerService.GetPlayerThumbnailUrl(UserId)
PlayerService.GetPlayerCharacter(UserId)
PlayerService.GetPlayerCharacterUrl(UserId)
PlayerService.GetCoors(Username)
PlayerService.Teleport(Username, Coors)
```

---

## <img src="/Docs/studio/httpservice.png" alt="HttpService icon" width="28" style="vertical-align:-6px" /> HttpService

HttpService là 1 dịch vụ cho phép bạn có thể gửi yêu cầu đến API ngoài.

**Lưu ý:**

- Các thể loại request được hỗ trợ: `GET`, `POST`, `PUT`, `DELETE`, `HEAD`, `OPTIONS`
- Các Content-Type: URL-encoded Params, JSON, Upload File

```luau
HttpService.Prepare(
    Id: int,
    Method: string,
    Url: string,
    Headers: table,
    Body: string,
    ContentType: string
)
HttpService.Request(Id)
HttpService.Release(Id)
```

---

## <img src="/Docs/studio/uiservice.png" alt="UIService icon" width="28" style="vertical-align:-6px" /> UIService

UIService là 1 dịch vụ cho phép bạn tạo và quản lý giao diện người dùng (UI) trong trò chơi.

> **Note:** UIService chỉ hỗ trợ các thành phần UI cơ bản như Frame, TextLabel, TextButton, ImageLabel, ImageButton, TextBox và ScrollingFrame.

**Lưu ý:**

- `Name` là tên định danh của UI element (chuỗi, không được trùng lặp).
- `Parent` là tên của element cha hoặc `"ScreenGui"` nếu là root.
- Các thuộc tính (`Properties`) truyền vào dưới dạng table.
- `AssetId` là ID hình ảnh từ NImageAssets (chuỗi), dùng cho ImageLabel và ImageButton.
- `SetPlaceholder` chỉ áp dụng cho TextBox.
- `SetCanvasSize`, `GetCanvasSize`, `SetScrollPosition` và `GetScrollPosition` chỉ áp dụng cho ScrollingFrame; `CanvasSize` là kích thước vùng nội dung có thể cuộn (XY).

```luau
UIService.Create(Name, Type, Parent, Properties)
UIService.Destroy(Name)
UIService.SetProperty(Name, Property, Value)
UIService.GetProperty(Name, Property)
UIService.SetVisible(Name, Boolean)
UIService.Tween(Name, Properties, Time, EasingDirection, EasingStyle)
UIService.OnClick(Name, Callback)
UIService.OnTextChanged(Name, Callback)
UIService.RemoveCallback(Name)
UIService.SetText(Name, Text)
UIService.GetText(Name)
UIService.SetImage(Name, AssetId)
UIService.SetPlaceholder(Name, PlaceholderText)
UIService.SetCanvasSize(Name, Width, Height)
UIService.GetCanvasSize(Name)
UIService.SetScrollPosition(Name, X, Y)
UIService.GetScrollPosition(Name)
```

---

## <img src="/Docs/studio/instanceservice.png" alt="InstanceService icon" width="28" style="vertical-align:-6px" /> InstanceService

InstanceService là 1 dịch vụ cho phép bạn tạo, xóa, lấy GUID và di chuyển các [Instance](instance.md) (Part, Model, NetworkEvent, ServerScript, ClientScript, ModuleScript, ...) trong Workspace hoặc các vùng chứa khác (ReplicatedStorage, ServerStorage, StarterPlayerScripts, ...).

**Lưu ý:**

- `Name` là tên của Instance (chuỗi).
- `Type` là loại Instance cần tạo (chuỗi, ví dụ: `"Part"`, `"Model"`, `"SpawnLocation"`, `"NetworkEvent"`, `"ServerScript"`, `"ClientScript"`, `"ModuleScript"`).
- `Parent` là tên Instance cha hoặc tên vùng chứa gốc (`"Workspace"`, `"ReplicatedStorage"`, `"ServerStorage"`, `"StarterPlayerScripts"`, ...).
- `GUID` là định danh duy nhất của Instance, dùng thay tên khi có nhiều Instance trùng tên.
- `Position` là bảng tọa độ `{X, Y, Z}` (chỉ áp dụng cho các Instance có vị trí trong không gian 3D, ví dụ Part).
- `Index` trong `GetGUID(Name, Index)` là số thứ tự (bắt đầu từ 1) của Instance trong danh sách các Instance trùng tên, theo thứ tự tạo. Nếu bỏ qua `Index` thì mặc định lấy `Index = 1`.
- Dùng `GetGUIDs(Name)` để lấy toàn bộ GUID của tất cả Instance trùng tên cùng lúc.
- Với `Type` là `"ServerScript"` hoặc `"ClientScript"`, truyền `Source` (mã Luau) qua `Properties.Source`; `Enabled` mặc định là `true`, có thể truyền `Properties.Enabled = false` để tạo mà chưa chạy ngay.
- Với `Type` là `"ModuleScript"`, `Properties.Source` bắt buộc phải là đoạn mã có `return` 1 giá trị, nếu không sẽ lỗi khi require.

```luau
InstanceService.Create(Name, Type, Parent, Properties)
InstanceService.Destroy(Name, Index)
InstanceService.DestroyByGUID(GUID)
InstanceService.GetGUID(Name, Index)
InstanceService.GetGUIDs(Name)
InstanceService.GetByGUID(GUID)
InstanceService.Move(Name, Position, Index)
InstanceService.MoveByGUID(GUID, Position)
InstanceService.SetEnabled(Name, Boolean, Index)
InstanceService.IsEnabled(Name, Index)
InstanceService.require(Name, Index)
InstanceService.requireByGUID(GUID)
```

---

## <img src="/Docs/studio/networkservice.png" alt="NetworkService icon" width="28" style="vertical-align:-6px" /> NetworkService

NetworkService là 1 dịch vụ điều khiển Instance [`NetworkEvent`](instance.md#networkevent) để giao tiếp 2 chiều giữa [`ServerScript`](instance.md#serverscript) và [`ClientScript`](instance.md#clientscript).

**Lưu ý:**

- `Name` là tên của NetworkEvent (chuỗi), phải trùng ở cả 2 bên Server và Client thì mới nhận được nhau.
- `Index` dùng khi có nhiều NetworkEvent trùng tên, giống cơ chế của InstanceService (bỏ qua thì mặc định `Index = 1`). Có thể dùng GUID thay Name+Index.
- `Username` trong các hàm gọi từ Server là tên người chơi nhận sự kiện, truyền `"*"` để gửi cho toàn bộ Client đang kết nối.
- Chỉ gọi được `FireServer`/`OnClientEvent` từ trong ClientScript, và chỉ gọi được `FireClient`/`FireAllClients`/`OnServerEvent` từ trong ServerScript. Gọi sai phía sẽ báo lỗi.
- `...` là danh sách tham số tùy ý (phải serialize được: string, number, boolean, table dữ liệu thuần) sẽ được truyền nguyên vẹn sang `Callback` ở đầu nhận.

```luau
-- Gọi được ở cả 2 phía (Server/Client), dùng để tạo/xóa Instance NetworkEvent
NetworkService.Create(Name, Parent)
NetworkService.Destroy(Name, Index)

-- Chỉ gọi được trong ServerScript
NetworkService.FireClient(Name, Username, ..., Index)
NetworkService.FireAllClients(Name, ..., Index)
NetworkService.OnServerEvent(Name, Callback, Index)

-- Chỉ gọi được trong ClientScript
NetworkService.FireServer(Name, ..., Index)
NetworkService.OnClientEvent(Name, Callback, Index)

-- Gọi được ở cả 2 phía
NetworkService.Disconnect(Name, Index)
```

---

## <img src="/Docs/studio/collisionservice.png" alt="CollisionService icon" width="28" style="vertical-align:-6px" /> CollisionService

CollisionService là 1 dịch vụ cho phép bạn quản lý va chạm giữa các đối tượng và nhóm va chạm trong trò chơi.

**Lưu ý:**

- `GroupName` là tên định danh của nhóm va chạm (chuỗi).
- `Instance` là tên của Part/Model cần gán vào nhóm.
- `SetCollision(GroupA, GroupB, Boolean)` để bật/tắt va chạm giữa 2 nhóm.

```luau
CollisionService.CreateGroup(GroupName)
CollisionService.DeleteGroup(GroupName)
CollisionService.AssignGroup(Instance, GroupName)
CollisionService.SetCollision(GroupA, GroupB, Boolean)
CollisionService.GetGroup(Instance)
CollisionService.GetGroups()
```

---

## <img src="/Docs/studio/lightingservice.png" alt="LightingService icon" width="28" style="vertical-align:-6px" /> LightingService

LightingService (hay EnvironmentService) là 1 dịch vụ cho phép bạn điều chỉnh thời gian trong ngày, ánh sáng môi trường và thời tiết của trò chơi.

**Lưu ý:**

- `TimeOfDay` là chuỗi theo định dạng `"HH:MM:SS"`.
- `WeatherType` là chuỗi: `"Clear"`, `"Rain"`, `"Storm"`, `"Fog"`, `"Snow"`.
- Thuộc tính ánh sáng (`Properties`) truyền vào dưới dạng table.

```luau
LightingService.SetTime(TimeOfDay)
LightingService.GetTime()
LightingService.SetWeather(WeatherType, Intensity)
LightingService.GetWeather()
LightingService.SetProperty(Property, Value)
LightingService.GetProperty(Property)
LightingService.SetAmbient(RGBA)
LightingService.SetFog(Enabled, Color, Start, End)
```

---

## <img src="/Docs/studio/notificationservice.png" alt="NotificationService icon" width="28" style="vertical-align:-6px" /> NotificationService

NotificationService là 1 dịch vụ cho phép bạn hiển thị thông báo dạng toast hoặc popup cho người chơi.

**Lưu ý:**

- `Username` là tên người chơi cần nhận thông báo, truyền `"*"` để gửi cho tất cả.
- `Duration` là thời gian hiển thị (số thực, đơn vị giây).
- `Type` là kiểu thông báo: `"Toast"`, `"Popup"`, `"Banner"`.

```luau
NotificationService.Send(Username, Title, Message, Type, Duration)
NotificationService.SendAll(Title, Message, Type, Duration)
NotificationService.Dismiss(Username, NotificationId)
NotificationService.OnDismiss(Username, Callback)
```

---

## <img src="/Docs/studio/inventoryservice.png" alt="InventoryService icon" width="28" style="vertical-align:-6px" /> InventoryService

InventoryService là 1 dịch vụ cho phép bạn quản lý túi đồ trong game của người chơi.

**Lưu ý:**

- `Username` là tên người chơi.
- `ItemId` là ID định danh của vật phẩm (chuỗi).
- `Quantity` là số lượng (số nguyên, tối thiểu 1).
- `Metadata` là thông tin bổ sung của vật phẩm (table, có thể nil).

```luau
InventoryService.GetInventory(Username)
InventoryService.AddItem(Username, ItemId, Quantity, Metadata)
InventoryService.RemoveItem(Username, ItemId, Quantity)
InventoryService.HasItem(Username, ItemId)
InventoryService.GetItemCount(Username, ItemId)
InventoryService.ClearInventory(Username)
InventoryService.TransferItem(FromUsername, ToUsername, ItemId, Quantity)
```

---

## <img src="/Docs/studio/leaderboardservice.png" alt="LeaderboardService icon" width="28" style="vertical-align:-6px" /> LeaderboardService

LeaderboardService là 1 dịch vụ cho phép bạn quản lý bảng xếp hạng và điểm số của người chơi trong trò chơi.

**Lưu ý:**

- `BoardName` là tên định danh của bảng xếp hạng (chuỗi).
- `Username` là tên người chơi.
- `Score` là giá trị điểm (số thực).
- `Order` là thứ tự sắp xếp: `"Ascending"` hoặc `"Descending"`.

```luau
LeaderboardService.CreateBoard(BoardName, Order)
LeaderboardService.DeleteBoard(BoardName)
LeaderboardService.SetScore(BoardName, Username, Score)
LeaderboardService.GetScore(BoardName, Username)
LeaderboardService.AddScore(BoardName, Username, Delta)
LeaderboardService.GetTopPlayers(BoardName, Limit)
LeaderboardService.GetRank(BoardName, Username)
LeaderboardService.ResetBoard(BoardName)
```

---

## <img src="/Docs/studio/spawnservice.png" alt="SpawnService icon" width="28" style="vertical-align:-6px" /> SpawnService

SpawnService là 1 dịch vụ cho phép bạn quản lý điểm hồi sinh (spawn point) của Player và NPC trong trò chơi.

**Lưu ý:**

- `SpawnName` là tên định danh của điểm hồi sinh (chuỗi).
- `Position` là bảng tọa độ `{X, Y, Z}`.
- `TargetType` là kiểu đối tượng: `"Player"` hoặc `"NPC"`.

```luau
SpawnService.CreateSpawn(SpawnName, Position)
SpawnService.DeleteSpawn(SpawnName)
SpawnService.SetSpawn(SpawnName, Position)
SpawnService.GetSpawn(SpawnName)
SpawnService.GetAllSpawns()
SpawnService.AssignSpawn(TargetType, Username, SpawnName)
SpawnService.RespawnAt(TargetType, Username, SpawnName)
SpawnService.Respawn(TargetType, Username)
```

---

## <img src="/Docs/studio/cameraservice.png" alt="CameraService icon" width="28" style="vertical-align:-6px" /> CameraService

CameraService là 1 dịch vụ cho phép bạn điều khiển góc nhìn camera và rung màn hình cho người chơi.

**Lưu ý:**

- `Username` là tên người chơi cần áp dụng, truyền `"*"` để áp dụng cho tất cả.
- `CameraType` là kiểu camera: `"Classic"`, `"Follow"`, `"Fixed"`, `"Scriptable"`.
- `Position` và `LookAt` là bảng tọa độ `{X, Y, Z}`.
- `ShakeIntensity` và `Duration` là số thực.

```luau
CameraService.SetType(Username, CameraType)
CameraService.SetPosition(Username, Position, LookAt)
CameraService.SetFOV(Username, FieldOfView)
CameraService.LockTo(Username, Instance)
CameraService.Unlock(Username)
CameraService.Shake(Username, ShakeIntensity, Duration)
CameraService.Reset(Username)
```

---

## <img src="/Docs/studio/analyticsservice.png" alt="AnalyticsService icon" width="28" style="vertical-align:-6px" /> AnalyticsService

AnalyticsService là 1 dịch vụ cho phép bạn theo dõi các chỉ số và sự kiện đơn giản trong trò chơi.

**Lưu ý:**

- `EventName` là tên sự kiện cần ghi nhận (chuỗi).
- `Properties` là thông tin bổ sung của sự kiện (table, có thể nil).
- `Username` là tên người chơi liên quan đến sự kiện (chuỗi, có thể nil).
- `Interval` là khoảng thời gian theo dõi theo giây.

```luau
AnalyticsService.TrackEvent(EventName, Username, Properties)
AnalyticsService.TrackError(ErrorMessage, Username, Properties)
AnalyticsService.SetCounter(MetricName, Value)
AnalyticsService.IncrementCounter(MetricName, Delta)
AnalyticsService.GetCounter(MetricName)
AnalyticsService.GetEvents(EventName, Limit)
AnalyticsService.ClearEvents(EventName)
```

---

## <img src="/Docs/studio/achievementservice.png" alt="AchievementService icon" width="28" style="vertical-align:-6px" /> AchievementService

AchievementService là 1 dịch vụ cho phép bạn quản lý và mở khóa thành tựu cho người chơi trong trò chơi.

**Lưu ý:**

- `AchievementId` là ID định danh của thành tựu (chuỗi).
- `Username` là tên người chơi.
- `Progress` là tiến trình hiện tại (số thực).
- `MaxProgress` là tiến trình tối đa để mở khóa thành tựu (số thực).

```luau
AchievementService.Register(AchievementId, Name, Description, MaxProgress)
AchievementService.Unregister(AchievementId)
AchievementService.GetAll()
AchievementService.GetProgress(Username, AchievementId)
AchievementService.SetProgress(Username, AchievementId, Progress)
AchievementService.AddProgress(Username, AchievementId, Delta)
AchievementService.Unlock(Username, AchievementId)
AchievementService.IsUnlocked(Username, AchievementId)
AchievementService.GetUnlocked(Username)
AchievementService.OnUnlock(Username, Callback)
```

---

## <img src="/Docs/studio/purchaseservice.png" alt="PurchaseService icon" width="28" style="vertical-align:-6px" /> PurchaseService

PurchaseService là 1 dịch vụ cho phép bạn xử lý các giao dịch, đơn hàng và tiền tệ trong game.

**Lưu ý:**

- `OrderId` là ID định danh của đơn hàng (chuỗi, tự sinh hoặc tự truyền).
- `Username` là tên người chơi thực hiện giao dịch.
- `ItemId` là ID vật phẩm cần mua.
- `Amount` là số lượng cần mua (số nguyên, tối thiểu 1).
- `CurrencyType` là loại tiền tệ dùng để thanh toán (chuỗi).

```luau
PurchaseService.CreateOrder(Username, ItemId, Amount, CurrencyType)
PurchaseService.ConfirmOrder(OrderId)
PurchaseService.CancelOrder(OrderId)
PurchaseService.GetOrder(OrderId)
PurchaseService.GetOrderHistory(Username, Limit)
PurchaseService.Refund(OrderId)
PurchaseService.OnPurchase(Username, Callback)
```

---

## <img src="/Docs/studio/chatservice.png" alt="ChatService icon" width="28" style="vertical-align:-6px" /> ChatService

ChatService là 1 dịch vụ cho phép bạn quản lý kênh chat, bong bóng chat và lọc từ ngữ trong trò chơi.

**Lưu ý:**

- `ChannelName` là tên kênh chat (chuỗi).
- `Username` là tên người chơi.
- `Message` là nội dung tin nhắn (chuỗi).
- `FilterLevel` là mức độ lọc từ: `"None"`, `"Moderate"`, `"Strict"`.

```luau
ChatService.CreateChannel(ChannelName)
ChatService.DeleteChannel(ChannelName)
ChatService.JoinChannel(Username, ChannelName)
ChatService.LeaveChannel(Username, ChannelName)
ChatService.SendMessage(Username, ChannelName, Message)
ChatService.MutePlayer(Username, Duration)
ChatService.UnmutePlayer(Username)
ChatService.SetFilter(ChannelName, FilterLevel)
ChatService.ShowBubble(Username, Message, Duration)
ChatService.HideBubble(Username)
ChatService.OnMessage(ChannelName, Callback)
```

---

## <img src="/Docs/studio/badgeservice.png" alt="BadgeService icon" width="28" style="vertical-align:-6px" /> BadgeService

BadgeService là 1 dịch vụ cho phép bạn cấp huy hiệu hoặc danh hiệu cho người chơi trong trò chơi.

**Lưu ý:**

- `BadgeId` là ID định danh của huy hiệu (chuỗi).
- `Username` là tên người chơi.

```luau
BadgeService.Register(BadgeId, Name, Description)
BadgeService.Unregister(BadgeId)
BadgeService.Award(Username, BadgeId)
BadgeService.Revoke(Username, BadgeId)
BadgeService.HasBadge(Username, BadgeId)
BadgeService.GetBadges(Username)
BadgeService.GetAll()
BadgeService.OnAward(Username, Callback)
```

---

## <img src="/Docs/studio/currencyservice.png" alt="CurrencyService icon" width="28" style="vertical-align:-6px" /> CurrencyService

CurrencyService là 1 dịch vụ cho phép bạn quản lý ví tiền, tiền tệ chính và tiền tệ sự kiện của người chơi.

**Lưu ý:**

- `Username` là tên người chơi.
- `CurrencyType` là loại tiền tệ (chuỗi, ví dụ: `"Coin"`, `"Gem"`, `"EventToken"`).
- `Amount` là số lượng tiền (số thực, không được âm).

```luau
CurrencyService.GetBalance(Username, CurrencyType)
CurrencyService.Add(Username, CurrencyType, Amount)
CurrencyService.Deduct(Username, CurrencyType, Amount)
CurrencyService.Transfer(FromUsername, ToUsername, CurrencyType, Amount)
CurrencyService.SetBalance(Username, CurrencyType, Amount)
CurrencyService.GetAllBalances(Username)
CurrencyService.OnTransaction(Username, Callback)
```

---

## <img src="/Docs/studio/questservice.png" alt="QuestService icon" width="28" style="vertical-align:-6px" /> QuestService

QuestService là 1 dịch vụ cho phép bạn quản lý tiến trình nhiệm vụ và phần thưởng cho người chơi.

**Lưu ý:**

- `QuestId` là ID định danh của nhiệm vụ (chuỗi).
- `Username` là tên người chơi.
- `Progress` là tiến trình hiện tại của bước nhiệm vụ (số thực).
- `Rewards` là bảng phần thưởng khi hoàn thành (table).

```luau
QuestService.Register(QuestId, Name, Description, Steps, Rewards)
QuestService.Unregister(QuestId)
QuestService.Assign(Username, QuestId)
QuestService.Unassign(Username, QuestId)
QuestService.GetProgress(Username, QuestId)
QuestService.AddProgress(Username, QuestId, StepIndex, Delta)
QuestService.SetProgress(Username, QuestId, StepIndex, Progress)
QuestService.Complete(Username, QuestId)
QuestService.IsCompleted(Username, QuestId)
QuestService.GetActive(Username)
QuestService.OnComplete(Username, Callback)
```

---

## <img src="/Docs/studio/rewardservice.png" alt="RewardService icon" width="28" style="vertical-align:-6px" /> RewardService

RewardService là 1 dịch vụ cho phép bạn quản lý quà điểm danh, quà hàng ngày và quà mã code cho người chơi.

**Lưu ý:**

- `Username` là tên người chơi.
- `RewardId` là ID định danh của phần thưởng (chuỗi).
- `Code` là mã code đổi quà (chuỗi).
- `Rewards` là bảng phần thưởng (table).

```luau
RewardService.ClaimDaily(Username)
RewardService.GetDailyStreak(Username)
RewardService.ClaimLoginReward(Username, RewardId)
RewardService.RedeemCode(Username, Code)
RewardService.RegisterCode(Code, Rewards, MaxUses, ExpiresAt)
RewardService.RevokeCode(Code)
RewardService.GetCodeInfo(Code)
RewardService.OnClaim(Username, Callback)
```

---

## <img src="/Docs/studio/banservice.png" alt="BanService icon" width="28" style="vertical-align:-6px" /> BanService

BanService (hay PunishService) là 1 dịch vụ cho phép bạn cấm, trừng phạt hoặc kick người chơi vi phạm ra khỏi trò chơi.

**Lưu ý:**

- `Username` là tên người chơi.
- `Reason` là lý do xử phạt (chuỗi).
- `Duration` là thời gian ban tính bằng giây, truyền `-1` để ban vĩnh viễn.

```luau
BanService.Kick(Username, Reason)
BanService.Ban(Username, Reason, Duration)
BanService.Unban(Username)
BanService.IsBanned(Username)
BanService.GetBanInfo(Username)
BanService.GetBanHistory(Username, Limit)
BanService.Warn(Username, Reason)
BanService.GetWarnings(Username)
BanService.ClearWarnings(Username)
```

---

## <img src="/Docs/studio/matchmakingservice.png" alt="MatchmakingService icon" width="28" style="vertical-align:-6px" /> MatchmakingService

MatchmakingService là 1 dịch vụ cho phép bạn xếp hàng, ghép trận và phân chia phòng chơi cho người chơi.

**Lưu ý:**

- `Username` là tên người chơi.
- `QueueName` là tên hàng đợi (chuỗi).
- `RoomId` là ID định danh của phòng chơi (chuỗi).
- `MaxPlayers` là số người chơi tối đa trong phòng (số nguyên).

```luau
MatchmakingService.CreateQueue(QueueName, MaxPlayers)
MatchmakingService.DeleteQueue(QueueName)
MatchmakingService.Join(Username, QueueName)
MatchmakingService.Leave(Username, QueueName)
MatchmakingService.GetQueue(QueueName)
MatchmakingService.CreateRoom(RoomId, QueueName)
MatchmakingService.CloseRoom(RoomId)
MatchmakingService.GetRoom(RoomId)
MatchmakingService.GetRooms(QueueName)
MatchmakingService.OnMatch(QueueName, Callback)
```

---

## <img src="/Docs/studio/partyservice.png" alt="PartyService icon" width="28" style="vertical-align:-6px" /> PartyService

PartyService là 1 dịch vụ cho phép bạn tạo nhóm, mời bạn bè và tổ đội cùng chơi trong trò chơi.

**Lưu ý:**

- `PartyId` là ID định danh của nhóm (chuỗi).
- `Username` là tên người chơi.
- `Leader` là tên trưởng nhóm (chuỗi).
- `MaxSize` là số thành viên tối đa của nhóm (số nguyên).

```luau
PartyService.CreateParty(Leader, MaxSize)
PartyService.DisbandParty(PartyId)
PartyService.Invite(PartyId, Username)
PartyService.AcceptInvite(PartyId, Username)
PartyService.DeclineInvite(PartyId, Username)
PartyService.Kick(PartyId, Username)
PartyService.Leave(PartyId, Username)
PartyService.GetParty(PartyId)
PartyService.GetPartyOf(Username)
PartyService.TransferLeader(PartyId, Username)
PartyService.OnInvite(Username, Callback)
```

---

## <img src="/Docs/studio/dialogueservice.png" alt="DialogueService icon" width="28" style="vertical-align:-6px" /> DialogueService

DialogueService là 1 dịch vụ cho phép bạn xử lý hội thoại và lựa chọn câu trả lời khi người chơi tương tác với NPC.

**Lưu ý:**

- `DialogueId` là ID định danh của chuỗi hội thoại (chuỗi).
- `NpcName` là tên NPC sở hữu hội thoại.
- `Nodes` là bảng các nút hội thoại (table), mỗi nút gồm: `text`, `choices` và `callback` tùy chọn.
- `Username` là tên người chơi đang tương tác.

```luau
DialogueService.Register(DialogueId, NpcName, Nodes)
DialogueService.Unregister(DialogueId)
DialogueService.Start(Username, DialogueId)
DialogueService.End(Username)
DialogueService.GetCurrent(Username)
DialogueService.Choose(Username, ChoiceIndex)
DialogueService.OnStart(NpcName, Callback)
DialogueService.OnEnd(NpcName, Callback)
DialogueService.OnChoice(DialogueId, Callback)
```

---

## <img src="/Docs/studio/teleportservice.png" alt="TeleportService icon" width="28" style="vertical-align:-6px" /> TeleportService

TeleportService là 1 dịch vụ cho phép bạn dịch chuyển người chơi giữa các khu vực hoặc server trong trò chơi.

**Lưu ý:**

- `Username` là tên người chơi.
- `ZoneName` là tên khu vực đích (chuỗi).
- `ServerId` là ID của server đích (chuỗi, có thể nil để dùng server mặc định).
- `Position` là bảng tọa độ `{X, Y, Z}` điểm xuất hiện sau khi dịch chuyển (có thể nil).

```luau
TeleportService.TeleportToZone(Username, ZoneName, Position)
TeleportService.TeleportToServer(Username, ServerId, Position)
TeleportService.TeleportGroup(Usernames, ZoneName, Position)
TeleportService.RegisterZone(ZoneName, ServerId, SpawnPosition)
TeleportService.UnregisterZone(ZoneName)
TeleportService.GetZone(ZoneName)
TeleportService.GetZones()
TeleportService.OnTeleport(Username, Callback)
```

---

## <img src="/Docs/studio/cooldownservice.png" alt="CooldownService icon" width="28" style="vertical-align:-6px" /> CooldownService

CooldownService là 1 dịch vụ cho phép bạn quản lý thời gian hồi chiêu của kỹ năng, item và hành động trong trò chơi.

**Lưu ý:**

- `Username` là tên người chơi.
- `CooldownId` là ID định danh của cooldown (chuỗi, ví dụ: `"Skill_Fireball"`, `"Item_Potion"`).
- `Duration` là thời gian hồi chiêu tính bằng giây (số thực).

```luau
CooldownService.Start(Username, CooldownId, Duration)
CooldownService.Cancel(Username, CooldownId)
CooldownService.IsReady(Username, CooldownId)
CooldownService.GetRemaining(Username, CooldownId)
CooldownService.GetAll(Username)
CooldownService.Reset(Username, CooldownId)
CooldownService.OnReady(Username, CooldownId, Callback)
```

---

## <img src="/Docs/studio/effectservice.png" alt="EffectService icon" width="28" style="vertical-align:-6px" /> EffectService

EffectService là 1 dịch vụ cho phép bạn tạo hiệu ứng hình ảnh, hạt particle và hiệu ứng màn hình trong trò chơi.

**Lưu ý:**

- `EffectId` là ID định danh của hiệu ứng (chuỗi).
- `Position` là bảng tọa độ `{X, Y, Z}` nơi phát hiệu ứng.
- `Instance` là tên Part/Model để gắn hiệu ứng lên (có thể nil).
- `Username` là tên người chơi cần áp dụng hiệu ứng màn hình (truyền `"*"` cho tất cả).
- `Properties` là bảng thuộc tính của hiệu ứng (table, có thể nil).

```luau
EffectService.Emit(EffectId, Position, Properties)
EffectService.EmitOn(EffectId, Instance, Properties)
EffectService.Stop(EffectId)
EffectService.StopAll()
EffectService.ScreenEffect(Username, EffectType, Properties, Duration)
EffectService.ClearScreenEffect(Username)
```

---

## <img src="/Docs/studio/soundeffectservice.png" alt="SoundEffectService icon" width="28" style="vertical-align:-6px" /> SoundEffectService

SoundEffectService là 1 dịch vụ tối ưu hóa việc phát âm thanh hiệu ứng 3D tại vị trí cụ thể trong không gian trò chơi.

**Lưu ý:**

- `SoundId` là ID âm thanh trên Noobietoria Marketplace.
- `Position` là bảng tọa độ `{X, Y, Z}` nơi phát âm thanh.
- `Instance` là tên Part/Model để phát âm thanh tại vị trí đó (có thể nil nếu dùng Position).
- `Volume` là âm lượng (số thực, `0.0` đến `1.0`).
- `RolloffDistance` là khoảng cách tắt dần âm thanh (số thực, đơn vị studs).

```luau
SoundEffectService.Play(SoundId, Position, Volume, RolloffDistance)
SoundEffectService.PlayOn(SoundId, Instance, Volume, RolloffDistance)
SoundEffectService.Stop(SoundId)
SoundEffectService.SetVolume(SoundId, Volume)
SoundEffectService.Preload(SoundId)
SoundEffectService.Release(SoundId)
```

---

## <img src="/Docs/studio/marketplaceservice.png" alt="MarketplaceService icon" width="28" style="vertical-align:-6px" /> MarketplaceService

MarketplaceService là 1 dịch vụ quản lý vật phẩm trên chợ Noobietoria, cho phép bạn đăng bán, hạ bán và truy vấn thông tin vật phẩm.

**Lưu ý:**

- `ListingId` là ID định danh của bài đăng bán (chuỗi).
- `ItemId` là ID vật phẩm trên Noobietoria Marketplace.
- `Price` là giá bán (số thực, không được âm).
- `CurrencyType` là loại tiền tệ chấp nhận (chuỗi).
- `Seller` là tên người bán (chuỗi).

```luau
MarketplaceService.ListItem(Seller, ItemId, Price, CurrencyType, Quantity)
MarketplaceService.DelistItem(ListingId)
MarketplaceService.GetListing(ListingId)
MarketplaceService.GetListings(Seller)
MarketplaceService.Search(Query, Filters)
MarketplaceService.Buy(Username, ListingId, Quantity)
MarketplaceService.GetTransactionHistory(Username, Limit)
MarketplaceService.OnSale(Seller, Callback)
MarketplaceService.OnPurchase(Username, Callback)
```

---

## <img src="/Docs/studio/passservice.png" alt="PassService icon" width="28" style="vertical-align:-6px" /> PassService

PassService (hay GamepassService) là 1 dịch vụ kiểm tra và cấp quyền sở hữu đặc quyền/Gamepass cho người chơi.

**Lưu ý:**

- `PassId` là ID định danh của Gamepass (chuỗi).
- `Username` là tên người chơi.
- `Permissions` là bảng quyền hạn đi kèm Gamepass (table, có thể nil).

```luau
PassService.Register(PassId, Name, Description, Permissions)
PassService.Unregister(PassId)
PassService.Grant(Username, PassId)
PassService.Revoke(Username, PassId)
PassService.HasPass(Username, PassId)
PassService.GetPasses(Username)
PassService.GetAll()
PassService.CheckPermission(Username, Permission)
PassService.OnGrant(Username, Callback)
```

---

## <img src="/Docs/studio/tradingservice.png" alt="TradingService icon" width="28" style="vertical-align:-6px" /> TradingService

TradingService là 1 dịch vụ cho phép người chơi giao dịch trực tiếp với nhau bằng cách đề xuất, xem xét và xác nhận trao đổi vật phẩm hoặc tiền tệ.

**Lưu ý:**

- `TradeId` là ID định danh của phiên giao dịch (chuỗi, tự sinh).
- `FromUsername` và `ToUsername` là tên 2 người chơi tham gia giao dịch.
- `Offer` là bảng mô tả những gì mỗi bên đưa ra (table), gồm: `items` (table) và `currencies` (table).

```luau
TradingService.CreateTrade(FromUsername, ToUsername)
TradingService.CancelTrade(TradeId)
TradingService.SetOffer(TradeId, Username, Offer)
TradingService.GetOffer(TradeId, Username)
TradingService.Confirm(TradeId, Username)
TradingService.Decline(TradeId, Username)
TradingService.GetTrade(TradeId)
TradingService.GetTradeHistory(Username, Limit)
TradingService.OnTradeComplete(Username, Callback)
```

---

## <img src="/Docs/studio/mailservice.png" alt="MailService icon" width="28" style="vertical-align:-6px" /> MailService

MailService là 1 dịch vụ hòm thư ingame cho phép người chơi gửi và nhận tin nhắn cùng vật phẩm đính kèm.

**Lưu ý:**

- `MailId` là ID định danh của thư (chuỗi, tự sinh).
- `FromUsername` là người gửi, `ToUsername` là người nhận.
- `Subject` là tiêu đề thư (chuỗi).
- `Body` là nội dung thư (chuỗi).
- `Attachments` là bảng vật phẩm hoặc tiền tệ đính kèm (table, có thể nil).

```luau
MailService.Send(FromUsername, ToUsername, Subject, Body, Attachments)
MailService.GetInbox(Username, Limit)
MailService.GetOutbox(Username, Limit)
MailService.GetMail(MailId)
MailService.Read(MailId)
MailService.Delete(MailId)
MailService.ClaimAttachments(MailId, Username)
MailService.OnReceive(Username, Callback)
```

---

## <img src="/Docs/studio/craftingservice.png" alt="CraftingService icon" width="28" style="vertical-align:-6px" /> CraftingService

CraftingService là 1 dịch vụ cho phép người chơi chế tạo vật phẩm mới từ các nguyên liệu có trong túi đồ.

**Lưu ý:**

- `RecipeId` là ID định danh của công thức chế tạo (chuỗi).
- `Username` là tên người chơi thực hiện chế tạo.
- `Ingredients` là bảng nguyên liệu cần thiết (table), mỗi phần tử gồm: `ItemId` và `Quantity`.
- `Output` là bảng vật phẩm tạo ra (table), gồm: `ItemId`, `Quantity` và `Metadata` tùy chọn.

```luau
CraftingService.RegisterRecipe(RecipeId, Name, Ingredients, Output)
CraftingService.UnregisterRecipe(RecipeId)
CraftingService.GetRecipe(RecipeId)
CraftingService.GetAllRecipes()
CraftingService.CanCraft(Username, RecipeId)
CraftingService.Craft(Username, RecipeId, Quantity)
CraftingService.GetCraftHistory(Username, Limit)
CraftingService.OnCraft(Username, Callback)
```

---

## <img src="/Docs/studio/inputservice.png" alt="InputService icon" width="28" style="vertical-align:-6px" /> InputService

InputService là 1 dịch vụ cho phép bạn bắt sự kiện đầu vào từ phím, chuột và màn hình cảm ứng cho người chơi.

**Lưu ý:**

- `Username` là tên người chơi cần lắng nghe đầu vào.
- `KeyCode` là mã phím (chuỗi, ví dụ: `"E"`, `"Space"`, `"LeftShift"`).
- `MouseButton` là nút chuột: `"Left"`, `"Right"`, `"Middle"`.
- `ActionName` là tên định danh hành động tùy chỉnh (chuỗi).

```luau
InputService.OnKeyPress(Username, KeyCode, Callback)
InputService.OnKeyRelease(Username, KeyCode, Callback)
InputService.OnMouseClick(Username, MouseButton, Callback)
InputService.OnMouseMove(Username, Callback)
InputService.OnTouch(Username, Callback)
InputService.BindAction(ActionName, Keys, Callback)
InputService.UnbindAction(ActionName)
InputService.IsKeyDown(Username, KeyCode)
InputService.Remove(Username, KeyCode)
```

---

## <img src="/Docs/studio/pathfindingservice.png" alt="PathfindingService icon" width="28" style="vertical-align:-6px" /> PathfindingService

PathfindingService là 1 dịch vụ tìm đường và di chuyển tự động cho NPC trong không gian 3D của trò chơi.

**Lưu ý:**

- `NpcName` là tên NPC cần điều hướng.
- `Destination` là bảng tọa độ đích `{X, Y, Z}` hoặc tên Instance.
- `AgentRadius` và `AgentHeight` là kích thước của NPC để tính đường đi (số thực).
- `Waypoints` là bảng các tọa độ trung gian (table, tự sinh từ `ComputePath`).

```luau
PathfindingService.ComputePath(NpcName, Destination, AgentRadius, AgentHeight)
PathfindingService.MoveTo(NpcName, Destination)
PathfindingService.MoveAlongWaypoints(NpcName, Waypoints)
PathfindingService.Stop(NpcName)
PathfindingService.Pause(NpcName)
PathfindingService.Resume(NpcName)
PathfindingService.GetWaypoints(NpcName)
PathfindingService.OnReached(NpcName, Callback)
PathfindingService.OnBlocked(NpcName, Callback)
```

---

## <img src="/Docs/studio/proximityservice.png" alt="ProximityService icon" width="28" style="vertical-align:-6px" /> ProximityService

ProximityService là 1 dịch vụ tạo các điểm tương tác (prompt) khi người chơi đến gần một đối tượng trong trò chơi.

**Lưu ý:**

- `PromptId` là ID định danh của điểm tương tác (chuỗi).
- `Instance` là tên Part/Model gắn điểm tương tác.
- `MaxDistance` là khoảng cách tối đa để hiển thị (số thực, đơn vị studs).
- `KeyCode` là phím kích hoạt (chuỗi, mặc định `"E"`).
- `HoldDuration` là thời gian giữ phím để kích hoạt (số thực, `0` nếu nhấn ngay).

```luau
ProximityService.Create(PromptId, Instance, Label, KeyCode, MaxDistance, HoldDuration)
ProximityService.Remove(PromptId)
ProximityService.SetVisible(PromptId, Boolean)
ProximityService.SetEnabled(PromptId, Boolean)
ProximityService.GetPrompt(PromptId)
ProximityService.GetAllPrompts()
ProximityService.OnTriggered(PromptId, Callback)
ProximityService.OnEnter(PromptId, Callback)
ProximityService.OnExit(PromptId, Callback)
```

---

## <img src="/Docs/studio/localizationservice.png" alt="LocalizationService icon" width="28" style="vertical-align:-6px" /> LocalizationService

LocalizationService là 1 dịch vụ đa ngôn ngữ cho phép bạn quản lý và hiển thị nội dung theo ngôn ngữ của người chơi.

**Lưu ý:**

- `Locale` là mã ngôn ngữ theo chuẩn BCP 47 (chuỗi, ví dụ: `"vi"`, `"en"`, `"ja"`).
- `Key` là khóa định danh của chuỗi dịch (chuỗi).
- `Translations` là bảng ánh xạ `{Locale: TranslatedText}` (table).
- `Username` là tên người chơi (để lấy ngôn ngữ của từng người).

```luau
LocalizationService.Register(Key, Translations)
LocalizationService.Unregister(Key)
LocalizationService.Translate(Key, Locale)
LocalizationService.TranslateFor(Key, Username)
LocalizationService.GetLocale(Username)
LocalizationService.SetLocale(Username, Locale)
LocalizationService.GetSupportedLocales()
LocalizationService.ImportTable(Translations)
```

---

## <img src="/Docs/studio/physicsservice.png" alt="PhysicsService icon" width="28" style="vertical-align:-6px" /> PhysicsService

PhysicsService là 1 dịch vụ cho phép bạn điều khiển trọng lực, lực đẩy và các thuộc tính vật lý của đối tượng trong trò chơi.

**Lưu ý:**

- `Instance` là tên Part/Model cần điều chỉnh vật lý.
- `Gravity` là gia tốc trọng lực (số thực, đơn vị studs/s²).
- `Force` là bảng vector lực `{X, Y, Z}`.
- `Velocity` là bảng vector vận tốc `{X, Y, Z}`.

```luau
PhysicsService.SetGravity(Gravity)
PhysicsService.GetGravity()
PhysicsService.ApplyForce(Instance, Force, Duration)
PhysicsService.ApplyImpulse(Instance, Force)
PhysicsService.SetVelocity(Instance, Velocity)
PhysicsService.GetVelocity(Instance)
PhysicsService.SetAnchored(Instance, Boolean)
PhysicsService.SetMass(Instance, Mass)
PhysicsService.SetFriction(Instance, Friction)
PhysicsService.SetElasticity(Instance, Elasticity)
```

---

## <img src="/Docs/studio/textservice.png" alt="TextService icon" width="28" style="vertical-align:-6px" /> TextService

TextService là 1 dịch vụ cho phép bạn xử lý, đo đạc và định dạng nội dung văn bản hiển thị trong trò chơi (TextLabel, TextButton, TextBox, bong bóng chat, ...).

**Lưu ý:**

- `Text` là nội dung văn bản cần xử lý (chuỗi).
- `Username` là tên người chơi, dùng để lọc từ ngữ theo cài đặt của họ.
- `Font` là kiểu chữ (chuỗi, ví dụ: "Gotham", "Roboto", mặc định "Gotham").
- `FontSize` là cỡ chữ (số thực, mặc định 14).
- `MaxWidth` là chiều rộng tối đa để xuống dòng tự động (số thực, đơn vị pixel).
- `MaxLines` là số dòng hiển thị tối đa (số nguyên).
- `MeasureText` trả về bảng `{Width, Height}`, `WrapText` trả về bảng các dòng sau khi xuống dòng.

```luau
TextService.MeasureText(Text, Font, FontSize)
TextService.FitText(Text, MaxWidth, Font, FontSize)
TextService.WrapText(Text, MaxWidth, Font, FontSize)
TextService.Truncate(Text, MaxLines, Ellipsis)
TextService.Filter(Username, Text)
TextService.EscapeRichText(Text)
TextService.CountCharacters(Text)
```

---

## <img src="/Docs/studio/networkevent.png" alt="NetworkEvent icon" width="28" style="vertical-align:-6px" /> NetworkEvent

NetworkEvent là instance dùng để thiết lập kênh giao tiếp hai chiều giữa Client và Server (tương tự RemoteEvent/RemoteFunction trong Roblox).

> **Note:** NetworkEvent phải được đặt bên trong một `ServerScript` hoặc `ModuleScript`. Không thể khai báo tự do ngoài script context.

**Cách triển khai:**

- Tạo: `New -> NetworkEvent` rồi đặt tên định danh duy nhất cho event.
- Server lắng nghe: dùng `NetworkEvent:OnServerEvent(Callback)`
- Client gửi lên Server: dùng `NetworkEvent:FireServer(...)`
- Server gửi xuống Client: dùng `NetworkEvent:FireClient(Username, ...)` hoặc `NetworkEvent:FireAllClients(...)`
- Client lắng nghe: dùng `NetworkEvent:OnClientEvent(Callback)`

**Lưu ý:**

- `EventName` là tên của NetworkEvent (chuỗi, phải duy nhất toàn game).
- Dữ liệu truyền qua NetworkEvent phải serializable (string, number, boolean, table — không được truyền function hoặc userdata).
- Giới hạn kích thước payload: tối đa 64KB mỗi lần gọi.
- Không nên gọi `FireAllClients` quá 20 lần/giây để tránh nghẽn mạng.

---

## <img src="/Docs/studio/serverscript.png" alt="ServerScript icon" width="28" style="vertical-align:-6px" /> ServerScript

ServerScript là instance chứa code Luau chạy hoàn toàn trên máy chủ, không bao giờ được gửi hay thực thi ở phía client.

> **Note:** ServerScript chỉ có thể truy cập các Service phía server. Không thể gọi các API chỉ dành cho client (ví dụ: `CameraService`, `InputService`) trực tiếp trong ServerScript — phải thông qua `NetworkEvent`.

**Cách triển khai:**

- Tạo: `New -> ServerScript` rồi viết code Luau bên trong.
- ServerScript tự động chạy khi game khởi động (server-side).
- Có thể `require()` các `ModuleScript` để tái sử dụng logic.

**Lưu ý:**

- ServerScript có quyền truy cập toàn bộ các dịch vụ server (DataStoreService, PlayerService, BanService, ...).
- Biến và state trong ServerScript được chia sẻ giữa tất cả player đang kết nối.
- Script bị lỗi runtime sẽ dừng thực thi nhưng không làm sập server; lỗi sẽ được ghi vào Server Log.

---

## <img src="/Docs/studio/modulescript.png" alt="ModuleScript icon" width="28" style="vertical-align:-6px" /> ModuleScript

ModuleScript là instance chứa code Luau có thể được `require()` từ bất kỳ ServerScript hoặc ClientScript nào để tái sử dụng logic chung.

> **Note:** ModuleScript chỉ chạy một lần duy nhất — kết quả trả về được cache lại. Lần `require()` tiếp theo sẽ nhận cùng một giá trị mà không chạy lại.

**Cách triển khai:**

- Tạo: `New -> ModuleScript` rồi trả về một table hoặc giá trị ở cuối script.
- Dùng `require(ModuleName)` trong ServerScript hoặc ClientScript để nạp module.

```luau
-- Ví dụ ModuleScript tên "MathUtils"
local MathUtils = {}

function MathUtils.Add(a, b)
    return a + b
end

return MathUtils

-- Dùng trong ServerScript hoặc ClientScript:
local MathUtils = require("MathUtils")
local result = MathUtils.Add(3, 5) -- 8
```

**Lưu ý:**

- ModuleScript có thể `require()` các ModuleScript khác (không được tạo vòng lặp phụ thuộc).
- Nếu ModuleScript được `require()` từ cả Server lẫn Client, nó sẽ chạy riêng biệt trên mỗi môi trường — không chia sẻ state.
- Tên ModuleScript phải duy nhất trong phạm vi game.

---

## <img src="/Docs/studio/clientscript.png" alt="ClientScript icon" width="28" style="vertical-align:-6px" /> ClientScript

ClientScript là instance chứa code Luau chạy hoàn toàn trên máy của từng người chơi (client-side), không có quyền truy cập các dịch vụ server.

> **Note:** ClientScript không thể truy cập DataStoreService, BanService hay bất kỳ API server-only nào. Để tương tác với server, phải dùng `NetworkEvent`.

**Cách triển khai:**

- Tạo: `New -> ClientScript` rồi viết code Luau bên trong.
- ClientScript tự động chạy trên mỗi client khi player tham gia game.
- Có thể `require()` các `ModuleScript` để tái sử dụng logic.

**Lưu ý:**

- ClientScript có quyền truy cập các dịch vụ client (CameraService, UIService, InputService, EffectService, ...).
- Mỗi player có instance ClientScript riêng — state không chia sẻ giữa các client.
- Script bị lỗi trên client chỉ ảnh hưởng đến client đó, không ảnh hưởng server hay client khác.

---

## <img src="/Docs/studio/networkeventservice.png" alt="NetworkEventService icon" width="28" style="vertical-align:-6px" /> NetworkEventService

NetworkEventService là dịch vụ quản lý toàn bộ các NetworkEvent trong game, cho phép bạn đăng ký, gửi và lắng nghe sự kiện mạng giữa Client và Server bằng code Luau.

**Lưu ý:**

- `EventName` là tên NetworkEvent đã được tạo trong Explorer (chuỗi, phải duy nhất).
- `Username` là tên người chơi nhận event (chuỗi), truyền `"*"` để gửi cho tất cả client.
- `...` là các tham số dữ liệu tùy ý, phải serializable (string, number, boolean, table).
- `Callback` là hàm xử lý nhận dữ liệu từ event.
- Khi gọi từ **ServerScript**: dùng `FireClient`, `FireAllClients`, `OnServerEvent`.
- Khi gọi từ **ClientScript**: dùng `FireServer`, `OnClientEvent`.

```luau
-- ===== SERVER SIDE (ServerScript) =====

-- Lắng nghe event từ client gửi lên
NetworkEventService.OnServerEvent(EventName, function(Username, ...)
    -- xử lý dữ liệu từ client
end)

-- Gửi event xuống một client cụ thể
NetworkEventService.FireClient(EventName, Username, ...)

-- Gửi event xuống tất cả client
NetworkEventService.FireAllClients(EventName, ...)

-- Gửi event xuống tất cả client trừ một người
NetworkEventService.FireAllClientsExcept(EventName, ExcludeUsername, ...)


-- ===== CLIENT SIDE (ClientScript) =====

-- Lắng nghe event từ server gửi xuống
NetworkEventService.OnClientEvent(EventName, function(...)
    -- xử lý dữ liệu từ server
end)

-- Gửi event lên server
NetworkEventService.FireServer(EventName, ...)

-- Gọi server và chờ kết quả trả về (invoke)
NetworkEventService.InvokeServer(EventName, ...)

-- Lắng nghe invoke từ client (server xử lý và trả về)
NetworkEventService.OnServerInvoke(EventName, function(Username, ...)
    return result
end)


-- ===== CHUNG =====

-- Ngắt lắng nghe một event
NetworkEventService.Disconnect(EventName)

-- Kiểm tra event có tồn tại không
NetworkEventService.Exists(EventName)
```


