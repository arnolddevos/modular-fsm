/// Event driven Finite State Machines process commands (possibly created by other
/// events), performing some side effect, and emitting events.
/// Commands are processed against a provided state. Events can be applied to states
/// to yield new states.
///
/// For more background on [Event-driven Finite State Machines](http://christopherhunt-software.blogspot.com/2021/02/event-driven-finite-state-machines.html).

/// Describes how to transition from one state to another
#[derive(Debug, PartialEq)]
pub enum Transition<S> {
    /// Transition into a new state
    Next(S),
    /// Stay in the existing state
    Same,
}

/// How to operate on just part of the state.
/// Self is the state of an FSM and T
/// is a view of that state of interest to
/// some Event or Command.
trait Lens<T> {
    /// Extract a view of state.
    fn extract(&self) -> &T;

    /// Update state to accord with a view.
    fn inject(&self, view: T) -> Self;
}

/// Blanket implementation views the whole of Self
impl<S> Lens<S> for S {
    fn extract(&self) -> &Self {
        self
    }

    fn inject(&self, part: Self) -> Self {
        part
    }
}

/// An event is something that may cause a state transition
pub trait Event<S> {
    fn fire(&self, state: &S) -> Transition<S>;
}

/// A command executes an effect dependent on state and an effect handler.
/// It may produce an event.
pub trait Command<S, H> {
    type Output: Event<S>;
    fn execute(&self, state: &S, handler: &mut H) -> Option<Self::Output>;
}

/// Describes the behavior of a Finite State Machine (FSM) that can receive commands and produce
/// events. Along the way, effects can be performed given the receipt of a command.
/// State can be reconsituted by replaying events.
///
/// The generic types refer to:
/// S  = State          - the state of your FSM
/// H = State Effect   - the effect handler, required by commands
trait Fsm<S, H> {
    /// Given a state and command, optionally emit an event. Can perform side
    /// effects along the way. This function is generally only called from the
    /// `run` function.
    fn for_command<C, T>(state: &S, command: &C, handler: &mut H) -> Option<C::Output>
    where
        C: Command<T, H>,
        S: Lens<T>,
    {
        command.execute(state.extract(), handler)
    }

    /// Given a state and event, produce a transition, which could transition to
    /// the next state. No side effects are to be performed. Can be used to replay
    /// events to attain a new state i.e. the major function of event sourcing.
    fn for_event<E, T>(state: &S, event: &E) -> Transition<S>
    where
        E: Event<T>,
        S: Lens<T>,
    {
        match event.fire(state.extract()) {
            Transition::Next(t) => Transition::Next(state.inject(t)),
            Transition::Same => Transition::Same,
        }
    }

    /// Optional logic for when transitioning into a new state.
    fn on_transition(_old_s: &S, _new_s: &S, _h: &mut H) {}

