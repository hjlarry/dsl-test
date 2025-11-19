# 性能测试指南

## 测试场景

### 1. 并行计算测试 (CPU密集型)
**文件**: `benchmarks/parallel_compute.yaml`
**测试内容**: 并行计算斐波那契数列第35项（每个任务约1-2秒）
**参数**:
- `parallel_count`: 并行任务数量 (默认10)
- `fib_n`: 斐波那契数列项数 (默认35)

### 2. 并行HTTP测试 (I/O密集型)
**文件**: `benchmarks/parallel_http.yaml`
**测试内容**: 并行发送HTTP GET请求到公共API
**参数**:
- `parallel_count`: 并行请求数量 (默认20)

## 测试方法

### 在我们的引擎中运行

```bash
# 并行计算测试 (10个并行任务)
time cargo run --release -- -f benchmarks/parallel_compute.yaml

# 并行HTTP测试 (20个并行请求)
time cargo run --release -- -f benchmarks/parallel_http.yaml

# 调整参数测试
cargo run --release -- -f benchmarks/parallel_compute.yaml -i parallel_count=20 -i fib_n=38
```

### 在Dify中运行

1. **创建新工作流**
2. **复制节点结构**:
   - Start节点（设置输入参数）
   - Code节点（初始化任务数组）
   - Iteration节点（配置循环）
   - Code节点（执行计算/HTTP请求）
   - Code节点（汇总结果）
   - End节点

3. **运行并记录时间**

## 对比指标

### 📊 主要指标

| 指标 | 说明 |
|------|------|
| **总执行时间** | 从开始到结束的总时长 |
| **并行效率** | 理论时间 vs 实际时间的比率 |
| **内存使用** | 峰值内存占用 |
| **启动时间** | 工作流启动到第一个节点执行的时间 |

### 📈 测试结果示例格式

```
## 测试环境
- CPU: [处理器型号]
- 内存: [RAM大小]
- OS: [操作系统]

## 并行计算测试 (10个任务, fib_n=35)

| 引擎 | 总时间 | 理论时间 | 并行效率 | 内存使用 |
|------|--------|----------|----------|----------|
| 我们的引擎 | 2.5s | 15s | 85% | 45MB |
| Dify | 15.2s | 15s | 1% | 120MB |

## 并行HTTP测试 (20个请求)

| 引擎 | 总时间 | 理论时间 | 并行效率 | 内存使用 |
|------|--------|----------|----------|----------|
| 我们的引擎 | 1.2s | 8s | 85% | 35MB |
| Dify | 8.5s | 8s | 6% | 95MB |
```

## 预期结果

### 我们的优势（预测）

1. **并行执行**
   - ✅ 真正的异步并行（Tokio runtime）
   - ✅ 智能依赖调度
   - ✅ 资源高效利用

2. **性能表现**
   - ✅ CPU密集型：接近线性加速比
   - ✅ I/O密集型：接近理论最优
   - ✅ 内存占用：更低（Rust零成本抽象）

3. **启动速度**
   - ✅ 二进制直接启动
   - ✅ 无Python解释器开销

### Dify的特点

- 可能是串行执行（需要验证）
- 或有限的并发控制
- Python runtime开销

## 测试脚本

自动化测试脚本：

```bash
#!/bin/bash
# benchmark.sh

echo "=== 并行计算测试 ==="
for count in 5 10 15 20; do
    echo "Testing with $count parallel tasks..."
    /usr/bin/time -l cargo run --release -- -f benchmarks/parallel_compute.yaml -i parallel_count=$count
done

echo ""
echo "=== 并行HTTP测试 ==="
for count in 10 20 30 40; do
    echo "Testing with $count parallel requests..."
    /usr/bin/time -l cargo run --release -- -f benchmarks/parallel_http.yaml -i parallel_count=$count
done
```

## 进阶测试

### 压力测试
```bash
# 100个并行计算任务
cargo run --release -- -f benchmarks/parallel_compute.yaml -i parallel_count=100

# 200个并行HTTP请求
cargo run --release -- -f benchmarks/parallel_http.yaml -i parallel_count=200
```

### 内存分析
```bash
# 使用valgrind (Linux)
valgrind --tool=massif cargo run --release -- -f benchmarks/parallel_compute.yaml

# 使用heaptrack (Linux)
heaptrack cargo run --release -- -f benchmarks/parallel_compute.yaml
```
