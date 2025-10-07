use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use solver::{
    components::{Component, OrderProducer, OrderProcessor, OrderConfirmer},
    events::{EventBus, OrderState},
    stores::{EventStore, Store},
    EventHandler,
};

#[tokio::test]
async fn test_full_order_lifecycle() {
    // Initialize tracing for tests
    let _ = tracing_subscriber::fmt::try_init();
    
    // Setup
    let event_bus = Arc::new(EventBus::new(100));
    let event_store = Arc::new(EventStore::new());
    
    event_store.initialize().await.unwrap();
    event_bus.register_handler(event_store.clone() as Arc<dyn EventHandler>).await;
    
    // Create components
    let order_producer = Arc::new(OrderProducer::new());
    let order_processor = Arc::new(OrderProcessor::new(event_store.clone()));
    let order_confirmer = Arc::new(OrderConfirmer::new(event_store.clone()));
    
    // Initialize components
    order_producer.initialize().await.unwrap();
    order_processor.initialize().await.unwrap();
    order_confirmer.initialize().await.unwrap();
    
    // Start components
    order_processor.start(event_bus.clone()).await.unwrap();
    order_confirmer.start(event_bus.clone()).await.unwrap();
    order_producer.start(event_bus.clone()).await.unwrap();
    
    // Let the system run for a bit
    sleep(Duration::from_secs(5)).await;
    
    // Stop producer
    order_producer.stop().await.unwrap();
    
    // Wait for processing to complete
    sleep(Duration::from_secs(3)).await;
    
    // Verify orders were created and processed
    let all_orders = event_store.get_all_orders().await.unwrap();
    assert!(!all_orders.is_empty(), "Should have created at least one order");
    
    // Check state counts
    let counts = event_store.get_state_counts().await.unwrap();
    println!("State counts: {:?}", counts);
    
    // Should have some confirmed orders
    let confirmed_count = counts.get("Confirmed").copied().unwrap_or(0);
    assert!(confirmed_count > 0, "Should have at least one confirmed order");
    
    // Stop other components
    order_processor.stop().await.unwrap();
    order_confirmer.stop().await.unwrap();
}

