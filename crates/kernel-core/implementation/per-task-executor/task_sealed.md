Only its core makes a task, and nobody outside copies or compares one, even when every part of it could be.
Code outside may move a task:

```
use wcontainers_kernel_core_per_task_executor::{ProcessorStateOf, Task};
use wcontainers_kernel_core_per_task_executor::executor_boundary::TaskSource;
use wcontainers_memory_shard::MemoryShard;

trait Plain: Clone + PartialEq {}

fn hand_on<Shard: MemoryShard, Source: TaskSource<Shard>>(task: Task<Shard, Source>) -> Task<Shard, Source> where
    Shard: Plain,
    Source: Plain,
    Source::Table: Plain,
    Source::Runner: Plain,
    ProcessorStateOf<Shard, Source>: Plain,
{
    task
}
```

It cannot build one field by field:

```compile_fail
use wcontainers_kernel_core_per_task_executor::{ProcessorStateOf, Task};
use wcontainers_kernel_core_per_task_executor::executor_boundary::TaskSource;
use wcontainers_memory_shard::MemoryShard;

trait Plain: Clone + PartialEq {}

fn hand_on<Shard: MemoryShard, Source: TaskSource<Shard>>(task: Task<Shard, Source>) -> Task<Shard, Source> where
    Shard: Plain,
    Source: Plain,
    Source::Table: Plain,
    Source::Runner: Plain,
    ProcessorStateOf<Shard, Source>: Plain,
{
    let Task { memory, processor, runner } = task;
    Task { memory, processor, runner }
}
```

It cannot copy one:

```compile_fail
use wcontainers_kernel_core_per_task_executor::{ProcessorStateOf, Task};
use wcontainers_kernel_core_per_task_executor::executor_boundary::TaskSource;
use wcontainers_memory_shard::MemoryShard;

trait Plain: Clone + PartialEq {}

fn hand_on<Shard: MemoryShard, Source: TaskSource<Shard>>(task: &Task<Shard, Source>) -> Task<Shard, Source> where
    Shard: Plain,
    Source: Plain,
    Source::Table: Plain,
    Source::Runner: Plain,
    ProcessorStateOf<Shard, Source>: Plain,
{
    Clone::clone(task)
}
```

And it cannot compare two:

```compile_fail
use wcontainers_kernel_core_per_task_executor::{ProcessorStateOf, Task};
use wcontainers_kernel_core_per_task_executor::executor_boundary::TaskSource;
use wcontainers_memory_shard::MemoryShard;

trait Plain: Clone + PartialEq {}

fn hand_on<Shard: MemoryShard, Source: TaskSource<Shard>>(task: &Task<Shard, Source>, other: &Task<Shard, Source>) -> bool where
    Shard: Plain,
    Source: Plain,
    Source::Table: Plain,
    Source::Runner: Plain,
    ProcessorStateOf<Shard, Source>: Plain,
{
    task == other
}
```
