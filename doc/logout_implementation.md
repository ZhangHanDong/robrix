# Robrix Logout功能实现文档

## 概述

Robrix的登出（Logout）功能通过状态机模式实现，确保了登出过程的可靠性和用户体验。该实现包含了完整的错误处理、进度反馈、以及在关键步骤失败时的恢复机制。

## 核心组件

### 1. LogoutStateMachine (logout_state_machine.rs)

状态机是登出流程的核心控制器，负责管理整个登出过程的状态转换和执行。

#### 主要状态（LogoutState）

```rust
pub enum LogoutState {
    Idle,                      // 初始状态
    PreChecking,              // 检查前置条件
    StoppingSyncService,      // 停止同步服务
    LoggingOutFromServer,     // 服务器端登出
    PointOfNoReturn,          // 无法回退点（已失效会话）
    ClosingTabs,              // 关闭UI标签（仅桌面端）
    CleaningAppState,         // 清理应用状态
    ShuttingDownTasks,        // 关闭后台任务
    RestartingRuntime,        // 重启Matrix运行时
    Completed,                // 登出完成
    Failed(LogoutError),      // 登出失败
}
```

#### 配置参数（LogoutConfig）

```rust
pub struct LogoutConfig {
    tab_close_timeout: Duration,           // 关闭标签页超时（10秒）
    app_state_cleanup_timeout: Duration,   // 清理应用状态超时（5秒）
    server_logout_timeout: Duration,       // 服务器登出超时（60秒）
    allow_cancellation: bool,              // 是否允许取消
    is_desktop: bool,                      // 是否为桌面模式
}
```

#### 执行流程

1. **预检查阶段** (10%)
   - 验证client存在
   - 验证sync service存在
   - 验证access token存在

2. **停止同步服务** (20%)
   - 调用sync_service.stop()
   - 确保后续登出不会有新的同步数据

3. **服务器登出** (30%)
   - 调用client.matrix_auth().logout()
   - 处理超时情况
   - 特殊处理M_UNKNOWN_TOKEN错误（token已失效）

4. **无法回退点** (50%)
   - 设置LOGOUT_POINT_OF_NO_RETURN标志
   - 删除保存的用户ID
   - 此后的失败将导致应用需要重启

5. **关闭标签页** (60%) - 仅桌面端
   - 发送CloseAllTabs action
   - 等待UI确认所有标签已关闭

6. **清理应用状态** (70%)
   - 泄漏资源以防止deadpool panic
   - 清理各种全局集合
   - 通知UI清理其状态

7. **关闭后台任务** (80%)
   - 调用shutdown_background_tasks()

8. **重启运行时** (90%)
   - 调用start_matrix_tokio()
   - 重新初始化Tokio运行时

9. **完成** (100%)
   - 发送LogoutSuccess action
   - 重置LOGOUT_IN_PROGRESS标志

### 2. LogoutConfirmModal (logout_confirm_modal.rs)

用户界面组件，提供登出确认对话框和进度反馈。

#### 主要功能

- 显示确认对话框
- 显示登出进度和百分比
- 处理用户取消操作
- 在无法回退后显示重启提示

#### 状态管理

```rust
pub struct LogoutConfirmModal {
    view: View,
    final_success: Option<bool>,  // None: 进行中, Some(true): 成功, Some(false): 失败
}
```

#### Action处理

- `LogoutAction::ProgressUpdate`: 更新进度信息
- `LogoutAction::LogoutSuccess`: 显示成功消息
- `LogoutAction::LogoutFailure`: 显示错误信息
- `LogoutAction::ApplicationRequiresRestart`: 显示重启提示

### 3. 错误处理 (logout_errors.rs)

分层的错误类型设计，区分可恢复和不可恢复错误。

#### 错误分类

```rust
pub enum LogoutError {
    Recoverable(RecoverableError),      // 可恢复，应用可继续使用
    Unrecoverable(UnrecoverableError),  // 不可恢复，需要重启应用
}
```

#### 可恢复错误
- `NoAccessToken`: 无访问令牌
- `ServerLogoutFailed`: 服务器登出失败
- `Timeout`: 操作超时
- `Cancelled`: 用户取消

#### 不可恢复错误
- `ClientMissing`: 客户端缺失
- `SyncServiceMissing`: 同步服务缺失
- `PostPointOfNoReturnFailure`: 无法回退点后的失败
- `RuntimeRestartFailed`: 运行时重启失败

### 4. 集成点

#### App.rs中的处理

