use std::time::Duration;

use ingjoo_queue::{InMemoryQueue, JobStatus, QueuedJob, Queue};
use serde_json::json;

#[tokio::test]
async fn test_enqueue_and_dequeue() {
    let queue = InMemoryQueue::new();
    let job = QueuedJob::new("default", "send_email", json!({"to": "test@example.com"}));

    let id = queue.enqueue(job).await.unwrap();
    assert!(!id.is_empty());

    let dequeued = queue.dequeue("default").await.unwrap().unwrap();
    assert_eq!(dequeued.id, id);
    assert_eq!(dequeued.name, "send_email");
    assert_eq!(dequeued.queue, "default");
    assert_eq!(dequeued.status, JobStatus::Running);
    assert_eq!(dequeued.attempts, 1);
}

#[tokio::test]
async fn test_dequeue_empty_queue() {
    let queue = InMemoryQueue::new();
    let result = queue.dequeue("default").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_dequeue_different_queue() {
    let queue = InMemoryQueue::new();
    let job = QueuedJob::new("emails", "send_email", json!({}));
    queue.enqueue(job).await.unwrap();

    let result = queue.dequeue("default").await.unwrap();
    assert!(result.is_none());

    let result = queue.dequeue("emails").await.unwrap();
    assert!(result.is_some());
}

#[tokio::test]
async fn test_ack_completes_job() {
    let queue = InMemoryQueue::new();
    let job = QueuedJob::new("default", "task", json!({}));
    let _id = queue.enqueue(job).await.unwrap();

    let dequeued = queue.dequeue("default").await.unwrap().unwrap();
    queue.ack(&dequeued.id).await.unwrap();

    let count = queue.pending_count("default").await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_nack_marks_failed() {
    let queue = InMemoryQueue::new();
    let job = QueuedJob::new("default", "task", json!({})).with_max_attempts(3);
    queue.enqueue(job).await.unwrap();

    let dequeued = queue.dequeue("default").await.unwrap().unwrap();
    queue.nack(&dequeued.id, "something went wrong").await.unwrap();

    let count = queue.pending_count("default").await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_nack_exhausted_becomes_dead_letter() {
    let queue = InMemoryQueue::new();
    let mut job = QueuedJob::new("default", "task", json!({})).with_max_attempts(1);
    job.attempts = 1;
    queue.enqueue(job).await.unwrap();

    let dequeued = queue.dequeue("default").await.unwrap().unwrap();
    queue.nack(&dequeued.id, "exhausted").await.unwrap();

    let next = queue.dequeue("default").await.unwrap();
    assert!(next.is_none());
}

#[tokio::test]
async fn test_retry_resets_to_pending() {
    let queue = InMemoryQueue::new();
    let job = QueuedJob::new("default", "task", json!({}));
    queue.enqueue(job).await.unwrap();

    let dequeued = queue.dequeue("default").await.unwrap().unwrap();
    queue.nack(&dequeued.id, "temporary failure").await.unwrap();

    queue.retry(&dequeued.id).await.unwrap();

    let count = queue.pending_count("default").await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_dead_letter() {
    let queue = InMemoryQueue::new();
    let job = QueuedJob::new("default", "task", json!({}));
    let id = queue.enqueue(job).await.unwrap();

    queue.dead_letter(&id).await.unwrap();

    let count = queue.pending_count("default").await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_pending_count() {
    let queue = InMemoryQueue::new();

    for i in 0..5 {
        let job = QueuedJob::new("default", &format!("task_{}", i), json!({}));
        queue.enqueue(job).await.unwrap();
    }

    assert_eq!(queue.pending_count("default").await.unwrap(), 5);
}

#[tokio::test]
async fn test_priority_ordering() {
    let queue = InMemoryQueue::new();

    let low = QueuedJob::new("default", "low", json!({})).with_priority(10);
    let high = QueuedJob::new("default", "high", json!({})).with_priority(-5);
    let mid = QueuedJob::new("default", "mid", json!({})).with_priority(0);

    queue.enqueue(low).await.unwrap();
    queue.enqueue(high).await.unwrap();
    queue.enqueue(mid).await.unwrap();

    let first = queue.dequeue("default").await.unwrap().unwrap();
    assert_eq!(first.name, "high");
    assert_eq!(first.priority, -5);

    let second = queue.dequeue("default").await.unwrap().unwrap();
    assert_eq!(second.name, "mid");

    let third = queue.dequeue("default").await.unwrap().unwrap();
    assert_eq!(third.name, "low");
}

#[tokio::test]
async fn test_fifo_same_priority() {
    let queue = InMemoryQueue::new();

    for i in 0..3 {
        let job = QueuedJob::new("default", &format!("task_{}", i), json!({}));
        queue.enqueue(job).await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let first = queue.dequeue("default").await.unwrap().unwrap();
    assert_eq!(first.name, "task_0");

    let second = queue.dequeue("default").await.unwrap().unwrap();
    assert_eq!(second.name, "task_1");
}

#[tokio::test]
async fn test_delayed_job_not_ready() {
    let queue = InMemoryQueue::new();
    let job = QueuedJob::new("default", "delayed", json!({}))
        .with_delay(Duration::from_secs(3600));

    queue.enqueue(job).await.unwrap();

    let result = queue.dequeue("default").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_queued_job_builder() {
    let job = QueuedJob::new("emails", "send", json!({"to": "a@b.com"}))
        .with_priority(5)
        .with_max_attempts(10)
        .with_delay(Duration::from_secs(30));

    assert_eq!(job.queue, "emails");
    assert_eq!(job.name, "send");
    assert_eq!(job.priority, 5);
    assert_eq!(job.max_attempts, 10);
    assert!(job.run_at.is_some());
    assert_eq!(job.status, JobStatus::Pending);
    assert_eq!(job.attempts, 0);
    assert!(!job.is_exhausted());
}

#[tokio::test]
async fn test_is_exhausted() {
    let mut job = QueuedJob::new("default", "task", json!({})).with_max_attempts(3);
    assert!(!job.is_exhausted());
    job.attempts = 3;
    assert!(job.is_exhausted());
}
