# Workflow Engine - User Guide

🚀 **高性能、可扩展的分布式工作流引擎**

## ✨ 核心特性 (Key Features)
- **分布式执行**: 支持多机并行执行，线性扩展性能 (New!)
- **高性能**: 基于 Rust 构建，极低的资源占用
- **多语言支持**: 内置 Python/JavaScript 脚本执行
- **丰富的节点**: 支持 HTTP, LLM, Shell, File, Loop 等 11 种节点
- **灵活部署**: 支持 CLI 单机运行、Webhook 服务模式、分布式集群模式


## 如何运行 (How to Run)

### 1. 构建项目 (Build the project)
```bash
cd /Users/hejl/projects/dsl/workflow-engine
cargo build
```

### 2. 运行工作流 (Run a workflow)
```bash
# 使用示例工作流
cargo run -- -f examples/example.yaml

# 使用日志输出
RUST_LOG=info cargo run -- -f examples/example.yaml

# 运行自定义工作流
cargo run -- -f /path/to/your/workflow.yaml

# 传递输入参数 (覆盖全局变量)
cargo run -- -f examples/example.yaml -i message="Hello from CLI" -i count=42
```

### 3. 发布构建 (Release build - optimized)
```bash
cargo build --release
./target/release/workflow-engine -f examples/example.yaml
```

### 4. Webhook 服务模式 (Server Mode)
启动 HTTP 服务器，通过 API 触发工作流：

```bash
# 启动服务器 (默认端口 3000)
cargo run -- serve --port 3000
```

**触发工作流**:
```bash
curl -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d '{
    "file": "examples/example.yaml",
    "inputs": {
      "message": "Hello from Webhook",
      "count": 100
    }
  }'
```

**响应**:
```json
{
  "status": "success",
  "outputs": {
    "node_id": { ... }
  },
  "error": null
}
```

### 5. 分布式执行模式 (Distributed Mode) 🚀
支持多机分布式执行，显著提升大规模工作流的执行效率。

**架构**:
- **Coordinator**: 负责任务调度和状态管理
- **Worker**: 负责执行具体任务 (支持水平扩展)

**启动步骤**:

1. **启动 Coordinator**:
```bash
./target/release/workflow-engine coordinator -p 8080
```

2. **启动 Workers** (可以在不同机器上):
```bash
# Worker 1
./target/release/workflow-engine worker -i worker-1 -p 3001 -c http://localhost:8080

# Worker 2
./target/release/workflow-engine worker -i worker-2 -p 3002 -c http://localhost:8080
```

3. **提交工作流**:
```bash
./target/release/workflow-engine submit -f benchmarks/distributed_test.yaml -c http://localhost:8080
```

**性能优势**:
- 真正的并行执行
- 线性扩展能力
- 适合大规模并发任务 (如批量爬虫、数据处理)


## YAML 工作流格式 (Workflow Format)

### 基础结构
```yaml
name: "工作流名称"
version: "1.0"

# 全局变量 (所有节点可访问)
global:
  key1: "value1"
  key2: 123

nodes:
  - id: "唯一ID"
    type: "节点类型"
    name: "显示名称"
    needs: ["依赖的节点ID"]  # 可选，为空表示初始节点
    params:
      # 节点特定参数
```

### 支持的节点类型

目前支持 **12 种节点类型**：
- **Shell**: 执行系统命令
- **HTTP**: 发送 HTTP 请求
- **Delay**: 延迟执行
- **Switch**: 条件判断
- **Script**: 嵌入式脚本 (Python/JavaScript)
- **LLM**: AI 大语言模型调用 (OpenAI API)
- **Transform**: JSON 数据提取和转换 (JSONPath)
- **File**: 文件读写操作
- **Loop**: 循环迭代 (ForEach)
- **Input**: 交互式用户输入
- **Assign**: 更新全局变量
- **MCP**: Model Context Protocol 工具调用

#### Shell 节点
执行系统命令
```yaml
- id: "my_shell"
  type: "shell"
  name: "执行脚本"
  params:
    command: "echo 'Hello' && ls -la"
```

