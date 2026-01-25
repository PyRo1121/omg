# Elm Architecture Pattern for OMG CLI

## Quick Reference

### Traditional Approach vs Elm Architecture

#### Traditional (src/cli/packages/status.rs)
```rust
pub async fn status(fast: bool) -> Result<()> {
    let start = std::time::Instant::now();
    
    // Direct logic mixed with I/O
    if let Ok(mut client) = DaemonClient::connect().await {
        // Fetch data
        // Display directly
        display_status_report(...)?;
    }
    
    Ok(())
}
```

#### Elm Architecture (src/cli/tea/status_model.rs)
```rust
// Model - State only
pub struct StatusModel {
    data: Option<StatusData>,
    loading: bool,
    error: Option<String>,
}

// Update - Pure state transitions
fn update(&mut self, msg: StatusMsg) -> Cmd<StatusMsg> {
    match msg {
        StatusMsg::Refresh => {
            self.loading = true;
            Cmd::exec(|| fetch_status_data())
        }
        StatusMsg::DataReceived(data) => {
            self.data = Some(data);
            self.loading = false;
            Cmd::none()
        }
    }
}

// View - Pure rendering
fn view(&self) -> String {
    format!("Status: {}", self.data.as_ref()?.total)
}
```

## Benefits

1. **Testability**: Each component tested independently
2. **Predictability**: State changes are explicit
3. **Reusability**: Commands can be composed
4. **Maintainability**: Clear separation of concerns

## Creating a New Model

### Step 1: Define Messages
```rust
#[derive(Debug, Clone)]
pub enum MyMsg {
    Start,
    DataReceived(Data),
    Error(String),
}
```

### Step 2: Define Model
```rust
pub struct MyModel {
    state: MyState,
    data: Option<Data>,
}

impl MyModel {
    pub fn new() -> Self {
        Self {
            state: MyState::Idle,
            data: None,
        }
    }
}
```

### Step 3: Implement Model Trait
```rust
impl Model for MyModel {
    type Msg = MyMsg;

    fn update(&mut self, msg: MyMsg) -> Cmd<MyMsg> {
        match msg {
            MyMsg::Start => {
                self.state = MyState::Loading;
                Cmd::exec(|| fetch_data())
            }
            MyMsg::DataReceived(data) => {
                self.data = Some(data);
                self.state = MyState::Ready;
                Cmd::none()
            }
            MyMsg::Error(err) => {
                self.state = MyState::Failed;
                Cmd::error(format!("Failed: {}", err))
            }
        }
    }

    fn view(&self) -> String {
        match self.state {
            MyState::Idle => "Ready".to_string(),
            MyState::Loading => "Loading...".to_string(),
            MyState::Ready => format!("Got: {:?}", self.data),
            MyState::Failed => "Error".to_string(),
        }
    }
}
```

### Step 4: Write Tests (TDD)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let model = MyModel::new();
        assert!(matches!(model.state, MyState::Idle));
    }

    #[test]
    fn test_start_transition() {
        let mut model = MyModel::new();
        let _cmd = model.update(MyMsg::Start);
        assert!(matches!(model.state, MyState::Loading));
    }

    #[test]
    fn test_data_received() {
        let mut model = MyModel::new();
        model.state = MyState::Loading;
        let _cmd = model.update(MyMsg::DataReceived(...));
        assert!(matches!(model.state, MyState::Ready));
    }
}
```

### Step 5: Create Wrapper
```rust
pub fn run_my_command() -> Result<(), std::io::Error> {
    let model = MyModel::new();
    Program::new(model).run()
}
```

## Command Types

### Cmd::none()
No operation
```rust
Cmd::none()
```

### Cmd::exec()
Execute a function and send result as message
```rust
Cmd::exec(|| {
    let data = fetch_data();
    MyMsg::DataReceived(data)
})
```

### Cmd::batch()
Execute multiple commands in sequence
```rust
Cmd::batch([
    Cmd::header("Title", "Subtitle"),
    Cmd::info("Starting..."),
    Cmd::exec(|| do_work()),
])
```

### Cmd::info(), Cmd::success(), Cmd::error(), Cmd::warning()
Display styled messages
```rust
Cmd::info("Information message")
Cmd::success("Operation completed!")
Cmd::error("Something went wrong")
Cmd::warning("Warning message")
```

### Cmd::header(), Cmd::card()
Display formatted sections
```rust
Cmd::header("Title", "Subtitle")
Cmd::card("Card Title", "Card content")
```

## Testing Patterns

### Testing State Transitions
```rust
#[test]
fn test_state_transition() {
    let mut model = MyModel::new();
    
    // Initial state
    assert_eq!(model.state, MyState::Idle);
    
    // After update
    let _cmd = model.update(MyMsg::Start);
    assert_eq!(model.state, MyState::Loading);
}
```

### Testing View Output
```rust
#[test]
fn test_view_contains_expected_content() {
    let mut model = MyModel::new();
    model.update(MyMsg::DataReceived(data));
    
    let view = model.view();
    assert!(view.contains("expected text"));
}
```

### Testing Command Emission
```rust
#[test]
fn test_emits_expected_command() {
    let mut model = MyModel::new();
    let cmd = model.update(MyMsg::Error("fail".to_string()));
    
    assert!(matches!(cmd, Cmd::Error(_)));
}
```

## Migration Checklist

- [ ] Define message types for all state changes
- [ ] Create model struct with state fields
- [ ] Implement Model trait (update, view)
- [ ] Write tests for all state transitions
- [ ] Create wrapper function for CLI integration
- [ ] Run tests and ensure 100% pass rate
- [ ] Update CLI command dispatch to use new model

## Common Patterns

### Async Operations
```rust
fn update(&mut self, msg: MyMsg) -> Cmd<MyMsg> {
    match msg {
        MyMsg::Fetch => {
            self.loading = true;
            // Use Cmd::exec with async-to-sync bridge
            Cmd::exec(|| {
                let data = block_on(fetch_async());
                MyMsg::DataReceived(data)
            })
        }
    }
}
```

### Progress Tracking
```rust
#[derive(Debug, Clone)]
pub enum ProgressMsg {
    Start,
    Progress { percent: usize },
    Complete,
}

fn update(&mut self, msg: ProgressMsg) -> Cmd<ProgressMsg> {
    match msg {
        ProgressMsg::Progress { percent } => {
            self.percent = percent;
            if percent % 10 == 0 {
                Cmd::info(format!("Progress: {}%", percent))
            } else {
                Cmd::none()
            }
        }
        _ => Cmd::none()
    }
}
```

### Error Handling
```rust
fn update(&mut self, msg: MyMsg) -> Cmd<MyMsg> {
    match msg {
        MyMsg::Error(err) => {
            self.error = Some(err.clone());
            self.loading = false;
            Cmd::batch([
                Cmd::error(format!("Operation failed: {}", err)),
                Cmd::Info("Check logs for details".to_string()),
            ])
        }
    }
}
```

## Resources

- Implementation: `/home/pyro1121/Documents/code/filemanager/omg/src/cli/tea/`
- Examples: `examples.rs`
- Test Suite: Run `cargo test --lib cli::tea`
- Summary: `/home/pyro1121/Documents/code/filemanager/omg/ELM_REFACTORING_SUMMARY.md`
