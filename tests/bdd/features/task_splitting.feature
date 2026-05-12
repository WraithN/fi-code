Feature: 任务拆分与执行
  Agent 应该能够将复杂任务拆分为多个子任务并串行执行

  Background:
    Given 一个配置了 Mock Provider 的后端服务

  Scenario: Agent 拆分复杂任务并生成任务计划
    When 用户发送消息 "帮我设计并实现一个 Web API"
    Then Agent 应该调用 handle_task_plan 工具
    And 任务计划应该包含至少 3 个子任务
    And 每个子任务应该有唯一 ID 和描述

  Scenario: Agent 串行执行子任务并汇总结果
    Given 用户已发送复杂任务 "帮我设计并实现一个 Web API"
    And Agent 已生成任务计划
    When Agent 开始执行任务计划
    Then 子任务应该被串行执行
    And 每个子任务完成后状态应该更新为 done
    And 用户应该收到所有子任务的执行结果汇总

  Scenario: 任务执行失败时 Agent 报告错误
    Given 用户已发送复杂任务 "执行一个注定失败的任务"
    And Agent 已生成任务计划
    When 某个子任务执行失败
    Then Agent 应该报告错误信息
    And 任务状态应该更新为 error
    And 用户应该收到包含错误详情的响应
