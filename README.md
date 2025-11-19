# Workflow Engine - User Guide

## 如何运行 (How to Run)

### 1. 构建项目 (Build the project)
```bash
cd /Users/hejl/projects/dsl/workflow-engine
cargo build
```

### 2. 运行工作流 (Run a workflow)
```bash
# 使用示例工作流
cargo run -- -f example.yaml

# 使用日志输出
RUST_LOG=info cargo run -- -f example.yaml

# 运行自定义工作流
cargo run -- -f /path/to/your/workflow.yaml
```

### 3. 发布构建 (Release build - optimized)
```bash
cargo build --release
./target/release/workflow-engine -f example.yaml
```

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

#### 1. Shell 节点
执行系统命令
```yaml
- id: "my_shell"
  type: "shell"
  name: "执行脚本"
  params:
    command: "echo 'Hello' && ls -la"
```

#### 2. HTTP 节点
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

#### 3. Delay 节点
等待指定时间
```yaml
- id: "wait"
  type: "delay"
  name: "等待"
  params:
    milliseconds: 1000  # 等待1秒
```

### 变量引用 (Variable Substitution)

在 `params` 中使用 `{{ }}` 语法引用变量：

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
