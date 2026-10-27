---
status: accepted
date: 2026-10-18
decision-makers: Kheagen Haskins
---

# One Gate for Every Change

## Context and Problem Statement

The world is the state that the simulation advances. It covers the terrain, the villagers, the
buildings and every faction.

Many actors change the world. The player acts through the god's hand, through miracles and spells,
and through a possessed creature. The AI acts too. It acts through the planner that decides for each
village, and through each rival faction.

A level also sets rules. Each rule limits what those actors may do.

Cosy mode is the clearest of these rules. A player can switch on a mode in which no faction attacks
them and they attack no one. A hostile action, a trade or any approach to a possible ally switches
cosy mode off.

A rule of this kind applies to the player and the AI at once. It also touches several kinds of
change to the world.

Where does the code check the rules of the level, and how does every change to the world reach that
check?

## Decision Drivers

- A rule of the level binds the player and the AI alike. In cosy mode, an attack by a faction on the
  player breaks the rule as surely as an attack by the player on a faction.
- One rule can cover several kinds of change. Cosy mode ends on a hostile action, a trade or an
  approach to a possible ally.
- The rules of the level will grow. The README lists two ways to stop a player from exploiting cosy
  mode, and the choice between them remains open. One way fixes cosy mode for the whole level. The
  other allows only the actions that keep the civilisation going. Either one adds a rule for the code
  to enforce.

## Considered Options

- One gate that checks every request
- Each system checks the rules itself
- Each actor checks the rules before it acts

## Decision Outcome

Chosen option: "One gate that checks every request", because the player and the AI then reach the
world in the same way, and a rule such as cosy mode becomes one check at the gate.

Every change to the world arrives as a request. A request is a small piece of data. It states which
actor wants to do what. The player and the AI alike issue requests.

A single gate checks each request against the rules of the level. The request takes effect only once
the gate has passed it.

Several points remain open. The developer settles them once the simulation has code for them:

- the fields of a request, and how Sarx represents one in Rust
- the API of the gate, and where in each step of the simulation it runs
- how an actor learns that the gate refused its request
- which updates count as a change to the world. The systems that move objects under physics update
  the world on every step. Whether those updates pass through the gate as requests is undecided.

### Consequences

- Good, because each rule of the level is written once, at the gate. Adding or changing a rule
  changes the gate alone.
- Good, because the same code checks the player and the AI. A rule therefore cannot apply to one and
  miss the other.
- Good, because every hostile action, trade and approach to a possible ally passes through the gate.
  As a result, the check that switches cosy mode off has one place to watch.
- Bad, because each kind of change to the world needs two things. One is a request that
  describes the change. The other is code that applies the request once the gate has passed it.
- Bad, because an actor learns of a refusal only after it has decided. An AI planner that chooses a
  forbidden action then has to choose again.
- Bad, because every request passes through one more step before it takes effect. The gate also
  runs its checks on every request, in every step of the simulation.

### Confirmation

Nothing in the code embodies this decision yet. `sarx-sim` is the crate for the simulation, and it
contains only the function that `cargo new` generates.

Once the simulation changes the world, a reviewer confirms three points:

- Every change to the world starts as a request.
- Each rule of the level appears as a check in the gate.
- No system, planner or input handler tests a rule of the level on its own.

## Pros and Cons of the Options

### One Gate that Checks Every Request

Each actor issues a request. One gate checks the request against the rules of the level before it
takes effect.

- Good, because each rule is written in one place, whatever actor or kind of change it covers.
- Good, because a request is a plain value. A developer can print or record it while debugging.
- Bad, because every kind of change needs a request type and the code that applies it.
- Bad, because an actor finds out about a refusal only after it has chosen its action.

### Each System Checks the Rules Itself

Each system that changes the world tests the rules of the level before it applies a change. The
system that resolves an attack, for example, tests whether either side is in cosy mode.

- Good, because a system changes the world directly, with no request type in between.
- Good, because each check lies next to the change that it guards.
- Bad, because a rule that covers several kinds of change becomes several checks. Cosy mode, for
  example, needs a check in each system that applies one of those changes.
- Bad, because a new system can leave out a check. It then lets through a change that the rules
  forbid, and the code shows no sign of the gap.

### Each Actor Checks the Rules Before It Acts

The code that turns the input of the player into actions discards any action that the level forbids.
Each AI planner does the same, before it acts.

- Good, because an AI planner never chooses an action that the level forbids.
- Bad, because each rule needs one copy for the player and another for the AI. The two copies can
  drift apart.
- Bad, because code that changes the world without going through an actor bypasses the rules.

## More Information

A Java developer will recognise the command pattern here. In that pattern, each action becomes an
object. One invoker runs every command, which gives it one place to check, log or refuse them. The
gate plays the part of the invoker.

The comparison stops holding at the request itself. A Java command usually contains an `execute`
method, and performs its own work. A request in Sarx is data alone, stating which actor wants to do
what. Other code applies the change, and that code runs only once the gate has passed the request.
