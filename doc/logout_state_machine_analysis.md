# Logout 状态机执行流程和数据流解析

## 一、整体架构

### 1. 触发机制
```
用户点击logout按钮 
    ↓
spaces_dock.rs 发送 LogoutConfirmModalAction::Open
    ↓
显示确认对话框
    ↓
用户确认后发送 MatrixRequest::Logout
    ↓
sliding_sync.rs 调用 logout_with_state_machine()
```

### 2. 核心组件
- **LogoutStateMachine**: 状态机本身，管理状态转换和执行
- **LogoutState**: 定义所有可能的状态
- **LogoutProgress**: 进度信息载体
- **LogoutAction**: UI和状态机之间的通信协议

## 二、状态转换流程

### 状态机的完整状态流：

```
Idle (0%)
  ↓
PreChecking (10%) - 验证前置条件
  ↓
StoppingSyncService (20%) - 停止同步服务
  ↓
LoggingOutFromServer (30%) - 服务器端登出
  ↓
PointOfNoReturn (50%) - 不可回退点 ⚠️
  ↓
ClosingTabs (60%) - 关闭标签页(仅桌面端)
  ↓
CleaningAppState (70%) - 清理应用状态
  ↓
ShuttingDownTasks (80%) - 关闭后台任务
  ↓
RestartingRuntime (90%) - 重启Matrix运行时
  ↓
Completed (100%) - 完成

失败时 → Failed(LogoutError)
```

### 每个状态的具体操作：

1. **PreChecking (预检查)**
   - 验证CLIENT存在
   - 验证SYNC_SERVICE存在
   - 验证access_token存在
   - 失败类型：可恢复错误

2. **StoppingSyncService (停止同步)**
   - 调用 `sync_service.stop()`
   - 防止新的同步数据进入

3. **LoggingOutFromServer (服务器登出)**
   - 调用 `client.matrix_auth().logout()`
   - 特殊处理：M_UNKNOWN_TOKEN错误（token已失效）视为成功
   - 超时时间：60秒

4. **PointOfNoReturn (不可回退点)** ⭐
   - 设置 `LOGOUT_POINT_OF_NO_RETURN = true`
   - 删除保存的用户ID
   - **此后的失败都是不可恢复的**

5. **ClosingTabs (关闭标签页)**
   - 仅桌面端执行
   - 发送 `MainDesktopUiAction::CloseAllTabs`
   - 等待UI确认（通过oneshot channel）
   - 超时时间：10秒

6. **CleaningAppState (清理应用状态)**
   - 清理全局资源（不再泄露）
   - 清理集合：TOMBSTONED_ROOMS、IGNORED_USERS、ALL_JOINED_ROOMS
   - 通知UI清理（清理用户资料缓存）
   - 超时时间：5秒

7. **ShuttingDownTasks (关闭后台任务)**
   - 调用 `shutdown_background_tasks()`

8. **RestartingRuntime (重启运行时)**
   - 调用 `start_matrix_tokio()`
   - 为下次登录准备新的运行时

## 三、数据流和通信机制

### 1. 状态机与UI的通信

**进度更新流程**：
```
LogoutStateMachine 
    ↓ (每次状态转换)
send_progress_update()
    ↓
Cx::post_action(LogoutAction::ProgressUpdate)
    ↓
App::handle_actions() 接收
    ↓
转发给 LogoutConfirmModal
    ↓
更新UI显示（进度条、消息文本）
```

**关键数据结构**：
```rust
LogoutProgress {
    state: LogoutState,        // 当前状态
    message: String,          // 显示消息
    percentage: u8,           // 完成百分比
    started_at: Instant,      // 开始时间
    step_started_at: Instant, // 当前步骤开始时间
}
```

### 2. 异步通信机制

**CloseAllTabs 示例**：
```rust
// 1. 创建oneshot channel
let (tx, rx) = oneshot::channel::<bool>();

// 2. 发送Action携带发送端
Cx::post_action(MainDesktopUiAction::CloseAllTabs { 
    on_tabs_closed: tx 
});

// 3. 等待UI响应（带超时）
tokio::time::timeout(
    self.config.tab_close_timeout,
    rx
).await
```

**CleanAppState 示例**：
```rust
// 状态机发送
LogoutAction::CleanAppState { on_clean_appstate: tx }
    ↓
// App.rs 处理
clear_cache();  // 清理用户资料缓存
self.app_state.saved_dock_state = Default::default();
on_clean_appstate.send(true);  // 通知完成
```

### 3. 全局状态标志

**两个关键的原子标志**：
- `LOGOUT_IN_PROGRESS`: 防止并发logout
- `LOGOUT_POINT_OF_NO_RETURN`: 标记不可恢复状态

使用场景：
```rust
// 检查是否可以开始logout
if LOGOUT_IN_PROGRESS.load(Ordering::Relaxed) {
    return Err("Logout already in progress");
}

// 设置标志
LOGOUT_IN_PROGRESS.store(true, Ordering::Relaxed);
```

### 4. 错误处理和恢复

**错误分类**：
```rust
enum LogoutError {
    Recoverable(RecoverableError),      // 应用可继续使用
    Unrecoverable(UnrecoverableError),  // 需要重启应用
}
```

**错误传播**：
```
状态机检测到错误
    ↓
transition_to(Failed(error))
    ↓
LogoutAction::LogoutFailure { error, is_unrecoverable }
    ↓
LogoutConfirmModal 显示错误
    ↓
根据错误类型显示不同按钮（确定 vs 重启）
```

## 四、关键设计要点

### 1. **状态机模式的优势**
- **可预测性**：每个状态的转换都是明确定义的
- **可恢复性**：在Point of No Return之前可以处理失败
- **进度可视化**：每个状态对应明确的完成百分比
- **解耦设计**：UI和业务逻辑通过Action分离

### 2. **Point of No Return 设计**
- **位置选择**：放在服务器登出成功后（50%）
- **意义**：一旦token失效，即使后续步骤失败也必须完成logout
- **处理策略**：此后的错误都是Unrecoverable，需要重启应用

### 3. **资源清理策略**
- **正常logout**：资源正常drop，避免内存泄露
- **程序退出**：故意泄露以避免deadpool panic
- **清理顺序**：先清理依赖runtime的资源，再处理runtime本身

### 4. **异步协调机制**
- **oneshot channel**：用于等待UI操作完成
- **超时保护**：每个需要等待的操作都有超时
- **Action系统**：实现异步任务和UI线程的通信

### 5. **容错设计**
- **Token失效处理**：M_UNKNOWN_TOKEN错误视为成功
- **桌面/移动适配**：ClosingTabs仅在桌面端执行
- **取消支持**：在Point of No Return前可以取消

### 6. **完整的生命周期管理**
```
开始 → 预检查 → 执行logout → 清理资源 → 重启准备 → 完成
         ↓                      ↓
      可恢复失败            不可恢复失败
         ↓                      ↓
      返回应用              提示重启
```

## 数据流总结

1. **控制流**：`sliding_sync` → `LogoutStateMachine` → `App` → `LogoutConfirmModal`
2. **状态流**：通过 `LogoutProgress` 和 `LogoutAction` 传递
3. **同步机制**：oneshot channel 实现异步等待
4. **全局状态**：原子标志确保状态一致性
5. **错误流**：分层错误处理，区分可恢复和不可恢复

这个设计确保了logout过程的可靠性、可视性和用户体验的流畅性。