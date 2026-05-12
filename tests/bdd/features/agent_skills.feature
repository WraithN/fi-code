Feature: Agent 技能调用
  Agent 应该能够发现和使用已注册的技能来辅助完成任务

  Background:
    Given 一个配置了 Mock Provider 的后端服务

  Scenario: Agent 使用 commit 技能生成提交信息
    Given 系统中已注册 commit 技能
    When 用户发送消息 "帮我写提交信息"
    Then Agent 应该调用 use_skill 工具
    And 技能名称应该为 "commit"
    And 技能内容应该被注入到对话上下文中
    And 用户应该收到包含 "提交信息" 的响应

  Scenario: Agent 使用 code-review 技能进行代码审查
    Given 系统中已注册 code-review 技能
    And 工作目录下存在文件 "bad_code.rs"，内容为 "fn main() { bad } "
    When 用户发送消息 "审查这段代码"
    Then Agent 应该调用 use_skill 工具
    And 技能名称应该为 "code-review"
    And 审查结果应该包含具体的改进建议
