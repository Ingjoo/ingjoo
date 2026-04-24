use ingjoo_core::pool::Pool;
use ingjoo_core::Dialect;
use ingjoo_queue::{JobStatus, QueuedJob, SqlQueue, Queue};
use serde_json::json;

async fn setup_pool() -> Pool {
    let tmp = tempfile::Builder::new()
        .prefix("queue_test_")
        .suffix(".db")
        .tempfile()
        .unwrap();
    let db_path = tmp.path().to_str().unwrap().to_string();
    std::mem::forget(tmp);

    let db_url = format!("sqlite://{}?mode=rwc", db_path);
    ingjoo_core::pool::install_drivers();
    let (pool, _) = ingjoo_core::pool::connect_pool(&db_url).await.unwrap();

    let dialect = Dialect::Sqlite;
    let create_sql = dialect.prepare(
        r#"CREATE TABLE IF NOT EXISTS queue_jobs (
            id TEXT PRIMARY KEY,
            queue TEXT NOT NULL DEFAULT 'default',
            name TEXT NOT NULL,
            payload TEXT NOT NULL DEFAULT '{}',
            priority INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'pending',
            attempts INTEGER NOT NULL DEFAULT 0,
            max_attempts INTEGER NOT NULL DEFAULT 3,
            run_at TEXT,
            started_at TEXT,
            completed_at TEXT,
            error TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_queue_jobs_status ON queue_jobs(queue, status);
        CREATE INDEX IF NOT EXISTS idx_queue_jobs_priority ON queue_jobs(priority ASC, created_at ASC)"#
    );
    for stmt in Dialect::split_ddl(&create_sql) {
        sqlx::query(stmt).execute(&pool).await.unwrap();
    }

    pool
}

#[tokio::test]
async fn test_sql_enqueue_and_dequeue() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let queue = SqlQueue::new(&pool, &dialect);

    let job = QueuedJob::new("default", "send_email", json!({"to": "test@example.com"}));
    let id = queue.enqueue(job).await.unwrap();
    assert!(!id.is_empty());

    let dequeued = queue.dequeue("default").await.unwrap().unwrap();
    assert_eq!(dequeued.id, id);
    assert_eq!(dequeued.name, "send_email");
    assert_eq!(dequeued.status, JobStatus::Running);
    assert_eq!(dequeued.attempts, 1);
}

#[tokio::test]
async fn test_sql_dequeue_empty() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let queue = SqlQueue::new(&pool, &dialect);

    let result = queue.dequeue("default").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_sql_ack() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let queue = SqlQueue::new(&pool, &dialect);

    let job = QueuedJob::new("default", "task", json!({}));
    queue.enqueue(job).await.unwrap();

    let dequeued = queue.dequeue("default").await.unwrap().unwrap();
    queue.ack(&dequeued.id).await.unwrap();

    assert_eq!(queue.pending_count("default").await.unwrap(), 0);

    let again = queue.dequeue("default").await.unwrap();
    assert!(again.is_none());
}

#[tokio::test]
async fn test_sql_nack_and_dead_letter() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let queue = SqlQueue::new(&pool, &dialect);

    let job = QueuedJob::new("default", "task", json!({})).with_max_attempts(1);
    queue.enqueue(job).await.unwrap();

    let dequeued = queue.dequeue("default").await.unwrap().unwrap();
    queue.nack(&dequeued.id, "failed").await.unwrap();

    assert_eq!(queue.pending_count("default").await.unwrap(), 0);

    let again = queue.dequeue("default").await.unwrap();
    assert!(again.is_none());
}

#[tokio::test]
async fn test_sql_retry() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let queue = SqlQueue::new(&pool, &dialect);

    let job = QueuedJob::new("default", "task", json!({}));
    queue.enqueue(job).await.unwrap();

    let dequeued = queue.dequeue("default").await.unwrap().unwrap();
    queue.nack(&dequeued.id, "temporary").await.unwrap();

    queue.retry(&dequeued.id).await.unwrap();
    assert_eq!(queue.pending_count("default").await.unwrap(), 1);
}

#[tokio::test]
async fn test_sql_pending_count() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let queue = SqlQueue::new(&pool, &dialect);

    for i in 0..5 {
        let job = QueuedJob::new("default", &format!("task_{}", i), json!({}));
        queue.enqueue(job).await.unwrap();
    }

    assert_eq!(queue.pending_count("default").await.unwrap(), 5);
}

#[tokio::test]
async fn test_sql_priority_ordering() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let queue = SqlQueue::new(&pool, &dialect);

    let low = QueuedJob::new("default", "low", json!({})).with_priority(10);
    let high = QueuedJob::new("default", "high", json!({})).with_priority(-5);
    let mid = QueuedJob::new("default", "mid", json!({})).with_priority(0);

    queue.enqueue(low).await.unwrap();
    queue.enqueue(high).await.unwrap();
    queue.enqueue(mid).await.unwrap();

    let first = queue.dequeue("default").await.unwrap().unwrap();
    assert_eq!(first.name, "high");

    let second = queue.dequeue("default").await.unwrap().unwrap();
    assert_eq!(second.name, "mid");

    let third = queue.dequeue("default").await.unwrap().unwrap();
    assert_eq!(third.name, "low");
}

#[tokio::test]
async fn test_sql_dead_letter_directly() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let queue = SqlQueue::new(&pool, &dialect);

    let job = QueuedJob::new("default", "task", json!({}));
    let id = queue.enqueue(job).await.unwrap();

    queue.dead_letter(&id).await.unwrap();

    assert_eq!(queue.pending_count("default").await.unwrap(), 0);
    let result = queue.dequeue("default").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_sql_different_queues_isolated() {
    let pool = setup_pool().await;
    let dialect = Dialect::Sqlite;
    let queue = SqlQueue::new(&pool, &dialect);

    let job_a = QueuedJob::new("queue_a", "task_a", json!({}));
    let job_b = QueuedJob::new("queue_b", "task_b", json!({}));
    queue.enqueue(job_a).await.unwrap();
    queue.enqueue(job_b).await.unwrap();

    let from_a = queue.dequeue("queue_a").await.unwrap().unwrap();
    assert_eq!(from_a.name, "task_a");

    let from_b = queue.dequeue("queue_b").await.unwrap().unwrap();
    assert_eq!(from_b.name, "task_b");

    assert_eq!(queue.pending_count("queue_a").await.unwrap(), 0);
    assert_eq!(queue.pending_count("queue_b").await.unwrap(), 0);
}