```rust
// 处理登出成功
if let Some(LogoutAction::LogoutSuccess) = action.downcast_ref() {
    self.show_login_screen(cx);
}

// 处理清理应用状态
if let Some(LogoutAction::CleanAppState { on_clean_appstate }) = action.downcast_ref() {
    // 清理用户资料缓存，防止线程局部析构函数问题
    crate::profile::user_profile_cache::clear_cache();
    // 重置保存的dock状态，防止切换回桌面模式时崩溃
    self.app_state.saved_dock_state = Default::default();
    let _ = on_clean_appstate.send(true);
}
```

#### sliding_sync.rs中的入口

```rust
MatrixRequest::Logout { is_desktop } => {
    match logout_with_state_machine(is_desktop).await {
        Ok(_) => log!("Logout completed successfully"),
        Err(e) => error!("Logout failed: {}", e),
    }
}
```

## 特殊设计考虑

### 1. 资源泄漏处理

#### Logout时的资源清理
在正常logout流程中，资源会被正常释放，不再使用`std::mem::forget`：

```rust
// 正常清理资源，允许它们被正确drop
CLIENT.lock().unwrap().take();
log!("Client cleared during logout");

SYNC_SERVICE.lock().unwrap().take();
log!("Sync service cleared during logout");

REQUEST_SENDER.lock().unwrap().take();
log!("Request sender cleared during logout");
```

这样可以避免用户logout后重新登录时的内存泄漏问题。

#### 程序退出时的特殊处理
只有在程序真正退出时，才会故意泄漏资源以避免deadpool panic：

```rust
// 在 cleanup_before_shutdown 中
// 先清理用户资料缓存，防止线程局部析构函数问题
crate::profile::user_profile_cache::clear_cache();

// 然后泄漏资源以防止deadpool panic
if let Some(client) = CLIENT.lock().unwrap().take() {
    std::mem::forget(client);
}
```

#### 清理顺序的重要性
1. 必须先清理依赖Tokio runtime的资源（如用户资料缓存）
2. 然后才能泄漏Tokio runtime本身
3. 这个顺序很关键，否则会导致析构函数在没有runtime的情况下执行异步操作

### 2. 无法回退点（Point of No Return）

一旦服务器登出成功或token失效，设置全局标志：
- `LOGOUT_POINT_OF_NO_RETURN`
- `LOGOUT_IN_PROGRESS`

这确保了即使后续步骤失败，应用也知道需要重启。

### 3. 进度反馈机制

通过`LogoutProgress`结构提供详细的进度信息：
- 当前状态
- 消息文本
- 完成百分比
- 开始时间和步骤时间

### 4. 桌面端特殊处理

桌面端需要额外处理标签页关闭：
- 发送`MainDesktopUiAction::CloseAllTabs`
- 等待UI确认所有标签已关闭
- 设置超时保护（10秒）

## 用户交互流程

1. **用户点击登出按钮**
   - 在spaces_dock.rs中触发`LogoutConfirmModalAction::Open`

2. **显示确认对话框**
   - 用户可以选择"确认"或"取消"

3. **执行登出流程**
   - 显示进度百分比
   - 禁用按钮防止重复操作

4. **处理结果**
   - 成功：显示成功消息，点击"关闭"返回登录界面
   - 失败（可恢复）：显示错误信息，可以点击"确定"继续使用
   - 失败（不可恢复）：显示重启提示，提供"立即重启"和"稍后重启"选项

## 安全性考虑

1. **Token处理**
   - 确保服务器端token被正确注销
   - 本地存储的token被清理

2. **用户数据**
   - 清理用户profile缓存
   - 清理临时文件
   - 不保存敏感信息

3. **状态一致性**
   - 使用原子操作标志
   - 确保状态机的原子性转换
   - 防止并发登出操作

## 测试要点

1. **正常流程测试**
   - 完整的登出流程
   - 进度更新显示

2. **异常流程测试**
   - 网络断开时的登出
   - Token已失效的处理
   - 各阶段超时处理

3. **取消操作测试**
   - 在无法回退点前取消
   - 确认取消后应用状态正常

4. **重启流程测试**
   - 不可恢复错误后的重启
   - 重启后能正常登录

5. **内存泄漏测试**
   - 多次logout和login循环，确保没有内存泄漏
   - 不同模式（桌面/移动）切换下的logout测试
   - 程序退出时的资源清理验证

## 未来改进建议

1. **遥测数据收集**
   - 利用LogoutTelemetry收集性能数据
   - 分析各步骤耗时

2. **离线登出支持**
   - 增强离线状态下的登出处理
   - 本地清理与服务器同步分离

3. **批量账号支持**
   - 支持多账号切换
   - 选择性登出特定账号

4. **恢复机制增强**
   - 更细粒度的错误恢复
   - 自动重试机制