#### HTTP 节点
发送HTTP请求
```yaml
- id: "api_call"
  type: "http"
  name: "调用API"
  params:
    method: "GET"  # or POST
    url: "https://api.example.com/data"
    body:  # POST时使用
      key: "value"
```

#### Delay 节点
等待指定时间
```yaml
- id: "wait"
  type: "delay"
  name: "等待"
  params:
    milliseconds: 1000  # 等待1秒
```

#### Switch 节点 (条件判断)
根据条件选择不同的值或路径
```yaml
- id: "condition"
  type: "switch"
  name: "条件判断"
  params:
    condition: "{{ nodes.fetch.output.status }} > 100"
    true_value: "高分处理"
    false_value: "常规处理"
```

**支持的条件运算符**:
- `==` (等于), `!=` (不等于)
- `>` (大于), `<` (小于)
- `>=` (大于等于), `<=` (小于等于)
- `true`, `false` (布尔字面量)

#### Script 节点 (嵌入式脚本)
执行 Python 或 JavaScript 脚本
```yaml
- id: "data_process"
  type: "script"
  name: "数据处理"
  params:
    language: "python"  # or "javascript", "js", "node"
    script: |
      import json
      data = {"result": 42}
      print(json.dumps(data))
```

**支持的语言**:
- `python` / `python3` (需要安装 Python 3)
- `javascript` / `js` / `node` (需要安装 Node.js)

#### 6. LLM 节点 (AI调用)
支持调用 OpenAI API 或兼容服务
```yaml
- id: "ai_analyze"
  type: "llm"
  name: "AI分析"
  params:
    model: "gpt-4"  # or "gpt-3.5-turbo"
    system: "你是数据分析专家"
    prompt: "分析这些数据：{{ nodes.fetch.output }}"
    temperature: 0.7  # 可选，默认0.7
    max_tokens: 500   # 可选
```

**配置**:
- 需要设置环境变量 `OPENAI_API_KEY`
- 或创建 `.env` 文件（参考 `.env.example`）
- 支持自定义 `base_url` 参数使用兼容服务

**输出格式**:
```json
{
  "content": "AI生成的文本",
  "model": "gpt-4",
  "usage": {
    "prompt_tokens": 100,
    "completion_tokens": 50,
    "total_tokens": 150
  }
}
```

#### 7. Transform 节点 (数据转换)
使用 JSONPath 提取和转换 JSON 数据
```yaml
- id: "extract"
  type: "transform"
  name: "提取数据"
  params:
    input: "{{ nodes.api.output.body }}"
    # 单字段提取
    path: "$.data.users[*].name"
    
    # 或多字段提取
    extract:
      names: "$.users[*].name"
      emails: "$.users[*].email"
```

**JSONPath 语法示例**:
- `$.data` - 获取 data 字段
- `$.users[0]` - 第一个用户
- `$.users[*].name` - 所有用户的 name
- `$[0:3]` - 前3个元素

#### 8. File 节点 (文件操作)
读写文件，持久化数据
```yaml
# 写文件
- id: "save"
  type: "file"
  params:
    operation: "write"  # or "read", "append"
    path: "/tmp/result.json"
    content: "{{ nodes.process.output }}"

# 读文件
- id: "load"
  type: "file"
  params:
    operation: "read"
    path: "./config.json"
```

#### 9. Loop 节点 (循环)
对数组中的每个元素执行相同的子工作流
```yaml
- id: "batch_process"
  type: "loop"
  name: "批量处理"
  params:
    items: "{{ nodes.fetch.output.urls }}"  # 数组
    steps:
      - id: "process_item"
        type: "shell"
        params:
          command: "echo 'Processing {{ loop.item }} at index {{ loop.index }}'"
      
      - id: "save_item"
        type: "file"
        needs: ["process_item"]
        params:
          operation: "write"
          path: "/tmp/item_{{ loop.index }}.txt"
          content: "{{ loop.item }}"
```

**Loop 上下文变量**:
- `{{ loop.item }}` - 当前迭代的元素
- `{{ loop.index }}` - 当前索引 (从0开始)
- `{{ loop.total }}` - 总元素数量

