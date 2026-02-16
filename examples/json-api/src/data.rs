use serde::{Deserialize, Serialize};
use std::cell::RefCell;

/// An item in our simple JSON API.
#[derive(Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: u64,
    pub name: String,
}

/// Input for creating or updating an item.
#[derive(Deserialize)]
pub struct CreateItem {
    pub name: String,
}

thread_local! {
    static ITEMS: RefCell<Vec<Item>> = RefCell::new(vec![
        Item { id: 1, name: "First item".to_string() },
        Item { id: 2, name: "Second item".to_string() },
    ]);
    static NEXT_ID: RefCell<u64> = RefCell::new(3);
}

pub fn list_items() -> Vec<Item> {
    ITEMS.with(|items| items.borrow().clone())
}

pub fn get_item(id: u64) -> Option<Item> {
    ITEMS.with(|items| items.borrow().iter().find(|i| i.id == id).cloned())
}

pub fn create_item(input: CreateItem) -> Item {
    let id = NEXT_ID.with(|n| {
        let mut n = n.borrow_mut();
        let id = *n;
        *n += 1;
        id
    });
    let item = Item {
        id,
        name: input.name,
    };
    ITEMS.with(|items| items.borrow_mut().push(item.clone()));
    item
}

pub fn update_item(id: u64, input: CreateItem) -> Option<Item> {
    ITEMS.with(|items| {
        let mut items = items.borrow_mut();
        if let Some(item) = items.iter_mut().find(|i| i.id == id) {
            item.name = input.name;
            Some(item.clone())
        } else {
            None
        }
    })
}

pub fn delete_item(id: u64) -> bool {
    ITEMS.with(|items| {
        let mut items = items.borrow_mut();
        let len = items.len();
        items.retain(|i| i.id != id);
        items.len() < len
    })
}