    /// This is the main entry point to the event driven FSM.
    /// Runs the state machine for a command, optionally performing effects,
    /// producing an event and transitioning to a new state. Also
    /// applies any "Entry/" or "Exit/" processing when arriving
    /// at a new state.
    fn step<C>(
        state: &S,
        command: &C,
        handler: &mut H,
    ) -> (Option<<C as Command<S, H>>::Output>, Transition<S>)
    where
        C: Command<S, H>,
    {
        let result = Self::for_command(state, command, handler);
        let trans = if let Some(event) = &result {
            let trans = Self::for_event(state, event);
            if let Transition::Next(new_s) = &trans {
                Self::on_transition(state, new_s, handler);
            };
            trans
        } else {
            Transition::Same
        };
        (result, trans)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step() {
        // Declare our state, commands and events

        #[derive(Debug, PartialEq)]
        enum State {
            Started,
            Stopped,
        }

        enum Command {
            Start,
            Stop,
        }

        #[derive(Debug, PartialEq)]
        enum Event {
            Started,
            Stopped,
        }

        // Declare an object to handle effects as we step through the FSM

        struct EffectHandlers {
            started: u32,
            stopped: u32,
            transitioned_stopped_to_started: u32,
            transitioned_started_to_stopped: u32,
        }

        impl EffectHandlers {
            pub fn start_something(&mut self) {
                self.started += 1;
            }

            pub fn stop_something(&mut self) {
                self.stopped += 1;
            }

            pub fn transitioned_started_to_stopped(&mut self) {
                self.transitioned_started_to_stopped += 1;
            }

            pub fn transitioned_stopped_to_started(&mut self) {
                self.transitioned_stopped_to_started += 1;
            }
        }

        impl super::Command<State, EffectHandlers> for Command {
            type Output = Event;
            fn execute(&self, s: &State, se: &mut EffectHandlers) -> Option<Event> {
                match (s, self) {
                    (State::Started, Command::Start) => None,
                    (State::Started, Command::Stop) => {
                        se.stop_something();
                        Some(Event::Stopped)
                    }
                    (State::Stopped, Command::Start) => {
                        se.start_something();
                        Some(Event::Started)
                    }
                    (State::Stopped, Command::Stop) => None,
                }
            }
        }

        impl super::Event<State> for Event {
            fn fire(&self, s: &State) -> Transition<State> {
                match (s, self) {
                    (State::Started, Event::Started) => Transition::Same,
                    (State::Started, Event::Stopped) => Transition::Next(State::Stopped),
                    (State::Stopped, Event::Started) => Transition::Next(State::Started),
                    (State::Stopped, Event::Stopped) => Transition::Same,
                }
            }
        }

        // Declare the FSM itself
        struct MyFsm {}

        impl Fsm<State, EffectHandlers> for MyFsm {
            // Let's implement this optional function to show how entry/exit
            // processing can be achieved, and also confirm that our FSM is
            // calling it.
            fn on_transition(old_s: &State, new_s: &State, se: &mut EffectHandlers) {
                match (old_s, new_s) {
                    (State::Started, State::Stopped) => se.transitioned_started_to_stopped(),
                    (State::Stopped, State::Started) => se.transitioned_stopped_to_started(),
                    _ => {
                        panic!("Unexpected transition");
                    }
                }
            }
        }

        // Initialize our effect handlers

        let mut se = EffectHandlers {
            started: 0,
            stopped: 0,
            transitioned_stopped_to_started: 0,
            transitioned_started_to_stopped: 0,
        };

        // Finally, test the FSM by stepping through various states

        let (e, t) = MyFsm::step(&State::Stopped, &Command::Start, &mut se);
        assert_eq!(e, Some(Event::Started));
        assert_eq!(t, Transition::Next(State::Started));
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 0);
        assert_eq!(se.transitioned_started_to_stopped, 0);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        let (e, t) = MyFsm::step(&State::Started, &Command::Start, &mut se);
        assert_eq!(e, None);
        assert_eq!(t, Transition::Same);
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 0);
        assert_eq!(se.transitioned_started_to_stopped, 0);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        let (e, t) = MyFsm::step(&State::Started, &Command::Stop, &mut se);
        assert_eq!(e, Some(Event::Stopped));
        assert_eq!(t, Transition::Next(State::Stopped));
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 1);
        assert_eq!(se.transitioned_started_to_stopped, 1);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        let (e, t) = MyFsm::step(&&State::Stopped, &Command::Stop, &mut se);
        assert_eq!(e, None);
        assert_eq!(t, Transition::Same);
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 1);
        assert_eq!(se.transitioned_started_to_stopped, 1);
        assert_eq!(se.transitioned_stopped_to_started, 1);
    }

    #[test]
    fn test_step_alt() {
        // Declare our state, commands and events

        #[derive(Debug, PartialEq)]
        enum State {
            Started,
            Stopped,
        }

        struct Start {}
        struct Stop {}

        #[derive(PartialEq, Debug)]
        struct Started {}

        #[derive(PartialEq, Debug)]
        struct Stopped {}

        // Declare an object to handle effects as we step through the FSM

        struct EffectHandlers {
            started: u32,
            stopped: u32,
            transitioned_stopped_to_started: u32,
            transitioned_started_to_stopped: u32,
        }

        impl EffectHandlers {
            pub fn start_something(&mut self) {
                self.started += 1;
            }

            pub fn stop_something(&mut self) {
                self.stopped += 1;
            }

            pub fn transitioned_started_to_stopped(&mut self) {
                self.transitioned_started_to_stopped += 1;
            }

            pub fn transitioned_stopped_to_started(&mut self) {
                self.transitioned_stopped_to_started += 1;
            }
        }

        impl Command<State, EffectHandlers> for Start {
            type Output = Started;
            fn execute(&self, s: &State, se: &mut EffectHandlers) -> Option<Started> {
                match s {
                    State::Stopped => {
                        se.start_something();
                        Some(Started {})
                    }
                    _ => None,
                }
            }
        }

        impl Command<State, EffectHandlers> for Stop {
            type Output = Stopped;
            fn execute(&self, s: &State, se: &mut EffectHandlers) -> Option<Stopped> {
                match s {
                    State::Started => {
                        se.stop_something();
                        Some(Stopped {})
                    }
                    _ => None,
                }
            }
        }

        impl Event<State> for Started {
            fn fire(&self, s: &State) -> Transition<State> {
                match s {
                    State::Stopped => Transition::Next(State::Started),
                    _ => Transition::Same,
                }
            }
        }

        impl Event<State> for Stopped {
            fn fire(&self, s: &State) -> Transition<State> {
                match s {
                    State::Started => Transition::Next(State::Stopped),
                    _ => Transition::Same,
                }
            }
        }

        // Declare the FSM itself
        struct MyFsm {}

        impl Fsm<State, EffectHandlers> for MyFsm {
            // Let's implement this optional function to show how entry/exit
            // processing can be achieved, and also confirm that our FSM is
            // calling it.
            fn on_transition(old_s: &State, new_s: &State, se: &mut EffectHandlers) {
                match (old_s, new_s) {
                    (State::Started, State::Stopped) => se.transitioned_started_to_stopped(),
                    (State::Stopped, State::Started) => se.transitioned_stopped_to_started(),
                    _ => {
                        panic!("Unexpected transition");
                    }
                }
            }
        }

        // Initialize our effect handlers

        let mut se = EffectHandlers {
            started: 0,
            stopped: 0,
            transitioned_stopped_to_started: 0,
            transitioned_started_to_stopped: 0,
        };

        // Finally, test the FSM by stepping through various states

        let (e, t) = MyFsm::step(&State::Stopped, &Start {}, &mut se);
        assert_eq!(e, Some(Started {}));
        assert_eq!(t, Transition::Next(State::Started));
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 0);
        assert_eq!(se.transitioned_started_to_stopped, 0);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        let (e, t) = MyFsm::step(&State::Started, &Start {}, &mut se);
        assert_eq!(e, None);
        assert_eq!(t, Transition::Same);
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 0);
        assert_eq!(se.transitioned_started_to_stopped, 0);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        let (e, t) = MyFsm::step(&State::Started, &Stop {}, &mut se);
        assert_eq!(e, Some(Stopped {}));
        assert_eq!(t, Transition::Next(State::Stopped));
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 1);
        assert_eq!(se.transitioned_started_to_stopped, 1);
        assert_eq!(se.transitioned_stopped_to_started, 1);

        let (e, t) = MyFsm::step(&&State::Stopped, &Stop {}, &mut se);
        assert_eq!(e, None);
        assert_eq!(t, Transition::Same);
        assert_eq!(se.started, 1);
        assert_eq!(se.stopped, 1);
        assert_eq!(se.transitioned_started_to_stopped, 1);
        assert_eq!(se.transitioned_stopped_to_started, 1);
    }
}
