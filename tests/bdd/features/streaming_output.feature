Feature: 流式输出与卡片渲染
  TUI 应该能够正确渲染 SSE 流式事件，包括不同类型的卡片

  Background:
    Given 一个配置了 Mock Provider 的后端服务
    And 一个初始化的 TUI 前端

  Scenario: 用户收到流式文本响应
    When 用户发送消息 "你好"
    Then 用户应该收到 Thinking 卡片
    And 用户应该看到流式的文本消息
    And 最终应该收到 Done 事件
    And 卡片状态应该从 thinking 变为 done

  Scenario: 用户收到工具调用卡片
    When 用户发送消息 "帮我写一个文件"
    Then 用户应该收到 ToolUse 卡片，显示工具名称和参数
    And 用户应该收到 ToolResult 卡片，显示执行结果
    And 最终结果卡片应该包含 "写入成功"

  Scenario: 用户收到 WriteFile 卡片
    When 用户发送消息 "写入配置文件"
    Then 用户应该收到 WriteFile 卡片，显示文件路径和内容摘要
    And 文件应该被实际写入

  Scenario: 长内容自动截断和展开
    When 用户发送消息 "生成一个很长的响应"
    Then 用户应该收到截断的内容提示
    When 用户点击展开按钮
    Then 用户应该看到完整的内容
