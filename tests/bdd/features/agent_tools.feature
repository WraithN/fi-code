Feature: Agent 工具调用
  Agent 应该能够根据用户请求调用不同的工具（write、read、bash、edit）

  Background:
    Given 一个配置了 Mock Provider 的后端服务

  Scenario: Agent 调用 write 工具写入文件
    When 用户发送消息 "帮我写一个 hello.rs 文件"
    Then Agent 应该调用 write 工具
    And 文件应该被写入到工作目录
    And 用户应该收到包含 "写入成功" 的响应

  Scenario: Agent 调用 read 工具读取文件
    Given 工作目录下存在文件 "test.txt"，内容为 "Hello World"
    When 用户发送消息 "读取 test.txt 的内容"
    Then Agent 应该调用 read 工具
    And 用户应该收到包含 "Hello World" 的响应

  Scenario: Agent 调用 bash 工具执行命令
    When 用户发送消息 "运行 ls 命令"
    Then Agent 应该调用 bash 工具
    And 用户应该收到命令执行结果

  Scenario: Agent 调用 edit 工具修改文件
    Given 工作目录下存在文件 "main.rs"，内容为 "fn main() {}"
    When 用户发送消息 "在 main.rs 中添加一行打印代码"
    Then Agent 应该调用 edit 工具
    And 文件内容应该包含修改后的代码
