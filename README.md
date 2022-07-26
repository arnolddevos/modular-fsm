# Ideas for taming Finite State Machines

In our model of a _Finite State Machine_ there are two functions
which are iterated to evolve a state:

```rust
(Command, State) -> Event // performs a side effect
(Event, State) -> State // a pure state transition
```

We refer to these as _state functions_.
In the most direct implementation, `Command`, `Event` and `State` 
are all concrete types, typically `enum` types.

In a very large FSM these two principal state functions divide
the enum cases and delegate to a number of smaller state
functions.

## The Problem

The issue is that any change in the specification of the FSM implies
a change to one or more of the three principal types.

That can then have widespread consequences in the state functions.
Inevitably, these impacts will go beyond the areas
directly concerned with the specified change.

For example, in one project the FSM required ~1400 loc with a further
~2800 loc for tests. Adding a member to the `State` type typically 
required a ~2000 loc diff. 

(Examples surveyed varied between ~1600 and ~3300 loc.)

The proposition here is that coupling accross this code base 
can be reduced by introducing traits and making the principal
state functions generic.

## Modularity with Command and Event Traits

A _Modular Finite State Machine_ (MFSM) relies on traits 
rather than enums to define commands and events. 
Each command (or event) consists of a distinct concrete type and 
an implementation of the `Command` (or `Event`) trait. 
This provides the state function for that particular command (or event). 

The principal state functions still delegate to these smaller
functions but the principal functions are now generic. 
They do not evolve when new commands and events are defined. 
They are the same for every MFSM.  

As there are no global command and event types, defining new
commands and events need not impact any existing definitions.
Definitions can be easily organised into separate modules. 

## Views on State with the Lens Trait

The MFSM design uses another trait, `Lens`, to decouple command
and event definitions from the state type.

The state of the MFSM is a concrete type with at least one `Lens`. 
Each `Lens`implementation provides a different view of the state. 
An efficient, blanket `Lens` implementation views the whole of the state.  

A command or event is always defined over a view of the state.
It may be the blanket view, but a narrower view will insulate the 
command or event from unrelated changes to the state type. 
Each state function can be simpler because it avoids the need
to deconstruct and reconstruct the whole state.

However, there is no free lunch.  The global state must
be deconstructed and reconstructed in the `Lens` implementation instead.
The idea is that it still clearer to separate this logic from commands and events; 
the impact of changes is more contained; and several commands and events 
may share the same view of state, reducing duplication. 

The `Lens` trait codifies methods that are sometimes defined on `State`
to extract and update a partial state.

## Notifications

Sometimes an event serves only to signal that a particular command has been executed,
but does not describe its outcome.  This is called a _notification_ and it
is a pure function of the associated command and state.

Notifications can create duplications between command types and event types.  
In the extreme, for each command type there is a similar, corresponding event type.

In the MFSM design a notification is best defined by implementing both `Command` and `Event` 
for one type.  If the command is run, the state function should return it as a notfication.  

## Testing

The MFSM approach ought to simplify testing in the same way it simplifies state functions.  

The biggest contribution to FSM test code is boilerplate to construct an initial and final state.  
This boilerplate must be updated whenever something is added or changed in the state type.   

The MFSM approach is to provide a separate test for each `Lens` implementation as well as each
`Command`.  The former are affected by changes in the state type but not necessarily the latter. 
Command tests involve more manageable initial and final view values.  
They can be easily organised into modules along with the commands.

## The FSM trait

Given the devolution of individual state functions to their respective command and event
definitions, what is the purpose of the `FSM` trait?   

It should be seen as the custodian of state.  Commands are submitted to an `FSM` to gain
access to the state.  The `FSM` trait provides generic, principal state functions and
the `step` function to sequence them as a single iteration.

The `FSM` trait also provides a hook to monitor state transitions and 
generate effects based on them.
Note: in the `MFSM` design the command and event that caused the transition 
are not available (their types are generic within the `FSM`).  

## Command Channels

Commands typically arrive at the `FSM` via a channel.   It may be necessary to unify several
command types as a common type for the channel.  This can be done by enum over those types.
If there is more than one command channel they may have different types.

## Logging Events

Logging of events is treated as a separate concern. 

If the log defines a type, conversions to and from that type must be provided
for each event.  For example, using the `From` trait. 

Another possibility is to derive `serde` `Serialize` and `Deserialize` 
for each event type.