**输出格式**:
```json
{
  "iterations": [
    {"process_item": {...}, "save_item": {...}},
    {"process_item": {...}, "save_item": {...}}
  ],
  "count": 2
}
```

#### 10. Input 节点 (Human in the loop)
在工作流执行过程中暂停，等待用户输入
```yaml
- id: "ask_name"
  type: "input"
  name: "询问用户名"
  params:
    prompt: "请输入你的名字:"
    default: "Guest"  # 可选，按回车使用默认值
```

**输出**: 用户输入的字符串
```json
"Alice"
```

#### 11. Assign 节点 (更新全局变量)
在工作流执行过程中更新全局变量，支持设置和追加操作
```yaml
- id: "set_var"
  type: "assign"
  params:
    assignments:
      - key: "count"
        value: 1
        mode: "set"
```

#### 12. MCP 节点 (Model Context Protocol)
调用支持 MCP 协议的服务器工具。支持 stdio 通信模式。
```yaml
- id: "call_tool"
  type: "mcp"
  params:
    server:
      command: "npx"
      args: ["-y", "@modelcontextprotocol/server-filesystem", "/Users/me/files"]
    tool: "read_file"
    arguments:
      path: "/Users/me/files/test.txt"
```
- id: "update_vars"
  type: "assign"
  name: "更新变量"
  params:
    assignments:
      - key: "counter"
        value: "42"
        mode: "set"  # 默认为 "set"
      
      - key: "results"
        value: "{{ nodes.process.output }}"
        mode: "append"  # 追加到数组
```

**支持的模式**:
- `set`: 设置变量值（覆盖）
- `append`: 将值追加到数组（如果变量不是数组，操作会被忽略）

**输出**: 返回所有更新后的变量值
```json
{
  "counter": 42,
  "results": [...]
}
```

### 变量引用 (Variable Substitution)

在 `params` 中使用 `{{ }}` 语法引用变量：

```yaml
- `{{ global.api_key }}` - 引用全局变量
- `{{ nodes.fetch_data.output }}` - 引用其他节点的输出
- `{{ nodes.http_call.output.body }}` - 嵌套字段访问
- `{{ loop.item }}` - 循环中的当前元素 (仅在 Loop 节点内)
- `{{ loop.index }}` - 循环索引 (仅在 Loop 节点内)
```

```yaml
global:
  api_url: "https://api.example.com"
  
nodes:
  - id: "fetch"
    type: "http"
    params:
      url: "{{ global.api_url }}/users"
  
  - id: "process"
    needs: ["fetch"]
    type: "shell"
    params:
      # 访问前一个节点的输出
      command: "echo 'Status: {{ nodes.fetch.output }}'"
```

### 内存系统 (Memory System)

- **全局内存 (Global Memory)**: `{{ global.key }}`
  - 在 YAML 的 `global` 部分定义
  - 所有节点可读可写
  - 线程安全

- **节点内存 (Node Memory)**: `{{ nodes.node_id.output }}`
  - 每个节点执行完会存储输出
  - 只能访问已完成的依赖节点的输出

### 并行执行示例

```yaml
nodes:
  # 节点A先执行
  - id: "start"
    type: "shell"
    params:
      command: "echo 'Starting'"
  
  # 节点B和C并行执行 (都依赖start)
  - id: "parallel_1"
    needs: ["start"]
    type: "shell"
    params:
      command: "sleep 2 && echo 'Task 1'"
  
  - id: "parallel_2"
    needs: ["start"]
    type: "shell"
    params:
      command: "sleep 2 && echo 'Task 2'"
  
  # 节点D等待B和C都完成
  - id: "end"
    needs: ["parallel_1", "parallel_2"]
    type: "shell"
    params:
      command: "echo 'All done'"
```

## 性能特性

- ✅ 自动并行执行 (基于DAG依赖图)
- ✅ 多线程异步执行 (Tokio runtime)
- ✅ 线程安全的内存系统 (DashMap)
- ✅ 最大并发数限制 (默认10个节点同时执行)