#[tokio::test]
async fn test_order_state_transitions() {
    let _ = tracing_subscriber::fmt::try_init();
    
    // Setup
    let event_bus = Arc::new(EventBus::new(100));
    let event_store = Arc::new(EventStore::new());
    
    event_store.initialize().await.unwrap();
    event_bus.register_handler(event_store.clone() as Arc<dyn EventHandler>).await;
    
    // Create and start processor and confirmer
    let order_processor = Arc::new(OrderProcessor::new(event_store.clone()));
    let order_confirmer = Arc::new(OrderConfirmer::new(event_store.clone()));
    
    order_processor.initialize().await.unwrap();
    order_confirmer.initialize().await.unwrap();
    
    order_processor.start(event_bus.clone()).await.unwrap();
    order_confirmer.start(event_bus.clone()).await.unwrap();
    
    // Manually create an order
    use solver::events::{Order, OrderCreatedEvent, OrderEvent};
    use uuid::Uuid;
    
    let order = Order {
        id: Uuid::new_v4(),
        amount: 500.0,
        asset: "USD".to_string(),
        state: OrderState::Created,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    
    let order_id = order.id;
    let event = OrderCreatedEvent::new(order);
    
    // Publish the event
    event_bus.publish(Arc::new(OrderEvent::Created(event))).await.unwrap();
    
    // Wait for processing
    sleep(Duration::from_millis(100)).await;
    
    // Check initial state
    let stored_order = event_store.get_order(&order_id).await.unwrap();
    assert!(stored_order.is_some(), "Order should be stored");
    assert_eq!(stored_order.unwrap().state, OrderState::Created);
    
    // Wait for processing to complete
    sleep(Duration::from_secs(2)).await;
    
    // Check final state
    let final_order = event_store.get_order(&order_id).await.unwrap();
    assert!(final_order.is_some(), "Order should still be stored");
    let final_state = final_order.unwrap().state;
    
    // Should be confirmed after full processing
    assert_eq!(final_state, OrderState::Confirmed, "Order should be confirmed");
    
    // Stop components
    order_processor.stop().await.unwrap();
    order_confirmer.stop().await.unwrap();
}

#[tokio::test]
async fn test_event_store_state_queries() {
    let _ = tracing_subscriber::fmt::try_init();
    
    let event_bus = Arc::new(EventBus::new(100));
    let event_store = Arc::new(EventStore::new());
    
    event_store.initialize().await.unwrap();
    event_bus.register_handler(event_store.clone() as Arc<dyn EventHandler>).await;
    
    // Create multiple orders
    use solver::events::{Order, OrderCreatedEvent, OrderEvent};
    use uuid::Uuid;
    
    for i in 0..5 {
        let order = Order {
            id: Uuid::new_v4(),
            amount: 100.0 * (i as f64 + 1.0),
            asset: "USD".to_string(),
            state: OrderState::Created,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        let event = OrderCreatedEvent::new(order);
        event_bus.publish(Arc::new(OrderEvent::Created(event))).await.unwrap();
    }
    
    // Wait for events to be processed
    sleep(Duration::from_millis(100)).await;
    
    // Query by state
    let created_orders = event_store.get_orders_by_state(OrderState::Created).await.unwrap();
    assert_eq!(created_orders.len(), 5, "Should have 5 created orders");
    
    // Get all orders
    let all_orders = event_store.get_all_orders().await.unwrap();
    assert_eq!(all_orders.len(), 5, "Should have 5 total orders");
    
    // Check state counts
    let counts = event_store.get_state_counts().await.unwrap();
    assert_eq!(*counts.get("Created").unwrap(), 5);
}

#[tokio::test]
async fn test_concurrent_order_processing() {
    let _ = tracing_subscriber::fmt::try_init();
    
    let event_bus = Arc::new(EventBus::new(100));
    let event_store = Arc::new(EventStore::new());
    
    event_store.initialize().await.unwrap();
    event_bus.register_handler(event_store.clone() as Arc<dyn EventHandler>).await;
    
    let order_processor = Arc::new(OrderProcessor::new(event_store.clone()));
    let order_confirmer = Arc::new(OrderConfirmer::new(event_store.clone()));
    
    order_processor.initialize().await.unwrap();
    order_confirmer.initialize().await.unwrap();
    
    order_processor.start(event_bus.clone()).await.unwrap();
    order_confirmer.start(event_bus.clone()).await.unwrap();
    
    // Create multiple orders concurrently
    use solver::events::{Order, OrderCreatedEvent, OrderEvent};
    use uuid::Uuid;
    
    let mut handles = vec![];
    
    for i in 0..10 {
        let event_bus_clone = event_bus.clone();
        let handle = tokio::spawn(async move {
            let order = Order {
                id: Uuid::new_v4(),
                amount: 100.0 * (i as f64 + 1.0),
                asset: "USD".to_string(),
                state: OrderState::Created,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            
            let event = OrderCreatedEvent::new(order);
            event_bus_clone.publish(Arc::new(OrderEvent::Created(event))).await.unwrap();
        });
        handles.push(handle);
    }
    
    // Wait for all orders to be published
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Wait for processing
    sleep(Duration::from_secs(3)).await;
    
    // Verify all orders were processed
    let all_orders = event_store.get_all_orders().await.unwrap();
    assert_eq!(all_orders.len(), 10, "Should have 10 orders");
    
    let confirmed_orders = event_store.get_orders_by_state(OrderState::Confirmed).await.unwrap();
    assert_eq!(confirmed_orders.len(), 10, "All orders should be confirmed");
    
    order_processor.stop().await.unwrap();
    order_confirmer.stop().await.unwrap();
}

#[tokio::test]
async fn test_component_lifecycle() {
    let _ = tracing_subscriber::fmt::try_init();
    
    let event_bus = Arc::new(EventBus::new(100));
    
    let order_producer = Arc::new(OrderProducer::new());
    
    // Test initialization
    assert!(order_producer.initialize().await.is_ok());
    
    // Test start
    assert!(order_producer.start(event_bus.clone()).await.is_ok());
    
    // Let it run briefly
    sleep(Duration::from_millis(500)).await;
    
    // Test stop
    assert!(order_producer.stop().await.is_ok());
}
