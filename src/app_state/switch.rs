use super::AppStateTrait;

/// Trait allow to switch state from one to another
pub trait SwitchState<T: AppStateTrait> {
    fn switch(&mut self, new_state: T);
}
