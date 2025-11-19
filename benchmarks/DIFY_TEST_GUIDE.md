# Dify 性能测试导入指南

## 📁 文件位置

已创建Dify工作流配置文件：
- **文件**: `benchmarks/dify_parallel_compute.yml`

## 📋 导入步骤

### 1. 导入工作流到Dify

1. 打开Dify工作室
2. 点击 "创建空白应用"
3. 选择 "工作流"
4. 在工作流编辑器中，点击右上角的 "..." 菜单
5. 选择 "导入DSL"
6. 上传 `dify_parallel_compute.yml` 文件
7. 点击确认导入

### 2. 验证工作流结构

导入后应该看到以下节点：

```
Start → Initialize Tasks → Parallel Compute (Iteration) → Summary → End
                              └─> Compute Fibonacci
```

**关键配置检查**：
- ✅ Iteration节点的 `is_parallel` 应该设置为 `true`
- ✅ `parallel_nums` 设置为 `10`

### 3. 运行测试

#### 测试1：小规模测试（5个任务）
```
输入参数:
- parallel_count: 5
- fib_n: 35
```

**点击运行，并记录：**
- 总执行时间
- 查看输出的 `total_computation_time`（所有任务的CPU时间总和）

#### 测试2：中等规模测试（10个任务）
```
输入参数:
- parallel_count: 10
- fib_n: 35
```

#### 测试3：大规模测试（20个任务）
```
输入参数:
- parallel_count: 20
- fib_n: 35
```

## 📊 结果对比表格

填写以下表格对比性能：

### 测试1: 5个并行任务 (fib_n=35)

| 引擎 | 总执行时间 | CPU总时间 | 并行效率 | 备注 |
|------|-----------|----------|----------|------|
| 我们的引擎 | 3.47s | 2.85s | 82% | ✅ 真并行 |
| Dify | _____s | _____s | ____% | 待测试 |

**并行效率计算**: `CPU总时间 / 总执行时间`
- 接近100% = 真正的并行
- 接近0% = 串行执行

### 测试2: 10个并行任务 (fib_n=35)

| 引擎 | 总执行时间 | CPU总时间 | 并行效率 |
|------|-----------|----------|----------|
| 我们的引擎 | ~3.5s | ~5.7s | ~163% |
| Dify | _____s | _____s | ____% |

### 测试3: 20个并行任务 (fib_n=35)

| 引擎 | 总执行时间 | CPU总时间 | 并行效率 |
|------|-----------|----------|----------|
| 我们的引擎 | ~4.0s | ~11.4s | ~285% |
| Dify | _____s | _____s | ____% |

## 🔍 观察要点

### 如果Dify是真正的并行（理想情况）
- 10个任务的总执行时间应该 ≈ 3.5s（单个任务的时间）
- CPU总时间应该 ≈ 5.7s（10 × 0.57s）
- 并行效率 > 100%

### 如果Dify是串行执行
- 10个任务的总执行时间应该 ≈ 6s（10 × 0.6s）
- CPU总时间 ≈ 总执行时间
- 并行效率 ≈ 100%

### 如果Dify有限并发（比如同时2个）
- 10个任务的总执行时间应该 ≈ 3s（5批 × 0.6s）
- CPU总时间 ≈ 6s
- 并行效率 ≈ 200%

## 🎯 预期结果

基于我们的经验，预测：

| 场景 | 可能性 | Dify表现 |
|------|--------|---------|
| 真正并行 | 30% | 与我们接近 |
| 有限并发（2-4线程） | 50% | 比我们慢2-5倍 |
| 串行执行 | 20% | 比我们慢8-10倍 |

## 📸 请记录

1. **执行时间截图**：工作流运行完成后的总时间
2. **输出结果**：
   - `total_tasks`: 任务总数
   - `total_computation_time`: CPU总时间
   - `average_time`: 平均单任务时间
   - `min_time` / `max_time`: 最小/最大时间

3. **资源使用**（如果可以看到）：
   - 内存峰值
   - CPU使用率

## 🚀 运行我们的引擎进行对比

同样的测试用我们的引擎运行：

```bash
# 测试1: 5个任务
time ./target/release/workflow-engine -f benchmarks/parallel_compute.yaml -i parallel_count=5 -i fib_n=35

# 测试2: 10个任务  
time ./target/release/workflow-engine -f benchmarks/parallel_compute.yaml -i parallel_count=10 -i fib_n=35

# 测试3: 20个任务
time ./target/release/workflow-engine -f benchmarks/parallel_compute.yaml -i parallel_count=20 -i fib_n=35
```

## 💡 故障排查

如果导入失败：
1. 检查Dify版本是否支持 `version: 0.4.0`
2. 尝试在Dify UI中手动创建同样的工作流结构
3. 确保 Code 节点的 Python 代码正确复制

如果执行报错：
1. 检查输入参数类型（必须是数字，不是字符串）
2. 查看Iteration节点的配置
3. 检查Code节点的变量映射
