use std::sync::Arc;
use std::sync::atomic::Ordering;
use crate::state::CrawlState;
use crate::queue::Queue;

pub async fn run_dispatcher(state: Arc<CrawlState>, worker_count: usize) {
    let sem = Arc::new(tokio::sync::Semaphore::new(worker_count));

    loop {
        // Check for graceful shutdown signal (e.g. Ctrl+C)
        if state.shutdown.load(Ordering::SeqCst) {
            break;
        }

        // if pages_crawled >= page_limit: break
        let pages = state.pages_crawled.load(Ordering::SeqCst);
        if pages >= state.page_limit {
            break;
        }

        let is_empty = state.queue.is_empty();
        let in_flight = state.in_flight.load(Ordering::SeqCst);

        // if queue is empty AND in_flight == 0: break
        if is_empty && in_flight == 0 {
            break;
        }

        // if queue is empty: wait for a notification from workers and continue
        if is_empty {
            state.notify.notified().await;
            continue;
        }

        // pop WorkUnit from queue
        let work_unit = state.queue.pop().await;

        // check visited -> skip if already seen
        // Note: VisitedTable stores UrlState::InFlight when insert is called. 
        // We only crawl if it was successfully inserted (which is checked in the worker before pushing).
        // However, we can double check here if it has been marked visited/inflight.
        // If not in visited table or if we want to be safe:
        if !state.visited.is_visited(work_unit.url.as_str()) && state.visited.get(work_unit.url.as_str()).is_none() {
            // Wait, if it wasn't inserted, it should have been inserted when enqueued. 
            // So we can assume it's valid if it came from the queue.
        }

        // increment in_flight
        state.in_flight.fetch_add(1, Ordering::SeqCst);

        // acquire permit (caps active tasks)
        let permit = sem.clone().acquire_owned().await.unwrap();

        let state_clone = Arc::clone(&state);
        tokio::spawn(async move {
            let _permit = permit;
            crate::worker::worker(work_unit, state_clone).await;
        });
    }

    // Wait for all in-flight workers to finish when exiting the dispatcher
    while state.in_flight.load(Ordering::SeqCst) > 0 {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
}
