Feature: 斜杠命令
  用户应该能够通过斜杠命令与系统进行交互

  Background:
    Given 一个运行中的后端服务

  Scenario: 用户切换模型
    Given 系统中配置了多个模型
    When 用户发送命令 "/model"
    Then 系统应该返回可用模型列表
    When 用户选择模型 "gpt-4o"
    Then 当前模型应该切换为 "gpt-4o"
    And 用户应该收到确认消息

  Scenario: 用户初始化项目
    Given 工作目录是空目录
    When 用户发送命令 "/init"
    Then 系统应该在根目录创建 AGENTS.md 文件
    And AGENTS.md 应该包含项目基本信息模板
    And 用户应该收到初始化完成的确认

  Scenario: 用户查看帮助信息
    When 用户发送命令 "/help"
    Then 系统应该返回所有可用命令的列表
    And 每个命令应该包含简要说明
