use std::{collections::HashMap, time::Instant};

use kurbo::{Point, Size};
use winit::{
    dpi::{LogicalPosition, LogicalSize},
    event::WindowEvent,
    event_loop::{ControlFlow, EventLoopWindowTarget},
    window::WindowId,
};

use crate::{
    action::{Timer, TimerToken},
    app::{AppUpdateEvent, UserEvent, APP_UPDATE_EVENTS},
    ext_event::EXT_EVENT_HANDLER,
    view::View,
    window::WindowConfig,
    window_handle::WindowHandle,
};

pub(crate) struct ApplicationHandle {
    window_handles: HashMap<winit::window::WindowId, WindowHandle>,
    timers: HashMap<TimerToken, Timer>,
}

impl ApplicationHandle {
    pub(crate) fn new() -> Self {
        Self {
            window_handles: HashMap::new(),
            timers: HashMap::new(),
        }
    }

    pub(crate) fn handle_user_event(
        &mut self,
        event_loop: &EventLoopWindowTarget<UserEvent>,
        event: UserEvent,
    ) {
        match event {
            UserEvent::AppUpdate => {
                self.handle_update_event(event_loop);
            }
            UserEvent::Idle => {
                self.idle();
            }
            UserEvent::QuitApp => {
                event_loop.exit();
            }
        }
    }

    pub(crate) fn handle_update_event(&mut self, event_loop: &EventLoopWindowTarget<UserEvent>) {
        let events = APP_UPDATE_EVENTS.with(|events| {
            let mut events = events.borrow_mut();
            std::mem::take(&mut *events)
        });
        for event in events {
            match event {
                AppUpdateEvent::NewWindow { view_fn, config } => {
                    self.new_window(event_loop, view_fn, config)
                }
                AppUpdateEvent::CloseWindow { window_id } => {
                    self.close_window(window_id, event_loop);
                }
                AppUpdateEvent::RequestTimer { timer } => {
                    self.request_timer(timer, event_loop);
                }
                #[cfg(target_os = "linux")]
                AppUpdateEvent::MenuAction {
                    window_id,
                    action_id,
                } => {
                    let window_handle = match self.window_handles.get_mut(&window_id) {
                        Some(window_handle) => window_handle,
                        None => return,
                    };
                    window_handle.menu_action(action_id);
                }
            }
        }
    }

