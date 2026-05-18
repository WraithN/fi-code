Feature: Agent 扩展工具调用
  Agent 应该能够调用更多类型的工具（grep、glob、git_status、git_log）

  Background:
    Given 一个配置了 Mock Provider 的后端服务

  Scenario: Agent 调用 grep 工具搜索代码
    When 用户发送消息 "grep 搜索 fn main"
    Then Agent 应该调用 grep 工具
    And 工具结果应该非空

  Scenario: Agent 调用 glob 工具查找文件
    When 用户发送消息 "glob 查找 .rs 文件"
    Then Agent 应该调用 glob 工具
    And 工具结果应该非空

  Scenario: Agent 调用 git_status 工具查看仓库状态
    When 用户发送消息 "查看 git 状态"
    Then Agent 应该调用 git_status 工具
    And 工具结果应该非空

  Scenario: Agent 调用 git_log 工具查看提交历史
    When 用户发送消息 "查看 git 日志"
    Then Agent 应该调用 git_log 工具
    And 工具结果应该非空
