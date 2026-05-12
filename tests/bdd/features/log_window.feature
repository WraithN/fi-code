Feature: 日志窗口
  用户应该能够通过快捷键查看系统日志

  Background:
    Given 一个运行中的后端服务
    And 一个初始化的 TUI 前端

  Scenario: 用户打开日志窗口
    Given TUI 当前处于正常聊天界面
    When 用户按下 Ctrl+L
    Then 日志窗口应该显示
    And 日志窗口应该显示历史日志信息

  Scenario: 日志窗口实时接收 SSE 日志
    Given 日志窗口已打开
    When 后端发送新的日志事件
    Then 日志窗口应该实时更新并显示新日志
    And 日志窗口应该自动滚动到最新日志

  Scenario: 用户关闭日志窗口
    Given 日志窗口已打开
    When 用户按下 Ctrl+L
    Then 日志窗口应该关闭
    And TUI 应该返回正常聊天界面

  Scenario: 连接断开时显示提示横幅
    Given 日志窗口已打开
    When 后端连接断开
    Then 日志窗口应该显示断开连接横幅
    And 日志内容应该保留不变