    pub(crate) fn handle_window_event(
        &mut self,
        window_id: winit::window::WindowId,
        event: WindowEvent,
        event_loop: &EventLoopWindowTarget<UserEvent>,
    ) {
        let window_handle = match self.window_handles.get_mut(&window_id) {
            Some(window_handle) => window_handle,
            None => return,
        };

        match event {
            WindowEvent::ActivationTokenDone { .. } => {}
            WindowEvent::Resized(size) => {
                let size: LogicalSize<f64> = size.to_logical(window_handle.scale);
                let size = Size::new(size.width, size.height);
                window_handle.size(size);
            }
            WindowEvent::Moved(position) => {
                let position: LogicalPosition<f64> = position.to_logical(window_handle.scale);
                let point = Point::new(position.x, position.y);
                window_handle.position(point);
            }
            WindowEvent::CloseRequested => {
                self.close_window(window_id, event_loop);
            }
            WindowEvent::Destroyed => {
                self.close_window(window_id, event_loop);
            }
            WindowEvent::DroppedFile(_) => {}
            WindowEvent::HoveredFile(_) => {}
            WindowEvent::HoveredFileCancelled => {}
            WindowEvent::Focused(focused) => {
                window_handle.focused(focused);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                window_handle.key_event(event);
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                window_handle.modifiers = modifiers.state();
            }
            WindowEvent::Ime(ime) => {
                window_handle.ime(ime);
            }
            WindowEvent::CursorMoved { position, .. } => {
                let position: LogicalPosition<f64> = position.to_logical(window_handle.scale);
                let point = Point::new(position.x, position.y);
                window_handle.pointer_move(point);
            }
            WindowEvent::CursorEntered { .. } => {}
            WindowEvent::CursorLeft { .. } => {
                window_handle.pointer_leave();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                window_handle.mouse_wheel(delta);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                window_handle.mouse_input(button, state);
            }
            WindowEvent::TouchpadMagnify { .. } => {}
            WindowEvent::SmartMagnify { .. } => {}
            WindowEvent::TouchpadRotate { .. } => {}
            WindowEvent::TouchpadPressure { .. } => {}
            WindowEvent::AxisMotion { .. } => {}
            WindowEvent::Touch(_) => {}
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                window_handle.scale(scale_factor);
            }
            WindowEvent::ThemeChanged(theme) => {
                window_handle.theme_changed(theme);
            }
            WindowEvent::Occluded(_) => {}
            WindowEvent::MenuAction(id) => {
                window_handle.menu_action(id);
            }
            WindowEvent::RedrawRequested => {
                window_handle.paint();
            }
        }
    }

    pub(crate) fn new_window(
        &mut self,
        event_loop: &EventLoopWindowTarget<UserEvent>,
        view_fn: Box<dyn FnOnce(WindowId) -> Box<dyn View>>,
        config: Option<WindowConfig>,
    ) {
        let mut window_builder = winit::window::WindowBuilder::new();
        if let Some(config) = config {
            if let Some(size) = config.size {
                let size = if size.width == 0.0 || size.height == 0.0 {
                    Size::new(800.0, 600.0)
                } else {
                    size
                };
                window_builder =
                    window_builder.with_inner_size(LogicalSize::new(size.width, size.height));
            }
            if let Some(pos) = config.position {
                window_builder = window_builder.with_position(LogicalPosition::new(pos.x, pos.y));
            }
            if let Some(show_titlebar) = config.show_titlebar {
                #[cfg(target_os = "macos")]
                if !show_titlebar {
                    use winit::platform::macos::WindowBuilderExtMacOS;
                    window_builder = window_builder
                        .with_title_hidden(true)
                        .with_titlebar_transparent(true)
                        .with_fullsize_content_view(true);
                }
                #[cfg(not(target_os = "macos"))]
                if !show_titlebar {
                    window_builder = window_builder.with_decorations(false);
                }
            }
        }
        let result = window_builder.build(event_loop);
        let window = match result {
            Ok(window) => window,
            Err(_) => return,
        };
        let window_id = window.id();
        let window_handle = WindowHandle::new(window, view_fn);
        self.window_handles.insert(window_id, window_handle);
    }

    fn close_window(
        &mut self,
        window_id: WindowId,
        #[cfg(target_os = "macos")] _event_loop: &EventLoopWindowTarget<UserEvent>,
        #[cfg(not(target_os = "macos"))] event_loop: &EventLoopWindowTarget<UserEvent>,
    ) {
        if let Some(handle) = self.window_handles.get_mut(&window_id) {
            handle.window = None;
            handle.destroy();
        }
        self.window_handles.remove(&window_id);
        #[cfg(not(target_os = "macos"))]
        if self.window_handles.is_empty() {
            event_loop.exit();
        }
    }

    pub(crate) fn idle(&mut self) {
        while let Some(trigger) = { EXT_EVENT_HANDLER.queue.lock().pop_front() } {
            trigger.notify();
        }
        for (_, handle) in self.window_handles.iter_mut() {
            handle.process_update();
        }
    }

    fn request_timer(&mut self, timer: Timer, event_loop: &EventLoopWindowTarget<UserEvent>) {
        self.timers.insert(timer.token, timer);
        self.fire_timer(event_loop);
    }

    fn fire_timer(&mut self, event_loop: &EventLoopWindowTarget<UserEvent>) {
        if self.timers.is_empty() {
            return;
        }

        let deadline = self.timers.values().map(|timer| timer.deadline).min();
        if let Some(deadline) = deadline {
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
        }
    }

    pub(crate) fn handle_timer(&mut self, event_loop: &EventLoopWindowTarget<UserEvent>) {
        let now = Instant::now();
        let tokens: Vec<TimerToken> = self
            .timers
            .iter()
            .filter_map(|(token, timer)| {
                if timer.deadline <= now {
                    Some(*token)
                } else {
                    None
                }
            })
            .collect();
        if !tokens.is_empty() {
            for token in tokens {
                if let Some(timer) = self.timers.remove(&token) {
                    (timer.action)(token);
                }
            }
            for (_, handle) in self.window_handles.iter_mut() {
                handle.process_update();
            }
        }
        self.fire_timer(event_loop);
    }
}
